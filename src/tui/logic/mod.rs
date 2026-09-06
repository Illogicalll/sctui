pub(crate) mod filtering;
mod input;
pub(crate) mod state;
pub(crate) mod utils;

use crate::api::{
    Lyrics, fetch_lyrics,
    add_track_to_playlist, create_playlist, delete_playlist, remove_track_from_playlist,
    fetch_related_tracks,
    API, Activity, Album, Artist, Playlist, Track, engage, fetch_playlist_tracks,
    fetch_search_albums, fetch_search_people, fetch_search_playlists, fetch_search_tracks,
    fetch_user_tracks,
};
use crate::player::Player;
use ratatui::crossterm::{
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    execute,
    terminal::supports_keyboard_enhancement,
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event},
};

use std::result::Result::Ok;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ratatui_image::{
    errors::Errors,
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
};
use reqwest::Method;
use image::DynamicImage;

use super::render::render;
use self::filtering::{build_filtered_views, clamp_selection, is_filter_active};
use self::input::helpers::reset_search_rows;
use self::input::{InputOutcome, handle_key_event, next_track, prev_track, toggle_play_pause};
use crate::media::{Media, MediaCommand};
use self::state::{AppData, AppState, Engagement, FollowingTracksFocus, LyricsStatus, PlaybackSource, PlaylistEdit};
use self::utils::{
    enter_radio,
    active_tracks, append_feed_tracks, play_queued_track, queued_from_current,
};

/// How long a track must have been playing before its lyrics are looked up.
const LYRICS_DEBOUNCE: Duration = Duration::from_millis(400);

enum AppEvent {
    Redraw(Result<ResizeResponse, Errors>),
    /// Cover art for `url`, downloaded off the UI thread; `None` if it failed.
    /// The path is a local copy of the bytes for the OS media integration.
    Artwork(String, Option<DynamicImage>, Option<std::path::PathBuf>),
}

/// Where a track's artwork bytes are cached for the OS media panel. souvlaki on
/// macOS loads the cover with `NSImage` and aborts the process if that returns
/// nil, which remote URLs do intermittently; a local file never does.
fn artwork_cache_path(url: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut h);
    std::env::temp_dir().join(format!("sctui-art-{:016x}.img", h.finish()))
}

/// Results of background fetches, folded into the app state at the top of each loop pass.
/// The `u64` is the request id the result belongs to; stale ones are dropped.
enum Msg {
    Likes(Vec<Track>),
    Playlists(Vec<Playlist>),
    Albums(Vec<Album>),
    Following(Vec<Artist>),
    PlaylistTracks(u64, Vec<Track>),
    AlbumTracks(u64, Vec<Track>),
    FollowingTracks(u64, Vec<Track>),
    FollowingLikes(u64, Vec<Track>),
    SearchTracks(u64, Vec<Track>),
    SearchAlbums(u64, Vec<Album>),
    SearchPlaylists(u64, Vec<Playlist>),
    SearchPeople(u64, Vec<Artist>),
    SearchPlaylistTracks(u64, Vec<Track>),
    SearchAlbumTracks(u64, Vec<Track>),
    SearchPeopleTracks(u64, Vec<Track>),
    SearchPeopleLikes(u64, Vec<Track>),
    Feed(Vec<Activity>),
    FeedTracks(u64, Vec<Track>),
    /// Tracks of one later feed item, to append to the queue while the feed plays.
    FeedQueue(u64, Vec<Track>),
    /// Related tracks for the radio seed, to append to the queue.
    Related(u64, Vec<Track>),
    /// A playlist the user just created, to show at the top of the library.
    PlaylistCreated(Playlist),
    /// Lyrics lookup result for a track URN (`None` = nothing found).
    Lyrics(String, Option<Lyrics>),
    Engagement(Engagement),
}

pub fn run(api: &mut Arc<Mutex<API>>, player: Player, config: crate::config::Loaded) -> anyhow::Result<()> {
    color_eyre::install().map_err(|e| anyhow::anyhow!(e))?;
    let terminal = ratatui::init();
    // Lets terminals that speak the kitty keyboard protocol report Shift+Enter; others still
    // send a plain Enter (Shift+G covers them).
    let enhanced_keys = supports_keyboard_enhancement().unwrap_or(false);
    if enhanced_keys {
        let _ = execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
    }
    let result = start(terminal, api, player, config);
    if enhanced_keys {
        let _ = execute!(std::io::stdout(), PopKeyboardEnhancementFlags);
    }
    ratatui::restore();
    result
}

/// Make the next `draw` rewrite every cell without erasing the screen first.
/// `Terminal::clear` sends an erase-display sequence, which shows as a blank
/// flash. Filling the spare buffer with a sentinel and swapping it in as the
/// "previous frame" makes every cell look changed, so the diff repaints them
/// all in the same flush as the new frame.
fn invalidate_screen(terminal: &mut DefaultTerminal) {
    for cell in terminal.current_buffer_mut().content.iter_mut() {
        cell.set_symbol("\u{E000}");
    }
    terminal.swap_buffers();
}

fn spawn_fetch<F>(api: Arc<Mutex<API>>, tx: Sender<Msg>, fetch_fn: F)
where
    F: FnOnce(&mut API) -> anyhow::Result<Msg> + Send + 'static,
{
    std::thread::spawn(move || {
        let result = {
            let mut api_guard = api.lock().unwrap();
            fetch_fn(&mut api_guard)
        };

        if let Ok(item) = result {
            let _ = tx.send(item);
        }
    });
}

/// Runs `fut` on the runtime and forwards its tracks tagged with `request_id`.
fn spawn_tracks(
    rt: &tokio::runtime::Runtime,
    tx: &Sender<Msg>,
    request_id: u64,
    fut: impl Future<Output = anyhow::Result<Vec<Track>>> + Send + 'static,
    wrap: fn(u64, Vec<Track>) -> Msg,
) -> tokio::task::JoinHandle<()> {
    let tx = tx.clone();
    rt.spawn(async move {
        if let Ok(tracks) = fut.await {
            let _ = tx.send(wrap(request_id, tracks));
        }
    })
}

fn start(
    mut terminal: DefaultTerminal,
    api: &mut Arc<Mutex<API>>,
    player: Player,
    config: crate::config::Loaded,
) -> anyhow::Result<()> {
    let mut state = AppState::default();
    state.keymap = config.keymap;
    state.theme_name = config.theme_name;
    state.theme_overrides = config.theme_overrides;

    let mut api_guard = api.lock().unwrap();
    let mut data = AppData::new(&mut api_guard, state.selected_row)?;
    drop(api_guard);

    let async_rt = tokio::runtime::Runtime::new().unwrap();
    // Fetch threads hold the `api` lock for whole HTTP requests, so the UI thread must never
    // take it. The token Arc is stable across re-auth (main.rs rebuilds the API around the
    // same Arc), so clone it once here.
    let token = api.lock().unwrap().token_clone();
    let auth = || Arc::clone(&token);

    let (tx, rx) = mpsc::channel::<Msg>();

    spawn_fetch(Arc::clone(api), tx.clone(), |api| {
        api.get_playlists().map(Msg::Playlists)
    });
    spawn_fetch(Arc::clone(api), tx.clone(), |api| {
        api.get_activities().map(Msg::Feed)
    });

    let mut picker = Picker::from_query_stdio()?;

    let (tx_worker, rx_worker) = mpsc::channel::<ResizeRequest>();
    let (tx_main, rx_main) = mpsc::channel::<AppEvent>();

    {
        let tx_main_render = tx_main.clone();
        std::thread::spawn(move || loop {
            if let Ok(request) = rx_worker.recv() {
                tx_main_render
                    .send(AppEvent::Redraw(request.resize_encode()))
                    .unwrap();
            }
        });
    }

    let mut cover_art_async = ThreadProtocol::new(tx_worker.clone(), None);
    let mut last_artwork_url: Option<String> = None;
    let mut last_artwork_file: Option<(String, std::path::PathBuf)> = None;
    let mut last_artwork_image: Option<DynamicImage> = None;
    let mut artwork_pending: Option<String> = None;

    let wave_buffer = player.wave_buffer();
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();
    // A full repaint whenever the visible view changes. ratatui only redraws changed cells,
    // so a glyph the terminal sized differently from unicode-width can leave a ghost behind;
    // repainting at view boundaries wipes any that slipped past sanitize_display.
    let mut last_view = None;

    // OS media integration (Now Playing widget, media keys). `media_*` remember what the OS
    // was last told so it is only updated on change, plus a periodic position re-sync.
    let (mut media, media_rx) = Media::new();
    let mut media_track_urn: Option<String> = None;
    let mut lyrics_cache: std::collections::HashMap<String, Option<Lyrics>> = std::collections::HashMap::new();
    let mut lyrics_pending: Option<(Track, Instant)> = None;
    let mut media_cover: Option<std::path::PathBuf> = None;
    let mut media_playing: Option<bool> = None;
    let mut media_synced_at = Instant::now();
    let mut media_synced_pos: u64 = 0;

    loop {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Msg::Likes(new) => {
                    for t in &new {
                        data.liked_track_urns.insert(t.track_urn.clone());
                    }
                    data.likes.extend(new);
                }
                Msg::Playlists(new) => {
                    for p in &new {
                        if !p.is_owned {
                            data.liked_playlist_uris.insert(p.tracks_uri.clone());
                        }
                    }
                    data.playlists.extend(new);
                }
                Msg::Albums(new) => {
                    for a in &new {
                        data.liked_album_uris.insert(a.tracks_uri.clone());
                    }
                    data.albums.extend(new);
                }
                Msg::Following(new) => {
                    for a in &new {
                        data.followed_user_urns.insert(a.urn.clone());
                    }
                    data.following.extend(new);
                }
                Msg::PlaylistTracks(id, t) => state.playlist_tracks_fetch.accept(
                    id, t, &mut data.playlist_tracks, &mut data.playlist_tracks_state,
                ),
                Msg::AlbumTracks(id, t) => state.album_tracks_fetch.accept(
                    id, t, &mut data.album_tracks, &mut data.album_tracks_state,
                ),
                Msg::FollowingTracks(id, t) => state.following_tracks_fetch.accept(
                    id, t, &mut data.following_tracks, &mut data.following_tracks_state,
                ),
                Msg::FollowingLikes(id, t) => state.following_likes_fetch.accept(
                    id, t, &mut data.following_likes_tracks, &mut data.following_likes_state,
                ),
                Msg::SearchTracks(id, t) => state.search_results_fetch.accept(
                    id, t, &mut data.search_tracks, &mut data.search_tracks_state,
                ),
                Msg::SearchAlbums(id, a) => state.search_results_fetch.accept(
                    id, a, &mut data.search_albums, &mut data.search_albums_state,
                ),
                Msg::SearchPlaylists(id, p) => state.search_results_fetch.accept(
                    id, p, &mut data.search_playlists, &mut data.search_playlists_state,
                ),
                Msg::SearchPeople(id, p) => state.search_results_fetch.accept(
                    id, p, &mut data.search_people, &mut data.search_people_state,
                ),
                Msg::SearchPlaylistTracks(id, t) => state.search_playlist_tracks_fetch.accept(
                    id, t, &mut data.search_playlist_tracks, &mut data.search_playlist_tracks_state,
                ),
                Msg::SearchAlbumTracks(id, t) => state.search_album_tracks_fetch.accept(
                    id, t, &mut data.search_album_tracks, &mut data.search_album_tracks_state,
                ),
                Msg::SearchPeopleTracks(id, t) => state.search_people_tracks_fetch.accept(
                    id, t, &mut data.search_people_tracks, &mut data.search_people_tracks_state,
                ),
                Msg::SearchPeopleLikes(id, t) => state.search_people_likes_fetch.accept(
                    id, t, &mut data.search_people_likes_tracks, &mut data.search_people_likes_state,
                ),
                Msg::Feed(new) => data.feed.extend(new),
                Msg::FeedTracks(id, t) => state.feed_tracks_fetch.accept(
                    id, t, &mut data.feed_tracks, &mut data.feed_tracks_state,
                ),
                Msg::FeedQueue(id, t) => {
                    if id == state.feed_queue_fetch.request_id
                        && state.playback_source == PlaybackSource::Feed
                    {
                        append_feed_tracks(&mut data.playback_tracks, &mut state.auto_queue, t);
                    }
                }
                Msg::Related(id, tracks) => {
                    if id == state.radio_fetch.request_id
                        && state.playback_source == PlaybackSource::Radio
                    {
                        append_feed_tracks(&mut data.playback_tracks, &mut state.auto_queue, tracks);
                        if state.radio_waiting
                            && let Some(next_idx) = state.auto_queue.pop_front()
                            && let Some(track) = data.playback_tracks.get(next_idx).cloned()
                        {
                            if let Some(current) = queued_from_current(&state, &data) {
                                state.playback_history.push(current);
                            }
                            player.play(track);
                            state.override_playing = None;
                            state.current_playing_index = Some(next_idx);
                            state.radio_waiting = false;
                        }
                    }
                }
                Msg::PlaylistCreated(playlist) => {
                    // Keep the cursor on the same playlist as the list shifts down by one.
                    if state.selected_tab == 0 && state.selected_subtab == 1 && !data.playlists.is_empty() {
                        state.selected_row += 1;
                        data.playlists_state.select(Some(state.selected_row));
                    }
                    data.playlists.insert(0, playlist);
                }
                Msg::Lyrics(urn, found) => {
                    lyrics_cache.insert(urn.clone(), found.clone());
                    if state.lyrics_track_urn.as_deref() == Some(urn.as_str()) {
                        state.lyrics = match found {
                            Some(l) => LyricsStatus::Found(l),
                            None => LyricsStatus::NotFound,
                        };
                    }
                }
                Msg::Engagement(done) => done.apply(&mut state, &mut data),
            }
        }

        while let Some(action) = state.engagement_queue.pop_front() {
            let token = auth();
            let tx = tx.clone();
            let (method, path) = match &action {
                Engagement::LikeTrack { track_id, .. } => {
                    (Method::POST, format!("likes/tracks/{}", track_id))
                }
                Engagement::UnlikeTrack { track_id, .. } => {
                    (Method::DELETE, format!("likes/tracks/{}", track_id))
                }
                Engagement::LikePlaylist { playlist_id, .. }
                | Engagement::LikeAlbum { playlist_id, .. } => {
                    (Method::POST, format!("likes/playlists/{}", playlist_id))
                }
                Engagement::UnlikePlaylist { playlist_id, .. }
                | Engagement::UnlikeAlbum { playlist_id, .. } => {
                    (Method::DELETE, format!("likes/playlists/{}", playlist_id))
                }
                Engagement::FollowUser { user_id, .. } => {
                    (Method::PUT, format!("me/followings/{}", user_id))
                }
                Engagement::UnfollowUser { user_id, .. } => {
                    (Method::DELETE, format!("me/followings/{}", user_id))
                }
            };
            async_rt.spawn(async move {
                if engage(token, method, path).await.is_ok() {
                    let _ = tx.send(Msg::Engagement(action));
                }
            });
        }

        while let Some(edit) = state.playlist_edit_queue.pop_front() {
            let token = auth();
            let tx = tx.clone();
            async_rt.spawn(async move {
                match edit {
                    PlaylistEdit::Add { playlist_id, tracks_uri, track_urn } => {
                        let _ = add_track_to_playlist(token, playlist_id, tracks_uri, track_urn).await;
                    }
                    PlaylistEdit::Remove { playlist_id, tracks_uri, track_urn } => {
                        let _ = remove_track_from_playlist(token, playlist_id, tracks_uri, track_urn).await;
                    }
                    PlaylistEdit::Create { title, track_urn, public } => {
                        if let Ok(playlist) = create_playlist(token, title, track_urn, public).await {
                            let _ = tx.send(Msg::PlaylistCreated(playlist));
                        }
                    }
                    PlaylistEdit::Delete { playlist_id } => {
                        let _ = delete_playlist(token, playlist_id).await;
                    }
                }
            });
        }

        let current_artwork_url = player.current_track().artwork_url;
        while let Ok(app_ev) = rx_main.try_recv() {
            match app_ev {
                AppEvent::Redraw(completed) => {
                    let _ = cover_art_async.update_resized_protocol(completed?);
                }
                AppEvent::Artwork(url, image, file) => {
                    if artwork_pending.as_deref() == Some(url.as_str()) {
                        artwork_pending = None;
                    }
                    if url != current_artwork_url {
                        continue; // track changed while this was downloading
                    }
                    last_artwork_file = file.map(|path| (url.clone(), path));
                    last_artwork_url = Some(url);
                    match image {
                        Some(image) => {
                            let resize_proto = picker.new_resize_protocol(image.clone());
                            cover_art_async =
                                ThreadProtocol::new(tx_worker.clone(), Some(resize_proto));
                            last_artwork_image = Some(image);
                        }
                        None => {
                            cover_art_async.empty_protocol();
                            last_artwork_image = None;
                        }
                    }
                }
            }
        }

        // Cover art downloads off the UI thread; the result arrives as an AppEvent above.
        if last_artwork_url.as_deref() != Some(current_artwork_url.as_str())
            && artwork_pending.as_deref() != Some(current_artwork_url.as_str())
        {
            artwork_pending = Some(current_artwork_url.clone());
            let tx = tx_main.clone();
            std::thread::spawn(move || {
                let bytes = reqwest::blocking::get(current_artwork_url.as_str())
                    .and_then(|r| r.bytes())
                    .ok();
                let image = bytes.as_ref().and_then(|b| image::load_from_memory(b).ok());
                let file = bytes.filter(|_| image.is_some()).and_then(|b| {
                    let path = artwork_cache_path(&current_artwork_url);
                    std::fs::write(&path, &b).ok().map(|_| path)
                });
                let _ = tx.send(AppEvent::Artwork(current_artwork_url, image, file));
            });
        }

        let filter_active = is_filter_active(&state);
        let filtered = build_filtered_views(&state, &data);
        let likes_len = if filter_active && state.selected_subtab == 0 {
            filtered.likes.len()
        } else {
            data.likes.len()
        };
        let playlist_tracks_len = if filter_active && state.selected_subtab == 1 {
            filtered.playlist_tracks.len()
        } else {
            data.playlist_tracks.len()
        };
        let albums_len = if filter_active && state.selected_subtab == 2 {
            filtered.albums.len()
        } else {
            data.albums.len()
        };
        let following_len = if filter_active && state.selected_subtab == 3 {
            filtered.following.len()
        } else {
            data.following.len()
        };

        clamp_selection(
            &mut state,
            &mut data,
            filter_active,
            likes_len,
            playlist_tracks_len,
            albums_len,
            following_len,
        );

        let albums_ref = if filter_active && state.selected_subtab == 2 {
            &filtered.albums
        } else {
            &data.albums
        };
        let following_ref = if filter_active && state.selected_subtab == 3 {
            &filtered.following
        } else {
            &data.following
        };

        if state.selected_tab == 0 && state.selected_subtab == 1 {
            let selected = data.playlists.get(state.selected_row).map(|p| p.tracks_uri.clone());
            state.playlist_tracks_fetch.refetch(
                &mut data.playlist_tracks,
                &mut data.playlist_tracks_state,
                &mut data.playlist_tracks_uri,
                &mut state.selected_playlist_track_row,
                selected,
                |id, uri| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_playlist_tracks(auth(), uri),
                    Msg::PlaylistTracks,
                ),
            );
        }

        if state.selected_tab == 0 && state.selected_subtab == 2 {
            let selected = albums_ref.get(state.selected_row).map(|a| a.tracks_uri.clone());
            state.album_tracks_fetch.refetch(
                &mut data.album_tracks,
                &mut data.album_tracks_state,
                &mut data.album_tracks_uri,
                &mut state.selected_album_track_row,
                selected,
                |id, uri| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_playlist_tracks(auth(), uri),
                    Msg::AlbumTracks,
                ),
            );
        }

        if state.selected_tab == 0 && state.selected_subtab == 3 {
            let selected = following_ref.get(state.selected_row).map(|a| a.urn.clone());
            let reset = state.following_tracks_fetch.refetch(
                &mut data.following_tracks,
                &mut data.following_tracks_state,
                &mut data.following_tracks_user_urn,
                &mut state.selected_following_track_row,
                selected.clone(),
                |id, urn| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_user_tracks(auth(), urn, "tracks"),
                    Msg::FollowingTracks,
                ),
            );
            if reset {
                state.following_tracks_focus = FollowingTracksFocus::Published;
            }
            state.following_likes_fetch.refetch(
                &mut data.following_likes_tracks,
                &mut data.following_likes_state,
                &mut data.following_likes_user_urn,
                &mut state.selected_following_like_row,
                selected,
                |id, urn| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_user_tracks(auth(), urn, "likes/tracks"),
                    Msg::FollowingLikes,
                ),
            );
        }

        if state.selected_tab == 2 {
            let selected = data.feed.get(state.selected_row);
            let single = selected.and_then(|a| a.track.clone());
            let key = selected.map(|a| a.key.clone());
            state.feed_tracks_fetch.refetch(
                &mut data.feed_tracks,
                &mut data.feed_tracks_state,
                &mut data.feed_tracks_key,
                &mut state.selected_info_row,
                key,
                |id, key| match single {
                    // A track post needs no fetch; deliver it through the same channel.
                    Some(track) => {
                        let tx = tx.clone();
                        async_rt.spawn(async move {
                            let _ = tx.send(Msg::FeedTracks(id, vec![track]));
                        })
                    }
                    None => spawn_tracks(
                        &async_rt, &tx, id,
                        fetch_playlist_tracks(auth(), key),
                        Msg::FeedTracks,
                    ),
                },
            );
        }

        // Radio: keep related tracks queued behind whatever is playing. Seeded by the last
        // track in the chain so each fetch drifts a little further from the start.
        if state.playback_source == PlaybackSource::Radio
            && state.manual_queue.is_empty()
            && state.auto_queue.len() <= 1
        {
            if let Some(seed) = data.playback_tracks.last().map(|t| t.track_urn.clone()) {
                if state.radio_fetched_for.as_deref() != Some(seed.as_str()) {
                    state.radio_fetched_for = Some(seed.clone());
                    state.radio_fetch.cancel();
                    let id = state.radio_fetch.request_id;
                    state.radio_fetch.task = Some(spawn_tracks(
                        &async_rt, &tx, id,
                        fetch_related_tracks(auth(), seed),
                        Msg::Related,
                    ));
                }
            }
        }

        // Enter in the feed: keep the queue going with the items after the one playing, in
        // feed order. Track items are appended at once; sets are fetched one at a time.
        if let Some(from) = state.feed_expand_from.take() {
            state.feed_queue_fetch.cancel();
            let id = state.feed_queue_fetch.request_id;
            let later: Vec<(Option<Track>, String)> = data
                .feed
                .iter()
                .skip(from + 1)
                .map(|a| (a.track.clone(), a.key.clone()))
                .collect();
            let token = auth();
            let tx = tx.clone();
            state.feed_queue_fetch.task = Some(async_rt.spawn(async move {
                for (track, key) in later {
                    let tracks = match track {
                        Some(track) => vec![track],
                        None => match fetch_playlist_tracks(Arc::clone(&token), key).await {
                            Ok(tracks) => tracks,
                            Err(_) => continue,
                        },
                    };
                    if tx.send(Msg::FeedQueue(id, tracks)).is_err() {
                        return;
                    }
                }
            }));
        }

        if state.selected_tab == 1 && state.search_needs_fetch {
            state.search_results_fetch.cancel();
            state.search_playlist_tracks_fetch.cancel();
            state.search_album_tracks_fetch.cancel();
            state.search_people_tracks_fetch.cancel();
            state.search_people_likes_fetch.cancel();

            let request_id = state.search_results_fetch.request_id;
            let token = auth();
            let query = state.query.clone();
            let filter = state.selected_searchfilter;

            reset_search_rows(&mut state);

            data.search_tracks.clear();
            data.search_tracks_state.select(Some(0));
            data.search_albums.clear();
            data.search_albums_state.select(Some(0));
            data.search_playlists.clear();
            data.search_playlists_state.select(Some(0));
            data.search_people.clear();
            data.search_people_state.select(Some(0));

            data.search_playlist_tracks.clear();
            data.search_playlist_tracks_state.select(Some(0));
            data.search_playlist_tracks_uri = None;

            data.search_album_tracks.clear();
            data.search_album_tracks_state.select(Some(0));
            data.search_album_tracks_uri = None;

            data.search_people_tracks.clear();
            data.search_people_tracks_state.select(Some(0));
            data.search_people_tracks_user_urn = None;

            data.search_people_likes_tracks.clear();
            data.search_people_likes_state.select(Some(0));
            data.search_people_likes_user_urn = None;

            state.search_needs_fetch = false;

            if !query.trim().is_empty() {
                let tx = tx.clone();
                state.search_results_fetch.task = Some(async_rt.spawn(async move {
                    match filter {
                        0 => {
                            if let Ok(tracks) = fetch_search_tracks(token, query).await {
                                let _ = tx.send(Msg::SearchTracks(request_id, tracks));
                            }
                        }
                        1 => {
                            if let Ok(albums) = fetch_search_albums(token, query).await {
                                let _ = tx.send(Msg::SearchAlbums(request_id, albums));
                            }
                        }
                        2 => {
                            if let Ok(playlists) = fetch_search_playlists(token, query).await {
                                let _ = tx.send(Msg::SearchPlaylists(request_id, playlists));
                            }
                        }
                        3 => {
                            if let Ok(people) = fetch_search_people(token, query).await {
                                let _ = tx.send(Msg::SearchPeople(request_id, people));
                            }
                        }
                        _ => {}
                    }
                }));
            }
        }

        if state.selected_tab == 1 {
            match state.selected_searchfilter {
                0 => {
                    if !data.search_tracks.is_empty() && state.selected_row >= data.search_tracks.len() {
                        state.selected_row = data.search_tracks.len() - 1;
                    }
                    data.search_tracks_state.select(Some(state.selected_row));
                }
                1 => {
                    if !data.search_albums.is_empty() && state.selected_row >= data.search_albums.len() {
                        state.selected_row = data.search_albums.len() - 1;
                        data.search_albums_state.select(Some(state.selected_row));
                    }
                }
                2 => {
                    if !data.search_playlists.is_empty()
                        && state.selected_row >= data.search_playlists.len()
                    {
                        state.selected_row = data.search_playlists.len() - 1;
                        data.search_playlists_state.select(Some(state.selected_row));
                    }
                }
                3 => {
                    if !data.search_people.is_empty() && state.selected_row >= data.search_people.len() {
                        state.selected_row = data.search_people.len() - 1;
                        data.search_people_state.select(Some(state.selected_row));
                    }
                }
                _ => {}
            }
        }

        if state.selected_tab == 1 && state.selected_searchfilter == 2 {
            let selected = data.search_playlists.get(state.selected_row).map(|p| p.tracks_uri.clone());
            state.search_playlist_tracks_fetch.refetch(
                &mut data.search_playlist_tracks,
                &mut data.search_playlist_tracks_state,
                &mut data.search_playlist_tracks_uri,
                &mut state.search_selected_playlist_track_row,
                selected,
                |id, uri| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_playlist_tracks(auth(), uri),
                    Msg::SearchPlaylistTracks,
                ),
            );
        }

        if state.selected_tab == 1 && state.selected_searchfilter == 1 {
            let selected = data.search_albums.get(state.selected_row).map(|a| a.tracks_uri.clone());
            state.search_album_tracks_fetch.refetch(
                &mut data.search_album_tracks,
                &mut data.search_album_tracks_state,
                &mut data.search_album_tracks_uri,
                &mut state.search_selected_album_track_row,
                selected,
                |id, uri| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_playlist_tracks(auth(), uri),
                    Msg::SearchAlbumTracks,
                ),
            );
        }

        if state.selected_tab == 1 && state.selected_searchfilter == 3 {
            let selected = data.search_people.get(state.selected_row).map(|a| a.urn.clone());
            let reset = state.search_people_tracks_fetch.refetch(
                &mut data.search_people_tracks,
                &mut data.search_people_tracks_state,
                &mut data.search_people_tracks_user_urn,
                &mut state.search_selected_person_track_row,
                selected.clone(),
                |id, urn| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_user_tracks(auth(), urn, "tracks"),
                    Msg::SearchPeopleTracks,
                ),
            );
            if reset {
                state.search_people_tracks_focus = FollowingTracksFocus::Published;
            }
            state.search_people_likes_fetch.refetch(
                &mut data.search_people_likes_tracks,
                &mut data.search_people_likes_state,
                &mut data.search_people_likes_user_urn,
                &mut state.search_selected_person_like_row,
                selected,
                |id, urn| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_user_tracks(auth(), urn, "likes/tracks"),
                    Msg::SearchPeopleLikes,
                ),
            );
        }

        media.pump();
        while let Ok(command) = media_rx.try_recv() {
            match command {
                MediaCommand::Toggle => toggle_play_pause(&player),
                MediaCommand::Play => player.resume(),
                MediaCommand::Pause | MediaCommand::Stop => player.pause(),
                MediaCommand::Next => next_track(&mut state, &mut data, &player),
                MediaCommand::Previous => prev_track(&mut state, &mut data, &player),
                MediaCommand::SeekForward => player.fast_forward(),
                MediaCommand::SeekBackward => player.rewind(),
            }
        }
        {
            let track = player.current_track();
            let playing = player.is_playing();
            if track.track_urn.is_empty() {
                if media_track_urn.take().is_some() {
                    media.set_stopped();
                    media_playing = None;
                }
            } else {
                // Lyrics follow the playing track, looked up once per track per session.
                // Playback never waits for this: the lookup is spawned on the async runtime
                // only after the track has been current for LYRICS_DEBOUNCE, so it starts
                // behind the first audio segment and skipped tracks are never looked up.
                if state.lyrics_track_urn.as_deref() != Some(track.track_urn.as_str()) {
                    state.lyrics_track_urn = Some(track.track_urn.clone());
                    match lyrics_cache.get(&track.track_urn) {
                        Some(Some(l)) => {
                            state.lyrics = LyricsStatus::Found(l.clone());
                            lyrics_pending = None;
                        }
                        Some(None) => {
                            state.lyrics = LyricsStatus::NotFound;
                            lyrics_pending = None;
                        }
                        None => {
                            state.lyrics = LyricsStatus::Loading;
                            lyrics_pending = Some((track.clone(), Instant::now()));
                        }
                    }
                }
                if let Some((pending, since)) = &lyrics_pending
                    && since.elapsed() >= LYRICS_DEBOUNCE
                {
                    let tx = tx.clone();
                    let t = pending.clone();
                    lyrics_pending = None;
                    async_rt.spawn(async move {
                        let urn = t.track_urn.clone();
                        let found = fetch_lyrics(t).await;
                        let _ = tx.send(Msg::Lyrics(urn, found));
                    });
                }

                let changed_track = media_track_urn.as_deref() != Some(track.track_urn.as_str());
                // Artwork is sent once its local copy exists (it arrives after the metadata).
                let cover = last_artwork_file
                    .as_ref()
                    .filter(|(url, path)| *url == track.artwork_url && path.exists())
                    .map(|(_, path)| path.clone());
                if changed_track || media_cover != cover {
                    media.set_track(&track, cover.as_deref());
                    media_track_urn = Some(track.track_urn.clone());
                    media_cover = cover;
                }
                // The OS extrapolates the position itself; re-send it only when the
                // track or play state changes, or after a seek moved it.
                let expected = if media_playing == Some(true) {
                    media_synced_pos + media_synced_at.elapsed().as_millis() as u64
                } else {
                    media_synced_pos
                };
                let drifted = state.progress.abs_diff(expected) > 1500;
                if changed_track || media_playing != Some(playing) || drifted {
                    media.set_playback(playing, state.progress);
                    media_playing = Some(playing);
                    media_synced_at = Instant::now();
                    media_synced_pos = state.progress;
                }
            }
        }

        let view = (
            state.selected_tab,
            state.selected_subtab,
            state.selected_searchfilter,
            state.visualizer_mode,
            state.visualizer_view,
            state.queue_visible,
            state.history_visible,
            state.help_visible,
        );
        if last_view != Some(view) {
            invalidate_screen(&mut terminal);
            last_view = Some(view);
        }
        terminal.draw(|frame| {
            render(
                frame,
                &state,
                &mut data,
                &filtered,
                &player,
                &mut cover_art_async,
                &wave_buffer,
            )
        })?;

        while event::poll(Duration::from_millis(10))? {
            match event::read()? {
                Event::Key(key) => {
                    if let InputOutcome::Quit =
                        handle_key_event(key, &mut state, &mut data, &player)
                    {
                        return Ok(());
                    }
                }
                Event::Resize(_, _) => {
                    picker = Picker::from_query_stdio()?;
                    if let Some(image) = last_artwork_image.as_ref() {
                        let resize_proto = picker.new_resize_protocol(image.clone());
                        cover_art_async =
                            ThreadProtocol::new(tx_worker.clone(), Some(resize_proto));
                    } else {
                        cover_art_async.empty_protocol();
                    }
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            state.progress = player.elapsed();

            let is_playing = player.is_playing();
            if is_playing {
                state.tick += 1.0;
            }

            let current_track = player.current_track();
            if is_playing && !current_track.track_urn.is_empty() {
                let preload_threshold = (current_track.duration_ms as f64 * 0.8) as u64;
                let should_preload = state.progress >= preload_threshold 
                    && state.progress < current_track.duration_ms.saturating_sub(100)
                    && state.preload_triggered_for_track_urn.as_deref() != Some(current_track.track_urn.as_str());
                
                if should_preload {
                    if let Some(current_idx) = state.current_playing_index {
                        let next_track = {
                            let tracks = active_tracks(&state, &data);
                            if state.repeat_enabled {
                                tracks.get(current_idx).cloned()
                            } else if let Some(queued) = state.manual_queue.front() {
                                Some(queued.track.clone())
                            } else if let Some(&next_idx) = state.auto_queue.front() {
                                tracks.get(next_idx).cloned()
                            } else {
                                None
                            }
                        };

                        match next_track {
                            Some(track) => {
                                if track.track_urn != current_track.track_urn && track.is_playable() {
                                    player.preload_next(track);
                                    state.preload_triggered_for_track_urn = Some(current_track.track_urn.clone());
                                }
                            }
                            // The list is running out: hand over to related tracks now so the
                            // radio fetch lands before this track ends.
                            None if state.playback_source != PlaybackSource::Radio => {
                                enter_radio(&mut state, &mut data, current_track.clone());
                            }
                            None => {}
                        }
                    }
                }

                if state.preload_triggered_for_track_urn.as_deref() != Some(current_track.track_urn.as_str()) {
                    state.preload_triggered_for_track_urn = None;
                }

                let at_end = state.progress >= current_track.duration_ms.saturating_sub(50)
                    && current_track.duration_ms > 0;

                if !at_end {
                    state.end_handled_track_urn = None;
                } else if state.end_handled_track_urn.as_deref() != Some(current_track.track_urn.as_str()) {
                    state.end_handled_track_urn = Some(current_track.track_urn.clone());

                    if let Some(current_idx) = state.current_playing_index {
                        if state.repeat_enabled {
                        if let Some(track) = active_tracks(&state, &data).get(current_idx) {
                            player.play(track.clone());
                            state.override_playing = None;
                        }
                        } else {
                            if let Some(queued) = state.manual_queue.pop_front() {
                            if let Some(current) = queued_from_current(&state, &data) {
                                state.playback_history.push(current);
                            }
                                play_queued_track(queued, &mut state, &mut data, &player, true);
                            } else if let Some(next_idx) = state.auto_queue.pop_front() {
                                if let Some(track) = active_tracks(&state, &data).get(next_idx) {
                                    if let Some(current) = queued_from_current(&state, &data) {
                                        state.playback_history.push(current);
                                    }
                                    player.play(track.clone());
                                    state.override_playing = None;
                                    state.current_playing_index = Some(next_idx);
                                }
                            } else {
                                // Nothing queued: related tracks are (or are about to be) on
                                // their way; Msg::Related starts the next one.
                                if state.playback_source != PlaybackSource::Radio {
                                    enter_radio(&mut state, &mut data, current_track.clone());
                                }
                                state.radio_waiting = true;
                            }
                        }
                    }
                }
            } else {
                state.preload_triggered_for_track_urn = None;
            }

            let filter_active = is_filter_active(&state);
            let filtered = build_filtered_views(&state, &data);
            let likes_len = if filter_active && state.selected_subtab == 0 {
                filtered.likes.len()
            } else {
                data.likes.len()
            };
            let playlist_tracks_len = if filter_active && state.selected_subtab == 1 {
                filtered.playlist_tracks.len()
            } else {
                data.playlist_tracks.len()
            };
            let albums_len = if filter_active && state.selected_subtab == 2 {
                filtered.albums.len()
            } else {
                data.albums.len()
            };
            let following_len = if filter_active && state.selected_subtab == 3 {
                filtered.following.len()
            } else {
                data.following.len()
            };

            clamp_selection(
                &mut state,
                &mut data,
                filter_active,
                likes_len,
                playlist_tracks_len,
                albums_len,
                following_len,
            );

            let view = (
                state.selected_tab,
                state.selected_subtab,
                state.selected_searchfilter,
                state.visualizer_mode,
                state.visualizer_view,
                state.queue_visible,
                state.history_visible,
                state.help_visible,
            );
            if last_view != Some(view) {
                invalidate_screen(&mut terminal);
                last_view = Some(view);
            }
            terminal.draw(|frame| {
                render(
                    frame,
                    &state,
                    &mut data,
                    &filtered,
                    &player,
                    &mut cover_art_async,
                    &wave_buffer,
                )
            })?;

            last_tick = Instant::now();

            // Feed is unbounded: only page when the cursor nears the end of what is loaded.
            if state.selected_tab == 2 && state.selected_row + 10 >= data.feed.len() {
                spawn_fetch(Arc::clone(api), tx.clone(), |api| {
                    api.get_activities().map(Msg::Feed)
                });
            }

            match state.selected_subtab {
                0 => spawn_fetch(Arc::clone(api), tx.clone(), |api| {
                    api.get_liked_tracks().map(Msg::Likes)
                }),
                1 => spawn_fetch(Arc::clone(api), tx.clone(), |api| {
                    api.get_playlists().map(Msg::Playlists)
                }),
                2 => spawn_fetch(Arc::clone(api), tx.clone(), |api| api.get_albums().map(Msg::Albums)),
                3 => spawn_fetch(Arc::clone(api), tx.clone(), |api| {
                    api.get_following().map(Msg::Following)
                }),
                _ => {}
            }
        }
    }
}
