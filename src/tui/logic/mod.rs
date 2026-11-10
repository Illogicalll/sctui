pub(crate) mod filtering;
mod input;
pub(crate) mod nav;
pub(crate) mod palette;
mod pager;
pub(crate) mod state;
pub(crate) mod toast;
pub(crate) mod utils;
pub(crate) mod wizard;

use crate::api::{
    Lyrics, fetch_lyrics,
    add_track_to_playlist, create_playlist, delete_playlist, remove_track_from_playlist,
    fetch_related_tracks, fetch_related_users,
    API, Activity, List, MAX_PAGES, Album, Artist, Playlist, Track, engage, fetch_playlist_tracks, has_playable_tracks,
    fetch_search_albums, fetch_search_people, fetch_search_playlists, fetch_search_tracks,
    fetch_user_tracks,
};
use crate::auth::Token;
use crate::player::{Player, TrackChange};
use rand::seq::SliceRandom;
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

use super::render::{NowPlaying, render};
use self::filtering::{FilteredViews, apply_unplayable_filter, build_filtered_views, rebuild_matches_if_active};
use self::nav::PaneId;
use self::pager::Pagers;
use self::input::{InputOutcome, expire_pending, handle_key_event, next_track, prev_track, toggle_play_pause};
use crate::media::{Media, MediaCommand};
use self::state::{AppData, AppState, Engagement, LyricsStatus, PlaybackSource, PlaylistEdit};
use self::utils::{
    enter_radio,
    append_feed_tracks, await_radio, end_of_track, extend_likes_playback, next_track_trigger_ms, peek_next,
    radio_result, radio_seed_due, start_station,
};

/// How long a track must have been playing before its lyrics are looked up.
const LYRICS_DEBOUNCE: Duration = Duration::from_millis(400);
/// How many playlists/albums are asked "anything playable?" at once.
const CONTAINER_PROBES: usize = 6;

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
    /// Related tracks for the radio seed, to append to the queue; `None` when the
    /// request failed.
    Related(u64, Option<Vec<Track>>),
    /// A mix built for an artist station: the seed artist plus a few related artists,
    /// one playable track apiece.
    ArtistStation(u64, Vec<Track>),
    /// A playlist the user just created, to show at the top of the library.
    PlaylistCreated(Playlist),
    /// Lyrics lookup result for a track URN (`None` = nothing found).
    /// `Err` when the lyrics service could not be asked; that is not remembered as a miss.
    Lyrics(String, Result<Option<Lyrics>, String>),
    Engagement(Engagement),
    /// A like or follow the server refused, and why; the optimistic change is undone.
    EngagementFailed(Engagement, String),
    /// A playlist write the server refused; the text is ready to show.
    PlaylistEditFailed(String),
    /// A fetch that failed; the text is ready to show.
    FetchFailed(String),
    /// A page fetch for a list finished: whether more pages remain, or why it failed.
    PageSettled(List, Result<bool, String>),
    /// A playlist or album (by `tracks_uri`) and whether anything in it can be played.
    /// `None` when the check itself failed; nothing is concluded from that.
    ContainerProbed(String, Option<bool>),
}

pub fn run(api: &mut Arc<Mutex<API>>, player: Player, config: crate::config::Loaded, setup: bool) -> anyhow::Result<()> {
    color_eyre::install().map_err(|e| anyhow::anyhow!(e))?;
    let terminal = ratatui::init();
    // Lets terminals that speak the kitty keyboard protocol report Shift+Enter; others still
    // send a plain Enter (S covers them).
    let enhanced_keys = supports_keyboard_enhancement().unwrap_or(false);
    if enhanced_keys {
        let _ = execute!(
            std::io::stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        );
    }
    let result = start(terminal, api, player, config, setup);
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

/// Bring filter matches, cursors and table states in line with the lists as they are
/// now. Run before every draw.
fn settle(state: &mut AppState, data: &mut AppData) -> FilteredViews {
    if !nav::has_nav(state.settings.layout) {
        state.nav_focused = false;
    }
    rebuild_matches_if_active(state, data);
    let views = build_filtered_views(state, data);
    nav::clamp(state, data, &views);
    nav::sync_tables(state, data);
    views
}

/// Fetch the next page of `list` on its own thread, unless `pagers` says not yet:
/// one is already out, the last one failed too recently, or the list has ended.
fn spawn_fetch<F>(pagers: &mut Pagers, list: List, api: &Arc<Mutex<API>>, tx: &Sender<Msg>, fetch_fn: F)
where
    F: FnOnce(&mut API) -> anyhow::Result<Msg> + Send + 'static,
{
    if !pagers.start(list, Instant::now()) {
        return;
    }
    let (api, tx) = (Arc::clone(api), tx.clone());
    std::thread::spawn(move || fetch_and_settle(list, &api, &tx, fetch_fn));
}

/// Run one page fetch and always report how it went: `pagers` holds the list back
/// until its `PageSettled` arrives, so a fetch that panics counts as one that failed.
/// A panic poisons the `api` lock on its way out; the cursors in there only move once
/// a page has been read, so the lock is taken regardless.
fn fetch_and_settle<F>(list: List, api: &Mutex<API>, tx: &Sender<Msg>, fetch_fn: F)
where
    F: FnOnce(&mut API) -> anyhow::Result<Msg>,
{
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut api_guard = api.lock().unwrap_or_else(|e| e.into_inner());
        let result = fetch_fn(&mut api_guard);
        (result, api_guard.has_more(list))
    }));
    let settled = match outcome {
        Ok((Ok(rows), more)) => {
            let _ = tx.send(rows);
            Ok(more)
        }
        Ok((Err(e), _)) => Err(format!("Fetch failed: {e:#}")),
        Err(_) => Err("Fetch failed: internal error".to_string()),
    };
    let _ = tx.send(Msg::PageSettled(list, settled));
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
        let _ = tx.send(match fut.await {
            Ok(tracks) => wrap(request_id, tracks),
            Err(e) => Msg::FetchFailed(format!("Could not load tracks: {e:#}")),
        });
    })
}

/// Up to this many of the seed artist's own tracks go into a station mix.
const ARTIST_STATION_OWN_TRACKS: usize = 15;
/// Total tracks in a station mix.
const ARTIST_STATION_SIZE: usize = 50;
/// How many related artists to pull tracks from, to fill out the rest of the mix.
const ARTIST_STATION_RELATED_ARTISTS: usize = 10;
/// Tracks taken from each related artist's own catalog.
const ARTIST_STATION_TRACKS_PER_RELATED_ARTIST: usize = 3;

/// A 50-track mix for an artist station: up to 15 of the seed artist's own tracks, the rest
/// backfilled from SoundCloud's track-level "related" recommendations plus a few tracks apiece
/// from artists SoundCloud considers similar — the closest a public-API client gets to
/// SoundCloud's own (private-API) "Artist station". Failures for individual sources are
/// swallowed; a partial mix (down to just the seed artist) still beats none.
async fn fetch_artist_station_tracks(token: Arc<Mutex<Token>>, seed_urn: String) -> anyhow::Result<Vec<Track>> {
    // One page of each artist is plenty to pick a handful of tracks from.
    let seed_tracks = fetch_user_tracks(Arc::clone(&token), seed_urn.clone(), "tracks", 1)
        .await
        .unwrap_or_default();

    let mut own_tracks: Vec<Track> = seed_tracks.iter().filter(|t| t.is_playable()).cloned().collect();
    own_tracks.shuffle(&mut rand::thread_rng());
    own_tracks.truncate(ARTIST_STATION_OWN_TRACKS);
    let mut seen: std::collections::HashSet<String> =
        own_tracks.iter().map(|t| t.track_urn.clone()).collect();

    let related_users = fetch_related_users(Arc::clone(&token), seed_urn.clone())
        .await
        .unwrap_or_default();

    let mut fetches = tokio::task::JoinSet::new();
    if let Some(track_urn) = seed_tracks.iter().find(|t| t.is_playable()).map(|t| t.track_urn.clone()) {
        let token = Arc::clone(&token);
        fetches.spawn(async move { fetch_related_tracks(token, track_urn).await.unwrap_or_default() });
    }
    for artist_urn in related_users.into_iter().take(ARTIST_STATION_RELATED_ARTISTS).map(|a| a.urn) {
        let token = Arc::clone(&token);
        fetches.spawn(async move {
            let mut tracks = fetch_user_tracks(token, artist_urn, "tracks", 1).await.unwrap_or_default();
            tracks.retain(Track::is_playable);
            tracks.truncate(ARTIST_STATION_TRACKS_PER_RELATED_ARTIST);
            tracks
        });
    }

    let mut related_pool = Vec::new();
    while let Some(result) = fetches.join_next().await {
        let Ok(tracks) = result else { continue };
        for track in tracks {
            if track.is_playable() && seen.insert(track.track_urn.clone()) {
                related_pool.push(track);
            }
        }
    }
    related_pool.shuffle(&mut rand::thread_rng());
    related_pool.truncate(ARTIST_STATION_SIZE.saturating_sub(own_tracks.len()));

    let mut tracks = own_tracks;
    tracks.extend(related_pool);
    tracks.shuffle(&mut rand::thread_rng());
    Ok(tracks)
}

fn start(
    mut terminal: DefaultTerminal,
    api: &mut Arc<Mutex<API>>,
    player: Player,
    config: crate::config::Loaded,
    setup: bool,
) -> anyhow::Result<()> {
    let mut state = AppState::default();
    state.keymap = config.keymap;
    state.theme_name = config.theme_name;
    state.theme_overrides = config.theme_overrides;
    state.settings = config.settings;
    state.config_load_failed = config.load_failed;
    wizard::on_start(&mut state, config.fresh, setup);
    player.set_crossfade(state.settings.crossfade_ms(), state.settings.crossfade_user_skips);
    state.eq = config.eq;
    crate::player::eq::set(state.eq.gains);

    let mut api_guard = api.lock().unwrap();
    let mut data = AppData::new(&mut api_guard, state.selected_row)?;
    drop(api_guard);
    apply_unplayable_filter(&mut state, &mut data);

    let async_rt = tokio::runtime::Runtime::new().unwrap();
    // Fetch threads hold the `api` lock for whole HTTP requests, so the UI thread must never
    // take it. The token Arc is stable across re-auth (main.rs swaps the token inside
    // it), so clone it once here.
    let token = api.lock().unwrap_or_else(|e| e.into_inner()).token_clone();
    let auth = || Arc::clone(&token);

    let (tx, rx) = mpsc::channel::<Msg>();

    // Playlist writes run one at a time, in the order they were made: add and remove
    // each read the whole track list and write it back, so two at once on the same
    // playlist would lose one of them.
    let (edit_tx, mut edit_rx) = tokio::sync::mpsc::unbounded_channel::<PlaylistEdit>();
    {
        let token = auth();
        let tx = tx.clone();
        async_rt.spawn(async move {
            while let Some(edit) = edit_rx.recv().await {
                let token = Arc::clone(&token);
                let (what, result) = match edit {
                    PlaylistEdit::Add { playlist_id, tracks_uri, track_urn } => (
                        "Adding to playlist",
                        add_track_to_playlist(token, playlist_id, tracks_uri, track_urn).await,
                    ),
                    PlaylistEdit::Remove { playlist_id, tracks_uri, track_urn } => (
                        "Removing from playlist",
                        remove_track_from_playlist(token, playlist_id, tracks_uri, track_urn).await,
                    ),
                    PlaylistEdit::Create { title, track_urn, public } => (
                        "Creating playlist",
                        create_playlist(token, title, track_urn, public).await.map(|playlist| {
                            let _ = tx.send(Msg::PlaylistCreated(playlist));
                        }),
                    ),
                    PlaylistEdit::Delete { playlist_id } => {
                        ("Deleting playlist", delete_playlist(token, playlist_id).await)
                    }
                };
                if let Err(e) = result {
                    let _ = tx.send(Msg::PlaylistEditFailed(format!("{what} failed: {e:#}")));
                }
            }
        });
    }

    let mut pagers = Pagers::default();
    spawn_fetch(&mut pagers, List::Playlists, api, &tx, |api| {
        api.get_playlists().map(Msg::Playlists)
    });
    // Later feed pages are only fetched while the feed tab is on screen, so a hidden
    // feed tab costs nothing beyond this first page - skip that too.
    if !state.settings.hide_feed_tab {
        spawn_fetch(&mut pagers, List::Feed, api, &tx, |api| {
            api.get_activities().map(Msg::Feed)
        });
    }

    // A terminal that does not answer the query gets half-block cover art at a guessed
    // cell size (the size ratatui-image itself assumes when it has to) instead of no app.
    let mut picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::from_fontsize((10, 20)));

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

    let mut hiding_unplayable = state.settings.hide_unplayable;
    loop {
        // Rows that arrived this iteration; the hide-unplayable setting prunes them
        // once here rather than in each of the arms below.
        let mut fetched = false;
        while let Ok(msg) = rx.try_recv() {
            fetched = true;
            match msg {
                Msg::Likes(new) => {
                    for t in &new {
                        data.liked_track_urns.insert(t.track_urn.clone());
                    }
                    extend_likes_playback(&mut state, &mut data, &new);
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
                        append_feed_tracks(&mut data.playback_tracks, &mut state.auto_queue, t, false);
                    }
                }
                Msg::Related(id, tracks) => {
                    if id == state.radio_fetch.request_id
                        && state.playback_source == PlaybackSource::Radio
                        && let Some(track) = radio_result(&mut state, &mut data, tracks, Instant::now())
                    {
                        player.play(track, TrackChange::Natural);
                    }
                }
                Msg::ArtistStation(id, tracks) => {
                    if id != state.artist_station_fetch.request_id {
                        // Superseded by a later request.
                    } else if let Some(seed) = tracks.first().cloned() {
                        player.play(seed, TrackChange::UserSkip);
                        player.resume();
                        start_station(&mut state, &mut data, tracks);
                    } else {
                        state.toast_err("No tracks found for that station");
                    }
                }
                Msg::PlaylistCreated(playlist) => {
                    // Keep the cursor on the same playlist as the list shifts down by one.
                    if !data.playlists.is_empty() {
                        *nav::cursor_mut(&mut state, PaneId::Playlists) += 1;
                    }
                    state.toast(format!("Created {}", playlist.title));
                    data.playlists.insert(0, playlist);
                }
                Msg::Lyrics(urn, found) => {
                    if let Ok(found) = &found {
                        lyrics_cache.insert(urn.clone(), found.clone());
                    }
                    if state.lyrics_track_urn.as_deref() == Some(urn.as_str()) {
                        state.lyrics = match found {
                            Ok(Some(l)) => LyricsStatus::Found(l),
                            Ok(None) => LyricsStatus::NotFound,
                            // Not cached, so the next time this track plays it is looked up again.
                            Err(_) => LyricsStatus::Unavailable,
                        };
                    }
                }
                Msg::Engagement(done) => done.apply(&mut state, &mut data),
                Msg::EngagementFailed(refused, why) => refused.failed(&why, &mut state, &mut data),
                Msg::PlaylistEditFailed(why) | Msg::FetchFailed(why) => state.toast_err(why),
                Msg::PageSettled(list, outcome) => {
                    // One notice per run of failures; the retries after it are silent.
                    let why = outcome.as_ref().err().cloned();
                    if pagers.settled(list, outcome.ok(), Instant::now())
                        && let Some(why) = why
                    {
                        state.toast_err(why);
                    }
                }
                Msg::ContainerProbed(uri, playable) => {
                    data.containers.probing = data.containers.probing.saturating_sub(1);
                    if playable == Some(false) {
                        data.containers.none_playable.insert(uri);
                    }
                }
            }
        }
        // A fresh token that could not be written to disk (it is still the one in use).
        if let Some(warning) = crate::auth::take_save_warning() {
            state.toast_err(warning);
        }
        // New rows, or the setting changed (from the settings page or the palette).
        if fetched || hiding_unplayable != state.settings.hide_unplayable {
            hiding_unplayable = state.settings.hide_unplayable;
            apply_unplayable_filter(&mut state, &mut data);
        }

        // With "hide unplayable tracks" on, ask about each listed playlist and album once
        // per session, a few at a time, so the ones with nothing playable can be dropped.
        if state.settings.hide_unplayable && data.containers.probing < CONTAINER_PROBES {
            let room = CONTAINER_PROBES - data.containers.probing;
            let next: Vec<String> = filtering::containers_to_probe(&data).take(room).map(str::to_string).collect();
            for uri in next {
                data.containers.probed.insert(uri.clone());
                data.containers.probing += 1;
                let (token, tx) = (auth(), tx.clone());
                async_rt.spawn(async move {
                    let playable = has_playable_tracks(token, uri.clone()).await.ok();
                    let _ = tx.send(Msg::ContainerProbed(uri, playable));
                });
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
                let _ = tx.send(match engage(token, method, path).await {
                    Ok(()) => Msg::Engagement(action),
                    Err(e) => Msg::EngagementFailed(action, format!("{e:#}")),
                });
            });
        }

        while let Some(edit) = state.playlist_edit_queue.pop_front() {
            let _ = edit_tx.send(edit);
        }

        let current_artwork_url = player.current_track().artwork_url;
        while let Ok(app_ev) = rx_main.try_recv() {
            match app_ev {
                AppEvent::Redraw(completed) => {
                    // An image that could not be encoded is simply not shown: the
                    // protocol gave its image up for encoding and stays empty.
                    if let Ok(completed) = completed {
                        let _ = cover_art_async.update_resized_protocol(completed);
                    }
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

        let filtered = settle(&mut state, &mut data);

        if state.selected_tab == 0 && state.selected_subtab == 1 {
            let selected = nav::selected_index(&state, PaneId::Playlists).and_then(|i| data.playlists.get(i)).map(|p| p.tracks_uri.clone());
            state.playlist_tracks_fetch.refetch(
                PaneId::PlaylistTracks,
                &mut state.child_cursors,
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
            let selected = nav::selected_index(&state, PaneId::Albums).and_then(|i| data.albums.get(i)).map(|a| a.tracks_uri.clone());
            state.album_tracks_fetch.refetch(
                PaneId::AlbumTracks,
                &mut state.child_cursors,
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
            let selected = nav::selected_index(&state, PaneId::Following).and_then(|i| data.following.get(i)).map(|a| a.urn.clone());
            state.following_tracks_fetch.refetch(
                PaneId::FollowingTracks,
                &mut state.child_cursors,
                &mut data.following_tracks,
                &mut data.following_tracks_state,
                &mut data.following_tracks_user_urn,
                &mut state.selected_following_track_row,
                selected.clone(),
                |id, urn| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_user_tracks(auth(), urn, "tracks", MAX_PAGES),
                    Msg::FollowingTracks,
                ),
            );
            state.following_likes_fetch.refetch(
                PaneId::FollowingLikes,
                &mut state.child_cursors,
                &mut data.following_likes_tracks,
                &mut data.following_likes_state,
                &mut data.following_likes_user_urn,
                &mut state.selected_following_like_row,
                selected,
                |id, urn| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_user_tracks(auth(), urn, "likes/tracks", MAX_PAGES),
                    Msg::FollowingLikes,
                ),
            );
        }

        if state.selected_tab == 2 {
            let selected = nav::selected_index(&state, PaneId::Feed).and_then(|i| data.feed.get(i));
            let single = selected.and_then(|a| a.track.clone());
            let key = selected.map(|a| a.key.clone());
            state.feed_tracks_fetch.refetch(
                PaneId::FeedTracks,
                &mut state.child_cursors,
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

        // Radio: keep related tracks queued behind whatever is playing. A failure comes
        // back as `None` (not a toast) so `radio_result` can retry it.
        if let Some(seed) = radio_seed_due(&state, &data, Instant::now()) {
            state.radio_fetched_for = Some(seed.clone());
            state.radio_fetch.cancel();
            let id = state.radio_fetch.request_id;
            let (token, tx) = (auth(), tx.clone());
            state.radio_fetch.task = Some(async_rt.spawn(async move {
                let _ = tx.send(Msg::Related(id, fetch_related_tracks(token, seed).await.ok()));
            }));
        }

        // Shift+Enter on a focused artist row: fetch its station mix in the background, then
        // Msg::ArtistStation starts playback once it lands.
        if let Some(artist) = state.artist_station_request.take() {
            state.artist_station_fetch.cancel();
            let id = state.artist_station_fetch.request_id;
            state.artist_station_fetch.task = Some(spawn_tracks(
                &async_rt, &tx, id,
                fetch_artist_station_tracks(auth(), artist.urn),
                Msg::ArtistStation,
            ));
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

            nav::reset_search(&mut state);

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
                    let found = match filter {
                        0 => fetch_search_tracks(token, query).await.map(|t| Msg::SearchTracks(request_id, t)),
                        1 => fetch_search_albums(token, query).await.map(|a| Msg::SearchAlbums(request_id, a)),
                        2 => fetch_search_playlists(token, query).await.map(|p| Msg::SearchPlaylists(request_id, p)),
                        3 => fetch_search_people(token, query).await.map(|p| Msg::SearchPeople(request_id, p)),
                        _ => return,
                    };
                    let _ = tx.send(found.unwrap_or_else(|e| Msg::FetchFailed(format!("Search failed: {e:#}"))));
                }));
            }
        }

        if state.selected_tab == 1 && state.selected_searchfilter == 2 {
            let selected = nav::selected_index(&state, PaneId::SearchPlaylists).and_then(|i| data.search_playlists.get(i)).map(|p| p.tracks_uri.clone());
            state.search_playlist_tracks_fetch.refetch(
                PaneId::SearchPlaylistTracks,
                &mut state.child_cursors,
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
            let selected = nav::selected_index(&state, PaneId::SearchAlbums).and_then(|i| data.search_albums.get(i)).map(|a| a.tracks_uri.clone());
            state.search_album_tracks_fetch.refetch(
                PaneId::SearchAlbumTracks,
                &mut state.child_cursors,
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
            let selected = nav::selected_index(&state, PaneId::SearchPeople).and_then(|i| data.search_people.get(i)).map(|a| a.urn.clone());
            state.search_people_tracks_fetch.refetch(
                PaneId::SearchPeopleTracks,
                &mut state.child_cursors,
                &mut data.search_people_tracks,
                &mut data.search_people_tracks_state,
                &mut data.search_people_tracks_user_urn,
                &mut state.search_selected_person_track_row,
                selected.clone(),
                |id, urn| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_user_tracks(auth(), urn, "tracks", MAX_PAGES),
                    Msg::SearchPeopleTracks,
                ),
            );
            state.search_people_likes_fetch.refetch(
                PaneId::SearchPeopleLikes,
                &mut state.child_cursors,
                &mut data.search_people_likes_tracks,
                &mut data.search_people_likes_state,
                &mut data.search_people_likes_user_urn,
                &mut state.search_selected_person_like_row,
                selected,
                |id, urn| spawn_tracks(
                    &async_rt, &tx, id,
                    fetch_user_tracks(auth(), urn, "likes/tracks", MAX_PAGES),
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
        if let Some(failure) = player.take_load_failure() {
            state.toast_err(format!("Couldn't play {}", failure.track.title));
            // Move on to the next track, unless several in a row have failed
            // (offline, most likely): then stop instead of emptying the queue.
            if !failure.give_up {
                next_track(&mut state, &mut data, &player);
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
            // A layout change moves the cover art; so does each wizard screen.
            state.settings.layout,
            state.wizard.as_ref().map(|w| w.stage),
        );
        if last_view != Some(view) {
            invalidate_screen(&mut terminal);
            last_view = Some(view);
        }
        expire_pending(&mut state);
        toast::expire(&mut state.toasts, Instant::now());
        nav::sync_tables(&state, &mut data);
        let now_playing = NowPlaying::from_player(&player);
        terminal.draw(|frame| {
            render(
                frame,
                &mut state,
                &mut data,
                &filtered,
                &now_playing,
                Some(&mut cover_art_async),
                &wave_buffer,
            )
        })?;

        let mut resized = false;
        while event::poll(Duration::from_millis(10))? {
            match event::read()? {
                Event::Key(key) => {
                    if let InputOutcome::Quit =
                        handle_key_event(key, &mut state, &mut data, &player)
                    {
                        return Ok(());
                    }
                }
                Event::Resize(_, _) => resized = true,
                _ => {}
            }
        }
        // Once per burst of resize events, and only after the pending input has been
        // read: the query's answer comes in on stdin too. The font size can change
        // with the window, so asking again is right, but a terminal that does not
        // answer just leaves the picker as it was.
        if resized {
            if let Ok(queried) = Picker::from_query_stdio() {
                picker = queried;
            }
            if let Some(image) = last_artwork_image.as_ref() {
                let resize_proto = picker.new_resize_protocol(image.clone());
                cover_art_async =
                    ThreadProtocol::new(tx_worker.clone(), Some(resize_proto));
            } else {
                cover_art_async.empty_protocol();
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
                // Repeat replays the same track, which the player still reads as a
                // seek (same urn) rather than a track change, so there is nothing to
                // overlap even though the intent now reaches the engine.
                // ponytail: repeat would need a second decode of the same track to
                // fade into itself; do that only if anyone asks for it.
                let crossfade_ms = if state.repeat_enabled { 0 } else { state.settings.crossfade_ms() };
                let handover_ms = next_track_trigger_ms(current_track.duration_ms, crossfade_ms);
                // Whichever comes first: the usual four fifths in, or long enough
                // before the handover that a crossfade has the next track ready.
                let preload_threshold = ((current_track.duration_ms as f64 * 0.8) as u64)
                    .min(handover_ms.saturating_sub(10_000));
                let should_preload = state.progress >= preload_threshold 
                    && state.progress < current_track.duration_ms.saturating_sub(100)
                    && state.preload_triggered_for_track_urn.as_deref() != Some(current_track.track_urn.as_str());
                
                if should_preload
                    && state.current_playing_index.is_some() {
                        match peek_next(&state, &data) {
                            Some(track) => {
                                if track.track_urn != current_track.track_urn && track.is_playable() {
                                    player.preload_next(track);
                                    state.preload_triggered_for_track_urn = Some(current_track.track_urn.clone());
                                }
                            }
                            // The list is running out: hand over to related tracks now so the
                            // radio fetch lands before this track ends.
                            None if state.playback_source != PlaybackSource::Radio => {
                                enter_radio(&mut state, &mut data, vec![current_track.clone()]);
                            }
                            None => {}
                        }
                    }

                if state.preload_triggered_for_track_urn.as_deref() != Some(current_track.track_urn.as_str()) {
                    state.preload_triggered_for_track_urn = None;
                }

                // The clock reaching the handover point, or the stream itself running
                // out first (a preview, or a track the network gave up on).
                let at_end = (state.progress >= handover_ms || player.track_ended())
                    && current_track.duration_ms > 0;

                if !at_end {
                    state.end_handled_track_urn = None;
                } else if state.end_handled_track_urn.as_deref() != Some(current_track.track_urn.as_str()) {
                    state.end_handled_track_urn = Some(current_track.track_urn.clone());

                    if state.current_playing_index.is_some() {
                        if let Some(track) = end_of_track(&mut state, &data) {
                            player.play(track, TrackChange::Natural);
                        } else {
                            // Nothing queued: related tracks are (or are about to be) on
                            // their way; Msg::Related starts the next one.
                            if state.playback_source != PlaybackSource::Radio {
                                enter_radio(&mut state, &mut data, vec![current_track.clone()]);
                            }
                            await_radio(&mut state);
                        }
                    }
                }
            } else {
                state.preload_triggered_for_track_urn = None;
            }

            let filtered = settle(&mut state, &mut data);

            let view = (
                state.selected_tab,
                state.selected_subtab,
                state.selected_searchfilter,
                state.visualizer_mode,
                state.visualizer_view,
                state.queue_visible,
                state.history_visible,
                state.help_visible,
                state.settings.layout,
                state.wizard.as_ref().map(|w| w.stage),
            );
            if last_view != Some(view) {
                invalidate_screen(&mut terminal);
                last_view = Some(view);
            }
            let now_playing = NowPlaying::from_player(&player);
            terminal.draw(|frame| {
                render(
                    frame,
                    &mut state,
                    &mut data,
                    &filtered,
                    &now_playing,
                    Some(&mut cover_art_async),
                    &wave_buffer,
                )
            })?;

            last_tick = Instant::now();

            // Feed is unbounded: only page when the cursor nears the end of what is loaded.
            if state.selected_tab == 2 && nav::cursor(&state, PaneId::Feed) + 10 >= data.feed.len() {
                spawn_fetch(&mut pagers, List::Feed, api, &tx, |api| {
                    api.get_activities().map(Msg::Feed)
                });
            }

            match state.selected_subtab {
                0 => spawn_fetch(&mut pagers, List::Likes, api, &tx, |api| {
                    api.get_liked_tracks().map(Msg::Likes)
                }),
                1 => spawn_fetch(&mut pagers, List::Playlists, api, &tx, |api| {
                    api.get_playlists().map(Msg::Playlists)
                }),
                2 => spawn_fetch(&mut pagers, List::Albums, api, &tx, |api| api.get_albums().map(Msg::Albums)),
                3 => spawn_fetch(&mut pagers, List::Following, api, &tx, |api| {
                    api.get_following().map(Msg::Following)
                }),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S5: a fetch that panics still settles its pager (as a failure, so it backs off
    /// and retries), and the lock it poisoned does not take the next fetch down too.
    #[test]
    fn a_panicking_fetch_settles_as_failed_and_the_next_one_still_runs() {
        let token = Token { access_token: String::new(), refresh_token: String::new(), obtained_at: 0 };
        let api = Mutex::new(API::init(Arc::new(Mutex::new(token))));
        let (tx, rx) = mpsc::channel();
        let now = Instant::now();
        let mut pagers = Pagers::default();

        assert!(pagers.start(List::Likes, now));
        fetch_and_settle(List::Likes, &api, &tx, |_| panic!("fetch blew up (expected by this test)"));
        let Ok(Msg::PageSettled(List::Likes, Err(why))) = rx.try_recv() else { panic!("no failed settle") };
        assert!(why.starts_with("Fetch failed"));
        assert!(api.is_poisoned());
        assert!(pagers.settled(List::Likes, None, now), "announced like any first failure");
        assert!(!pagers.start(List::Likes, now), "backing off");
        assert!(pagers.start(List::Likes, now + Duration::from_secs(2)), "then retried");

        fetch_and_settle(List::Likes, &api, &tx, |_| Ok(Msg::Likes(Vec::new())));
        assert!(matches!(rx.try_recv(), Ok(Msg::Likes(_))));
        assert!(matches!(rx.try_recv(), Ok(Msg::PageSettled(List::Likes, Ok(true)))));
    }
}
