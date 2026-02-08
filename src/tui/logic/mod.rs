mod filtering;
mod input;
mod animation;
pub(crate) mod state;
mod utils;

use crate::api::{
    API, fetch_album_tracks, fetch_following_liked_tracks, fetch_following_tracks,
    fetch_playlist_tracks,
};
use crate::player::Player;
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event},
};

use std::result::Result::Ok;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ratatui_image::{
    errors::Errors,
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
};
use reqwest;
use image::DynamicImage;

use super::render::render;
use self::filtering::{build_filtered_views, clamp_selection, is_filter_active};
use self::input::{handle_key_event, InputOutcome};
use self::animation::{SinSignal, on_tick};
use self::state::{AppData, AppState, FollowingTracksFocus, PlaybackSource};
use self::utils::{build_queue, play_queued_track, queued_from_current};

const TAB_TITLES: [&str; 3] = ["Library", "Search", "Feed"];
const SUBTAB_TITLES: [&str; 4] = ["Likes", "Playlists", "Albums", "Following"];
const SEARCHFILTERS: [&str; 4] = ["Tracks", "Albums", "Playlists", "People"];

enum AppEvent {
    Redraw(Result<ResizeResponse, Errors>),
}

pub fn run(api: &mut Arc<Mutex<API>>, player: Player) -> anyhow::Result<()> {
    color_eyre::install().map_err(|e| anyhow::anyhow!(e))?;
    let terminal = ratatui::init();
    let result = start(terminal, api, player);
    ratatui::restore();
    result
}

fn spawn_fetch<T, F>(api: Arc<Mutex<API>>, tx: std::sync::mpsc::Sender<T>, fetch_fn: F)
where
    T: Send + 'static,
    F: FnOnce(&mut API) -> anyhow::Result<T> + Send + 'static,
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

fn start(
    mut terminal: DefaultTerminal,
    api: &mut Arc<Mutex<API>>,
    player: Player,
) -> anyhow::Result<()> {
    let mut state = AppState::new();

    let mut api_guard = api.lock().unwrap();
    let mut data = AppData::new(&mut api_guard, state.selected_row)?;
    drop(api_guard);

    let mut signal = SinSignal::new(0.1, 2.0, 10.0);
    let mut data_points = signal.by_ref().take(200).collect::<Vec<(f64, f64)>>();
    let mut window = [0.0, 20.0];
    let async_rt = tokio::runtime::Runtime::new().unwrap();

    let (tx_likes, rx_likes): (Sender<Vec<crate::api::Track>>, Receiver<Vec<crate::api::Track>>) =
        mpsc::channel();
    let (tx_playlists, rx_playlists): (
        Sender<Vec<crate::api::Playlist>>,
        Receiver<Vec<crate::api::Playlist>>,
    ) = mpsc::channel();
    let (tx_playlist_tracks, rx_playlist_tracks): (
        Sender<(u64, Vec<crate::api::Track>)>,
        Receiver<(u64, Vec<crate::api::Track>)>,
    ) = mpsc::channel();
    let (tx_album_tracks, rx_album_tracks): (
        Sender<(u64, Vec<crate::api::Track>)>,
        Receiver<(u64, Vec<crate::api::Track>)>,
    ) = mpsc::channel();
    let (tx_albums, rx_albums): (Sender<Vec<crate::api::Album>>, Receiver<Vec<crate::api::Album>>) =
        mpsc::channel();
    let (tx_following, rx_following): (
        Sender<Vec<crate::api::Artist>>,
        Receiver<Vec<crate::api::Artist>>,
    ) = mpsc::channel();
    let (tx_following_tracks, rx_following_tracks): (
        Sender<(u64, Vec<crate::api::Track>)>,
        Receiver<(u64, Vec<crate::api::Track>)>,
    ) = mpsc::channel();
    let (tx_following_likes, rx_following_likes): (
        Sender<(u64, Vec<crate::api::Track>)>,
        Receiver<(u64, Vec<crate::api::Track>)>,
    ) = mpsc::channel();

    spawn_fetch(Arc::clone(api), tx_playlists.clone(), |api| {
        api.get_playlists()
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
    let mut last_artwork_image: Option<DynamicImage> = None;

    let wave_buffer = player.wave_buffer();
    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        data.apply_updates(
            &rx_likes,
            &rx_playlists,
            &rx_playlist_tracks,
            &rx_album_tracks,
            &rx_following_tracks,
            &rx_following_likes,
            &rx_albums,
            &rx_following,
            state.playlist_tracks_request_id,
            state.album_tracks_request_id,
            state.following_tracks_request_id,
            state.following_likes_request_id,
        );

        while let Ok(app_ev) = rx_main.try_recv() {
            match app_ev {
                AppEvent::Redraw(completed) => {
                    let _ = cover_art_async.update_resized_protocol(completed?);
                }
            }
        }

        let url = &player.current_track().artwork_url;
        let should_update = match &last_artwork_url {
            Some(prev) => prev != url,
            None => true,
        };

        if should_update {
            if let Ok(resp) = reqwest::blocking::get(url.as_str()) {
                if let Ok(bytes) = resp.bytes() {
                    if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                        let resize_proto = picker.new_resize_protocol(dyn_img.clone());
                        cover_art_async =
                            ThreadProtocol::new(tx_worker.clone(), Some(resize_proto));

                        last_artwork_url = Some(url.clone());
                        last_artwork_image = Some(dyn_img);
                    }
                }
            }
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

        let likes_ref = if filter_active && state.selected_subtab == 0 {
            &filtered.likes
        } else {
            &data.likes
        };
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
            if let Some(selected_playlist) = data.playlists.get(state.selected_row) {
                let tracks_uri = selected_playlist.tracks_uri.clone();
                let needs_fetch = data
                    .playlist_tracks_uri
                    .as_deref()
                    .map(|uri| uri != tracks_uri.as_str())
                    .unwrap_or(true);
                if needs_fetch {
                    if let Some(handle) = state.playlist_tracks_task.take() {
                        handle.abort();
                    }
                    state.playlist_tracks_request_id =
                        state.playlist_tracks_request_id.wrapping_add(1);
                    let request_id = state.playlist_tracks_request_id;
                    let token = {
                        let api_guard = api.lock().unwrap();
                        api_guard.token_clone()
                    };
                    data.playlist_tracks_uri = Some(tracks_uri.clone());
                    data.playlist_tracks.clear();
                    data.playlist_tracks_state.select(Some(0));
                    state.selected_playlist_track_row = 0;
                    let tx = tx_playlist_tracks.clone();
                    state.playlist_tracks_task = Some(async_rt.spawn(async move {
                        if let Ok(tracks) = fetch_playlist_tracks(token, tracks_uri).await {
                            let _ = tx.send((request_id, tracks));
                        }
                    }));
                }
            } else {
                data.playlist_tracks.clear();
                data.playlist_tracks_state.select(Some(0));
                data.playlist_tracks_uri = None;
                state.selected_playlist_track_row = 0;
            }

            if !data.playlist_tracks.is_empty()
                && state.selected_playlist_track_row >= data.playlist_tracks.len()
            {
                state.selected_playlist_track_row = data.playlist_tracks.len() - 1;
                data.playlist_tracks_state
                    .select(Some(state.selected_playlist_track_row));
            }
        }

        let playlists_ref = &data.playlists;
        let playlist_tracks_ref = if filter_active && state.selected_subtab == 1 {
            &filtered.playlist_tracks
        } else {
            &data.playlist_tracks
        };

        if state.selected_tab == 0 && state.selected_subtab == 2 {
            if let Some(selected_album) = albums_ref.get(state.selected_row) {
                let tracks_uri = selected_album.tracks_uri.clone();
                let needs_fetch = data
                    .album_tracks_uri
                    .as_deref()
                    .map(|uri| uri != tracks_uri.as_str())
                    .unwrap_or(true);
                if needs_fetch {
                    if let Some(handle) = state.album_tracks_task.take() {
                        handle.abort();
                    }
                    state.album_tracks_request_id =
                        state.album_tracks_request_id.wrapping_add(1);
                    let request_id = state.album_tracks_request_id;
                    let token = {
                        let api_guard = api.lock().unwrap();
                        api_guard.token_clone()
                    };
                    data.album_tracks_uri = Some(tracks_uri.clone());
                    data.album_tracks.clear();
                    data.album_tracks_state.select(Some(0));
                    state.selected_album_track_row = 0;
                    let tx = tx_album_tracks.clone();
                    state.album_tracks_task = Some(async_rt.spawn(async move {
                        if let Ok(tracks) = fetch_album_tracks(token, tracks_uri).await {
                            let _ = tx.send((request_id, tracks));
                        }
                    }));
                }
            } else {
                data.album_tracks.clear();
                data.album_tracks_state.select(Some(0));
                data.album_tracks_uri = None;
                state.selected_album_track_row = 0;
            }

            if !data.album_tracks.is_empty()
                && state.selected_album_track_row >= data.album_tracks.len()
            {
                state.selected_album_track_row = data.album_tracks.len() - 1;
                data.album_tracks_state
                    .select(Some(state.selected_album_track_row));
            }
        }

        if state.selected_tab == 0 && state.selected_subtab == 3 {
            if let Some(selected_artist) = following_ref.get(state.selected_row) {
                let user_urn = selected_artist.urn.clone();
                let user_urn_for_tracks = user_urn.clone();
                let user_urn_for_likes = user_urn.clone();

                let needs_tracks = data
                    .following_tracks_user_urn
                    .as_deref()
                    .map(|urn| urn != user_urn.as_str())
                    .unwrap_or(true);
                if needs_tracks {
                    if let Some(handle) = state.following_tracks_task.take() {
                        handle.abort();
                    }
                    state.following_tracks_request_id =
                        state.following_tracks_request_id.wrapping_add(1);
                    let request_id = state.following_tracks_request_id;
                    let token = {
                        let api_guard = api.lock().unwrap();
                        api_guard.token_clone()
                    };
                    data.following_tracks_user_urn = Some(user_urn.clone());
                    data.following_tracks.clear();
                    data.following_tracks_state.select(Some(0));
                    state.selected_following_track_row = 0;
                    state.following_tracks_focus = FollowingTracksFocus::Published;
                    let tx = tx_following_tracks.clone();
                    state.following_tracks_task = Some(async_rt.spawn(async move {
                        if let Ok(tracks) = fetch_following_tracks(token, user_urn_for_tracks).await {
                            let _ = tx.send((request_id, tracks));
                        }
                    }));
                }

                let needs_likes = data
                    .following_likes_user_urn
                    .as_deref()
                    .map(|urn| urn != user_urn.as_str())
                    .unwrap_or(true);
                if needs_likes {
                    if let Some(handle) = state.following_likes_task.take() {
                        handle.abort();
                    }
                    state.following_likes_request_id =
                        state.following_likes_request_id.wrapping_add(1);
                    let request_id = state.following_likes_request_id;
                    let token = {
                        let api_guard = api.lock().unwrap();
                        api_guard.token_clone()
                    };
                    data.following_likes_user_urn = Some(user_urn.clone());
                    data.following_likes_tracks.clear();
                    data.following_likes_state.select(Some(0));
                    state.selected_following_like_row = 0;
                    let tx = tx_following_likes.clone();
                    state.following_likes_task = Some(async_rt.spawn(async move {
                        if let Ok(tracks) = fetch_following_liked_tracks(token, user_urn_for_likes).await {
                            let _ = tx.send((request_id, tracks));
                        }
                    }));
                }
            } else {
                data.following_tracks.clear();
                data.following_tracks_state.select(Some(0));
                data.following_tracks_user_urn = None;
                data.following_likes_tracks.clear();
                data.following_likes_state.select(Some(0));
                data.following_likes_user_urn = None;
                state.selected_following_track_row = 0;
                state.selected_following_like_row = 0;
                state.following_tracks_focus = FollowingTracksFocus::Published;
            }

            if !data.following_tracks.is_empty()
                && state.selected_following_track_row >= data.following_tracks.len()
            {
                state.selected_following_track_row = data.following_tracks.len() - 1;
                data.following_tracks_state
                    .select(Some(state.selected_following_track_row));
            }
            if !data.following_likes_tracks.is_empty()
                && state.selected_following_like_row >= data.following_likes_tracks.len()
            {
                state.selected_following_like_row = data.following_likes_tracks.len() - 1;
                data.following_likes_state
                    .select(Some(state.selected_following_like_row));
            }
        }
        let queue_tracks = match state.playback_source {
            PlaybackSource::Likes => &data.likes,
            PlaybackSource::Playlist
            | PlaybackSource::Album
            | PlaybackSource::FollowingPublished
            | PlaybackSource::FollowingLikes => &data.playback_tracks,
        };
        let previous_playing_track = state
            .playback_history
            .last()
            .map(|queued| queued.track.clone());
        let current_playing_track = state
            .override_playing
            .as_ref()
            .map(|queued| queued.track.clone())
            .or_else(|| {
                state
                    .current_playing_index
                    .and_then(|idx| queue_tracks.get(idx).cloned())
            });
        terminal.draw(|frame| {
            render(
                frame,
                likes_ref,
                queue_tracks,
                &mut data.likes_state,
                playlists_ref,
                &mut data.playlists_state,
                playlist_tracks_ref,
                &mut data.playlist_tracks_state,
                &data.album_tracks,
                &mut data.album_tracks_state,
                albums_ref,
                &mut data.albums_state,
                following_ref,
                &mut data.following_state,
                &data.following_tracks,
                &mut data.following_tracks_state,
                &data.following_likes_tracks,
                &mut data.following_likes_state,
                state.selected_tab,
                &TAB_TITLES,
                state.selected_subtab,
                &SUBTAB_TITLES,
                state.selected_row,
                state.selected_playlist_track_row,
                state.selected_album_track_row,
                state.selected_following_track_row,
                state.selected_following_like_row,
                state.following_tracks_focus == FollowingTracksFocus::Likes,
                &state.query,
                &SEARCHFILTERS,
                state.selected_searchfilter,
                state.info_pane_selected,
                state.selected_info_row,
                &mut data_points,
                &mut window,
                &mut state.progress,
                player.current_track(),
                &mut cover_art_async,
                player.get_volume(),
                state.shuffle_enabled,
                state.repeat_enabled,
                state.queue_visible,
                &state.manual_queue,
                &state.auto_queue,
                current_playing_track.clone(),
                previous_playing_track.clone(),
                state.help_visible,
                state.quit_confirm_visible,
                state.quit_confirm_selected,
                state.search_popup_visible,
                &state.search_query,
                state.search_matches.len(),
                state.visualizer_mode,
                &wave_buffer,
                state.visualizer_view,
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
                on_tick(&mut data_points, &mut window, &mut signal);
            }

            let current_track = player.current_track();
            if is_playing && !current_track.track_urn.is_empty() {
                let preload_threshold = (current_track.duration_ms as f64 * 0.8) as u64;
                let should_preload = state.progress >= preload_threshold 
                    && state.progress < current_track.duration_ms.saturating_sub(100)
                    && state.preload_triggered_for_track_urn.as_deref() != Some(current_track.track_urn.as_str());
                
                if should_preload {
                    if let Some(current_idx) = state.current_playing_index {
                        let active_tracks = match state.playback_source {
                            PlaybackSource::Likes => &data.likes,
                            PlaybackSource::Playlist
                            | PlaybackSource::Album
                            | PlaybackSource::FollowingPublished
                            | PlaybackSource::FollowingLikes => &data.playback_tracks,
                        };

                        let next_track = if state.repeat_enabled {
                            active_tracks.get(current_idx).cloned()
                        } else if let Some(queued) = state.manual_queue.front() {
                            Some(queued.track.clone())
                        } else if let Some(&next_idx) = state.auto_queue.front() {
                            active_tracks.get(next_idx).cloned()
                        } else {
                            if state.auto_queue.is_empty() {
                                state.auto_queue = build_queue(current_idx, active_tracks, state.shuffle_enabled);
                            }
                            state.auto_queue.front().and_then(|&idx| active_tracks.get(idx).cloned())
                        };

                        if let Some(track) = next_track {
                            if track.track_urn != current_track.track_urn && track.is_playable() {
                                player.preload_next(track);
                                state.preload_triggered_for_track_urn = Some(current_track.track_urn.clone());
                            }
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
                        let active_tracks = match state.playback_source {
                            PlaybackSource::Likes => &data.likes,
                            PlaybackSource::Playlist
                            | PlaybackSource::Album
                            | PlaybackSource::FollowingPublished
                            | PlaybackSource::FollowingLikes => &data.playback_tracks,
                        };
                        if let Some(track) = active_tracks.get(current_idx) {
                            player.play(track.clone());
                            state.override_playing = None;
                        }
                        } else {
                            if state.manual_queue.is_empty() && state.auto_queue.is_empty() {
                            let active_tracks = match state.playback_source {
                                PlaybackSource::Likes => &data.likes,
                                PlaybackSource::Playlist
                                | PlaybackSource::Album
                                | PlaybackSource::FollowingPublished
                                | PlaybackSource::FollowingLikes => &data.playback_tracks,
                            };
                                state.auto_queue =
                                    build_queue(current_idx, active_tracks, state.shuffle_enabled);
                            }
                            if let Some(queued) = state.manual_queue.pop_front() {
                            if let Some(current) = queued_from_current(&state, &data) {
                                state.playback_history.push(current);
                            }
                                play_queued_track(queued, &mut state, &mut data, &player, true);
                            } else if let Some(next_idx) = state.auto_queue.pop_front() {
                                let active_tracks = match state.playback_source {
                                PlaybackSource::Likes => &data.likes,
                                PlaybackSource::Playlist
                                | PlaybackSource::Album
                                | PlaybackSource::FollowingPublished
                                | PlaybackSource::FollowingLikes => &data.playback_tracks,
                                };
                                if let Some(track) = active_tracks.get(next_idx) {
                                    if let Some(current) = queued_from_current(&state, &data) {
                                        state.playback_history.push(current);
                                    }
                                    player.play(track.clone());
                                    state.override_playing = None;
                                    state.current_playing_index = Some(next_idx);
                                }
                            } else {
                                player.pause();
                                state.current_playing_index = None;
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

            let likes_ref = if filter_active && state.selected_subtab == 0 {
                &filtered.likes
            } else {
                &data.likes
            };
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

            let playlists_ref = &data.playlists;
            let playlist_tracks_ref = if filter_active && state.selected_subtab == 1 {
                &filtered.playlist_tracks
            } else {
                &data.playlist_tracks
            };

            let queue_tracks = match state.playback_source {
                PlaybackSource::Likes => &data.likes,
                PlaybackSource::Playlist
                | PlaybackSource::Album
                | PlaybackSource::FollowingPublished
                | PlaybackSource::FollowingLikes => &data.playback_tracks,
            };
            let previous_playing_track = state
                .playback_history
                .last()
                .map(|queued| queued.track.clone());
            let current_playing_track = state
                .override_playing
                .as_ref()
                .map(|queued| queued.track.clone())
                .or_else(|| {
                    state
                        .current_playing_index
                        .and_then(|idx| queue_tracks.get(idx).cloned())
                });
            terminal.draw(|frame| {
                render(
                    frame,
                    likes_ref,
                    queue_tracks,
                    &mut data.likes_state,
                    playlists_ref,
                    &mut data.playlists_state,
                    playlist_tracks_ref,
                    &mut data.playlist_tracks_state,
                    &data.album_tracks,
                    &mut data.album_tracks_state,
                    albums_ref,
                    &mut data.albums_state,
                    following_ref,
                    &mut data.following_state,
                    &data.following_tracks,
                    &mut data.following_tracks_state,
                    &data.following_likes_tracks,
                    &mut data.following_likes_state,
                    state.selected_tab,
                    &TAB_TITLES,
                    state.selected_subtab,
                    &SUBTAB_TITLES,
                    state.selected_row,
                    state.selected_playlist_track_row,
                    state.selected_album_track_row,
                    state.selected_following_track_row,
                    state.selected_following_like_row,
                    state.following_tracks_focus == FollowingTracksFocus::Likes,
                    &state.query,
                    &SEARCHFILTERS,
                    state.selected_searchfilter,
                    state.info_pane_selected,
                    state.selected_info_row,
                    &mut data_points,
                    &mut window,
                    &mut state.progress,
                    player.current_track(),
                    &mut cover_art_async,
                    player.get_volume(),
                    state.shuffle_enabled,
                    state.repeat_enabled,
                    state.queue_visible,
                    &state.manual_queue,
                    &state.auto_queue,
                    current_playing_track,
                    previous_playing_track,
                    state.help_visible,
                    state.quit_confirm_visible,
                    state.quit_confirm_selected,
                    state.search_popup_visible,
                    &state.search_query,
                    state.search_matches.len(),
                    state.visualizer_mode,
                    &wave_buffer,
                    state.visualizer_view,
                )
            })?;

            last_tick = Instant::now();

            match state.selected_subtab {
                0 => spawn_fetch(Arc::clone(api), tx_likes.clone(), |api| {
                    api.get_liked_tracks()
                }),
                1 => spawn_fetch(Arc::clone(api), tx_playlists.clone(), |api| {
                    api.get_playlists()
                }),
                2 => spawn_fetch(Arc::clone(api), tx_albums.clone(), |api| api.get_albums()),
                3 => spawn_fetch(Arc::clone(api), tx_following.clone(), |api| {
                    api.get_following()
                }),
                _ => {}
            }
        }
    }
}
