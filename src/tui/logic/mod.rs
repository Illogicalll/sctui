mod filtering;
mod input;
mod animation;
mod state;
mod utils;

use crate::api::API;
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
use self::state::{AppData, AppState};
use self::utils::build_queue;

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

fn spawn_fetch<T, F>(api: Arc<Mutex<API>>, tx: std::sync::mpsc::Sender<Vec<T>>, fetch_fn: F)
where
    T: Send + 'static,
    F: FnOnce(&mut API) -> anyhow::Result<Vec<T>> + Send + 'static,
{
    std::thread::spawn(move || {
        let result = {
            let mut api_guard = api.lock().unwrap();
            fetch_fn(&mut api_guard)
        };

        if let Ok(items) = result {
            let _ = tx.send(items);
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

    let (tx_likes, rx_likes): (Sender<Vec<crate::api::Track>>, Receiver<Vec<crate::api::Track>>) =
        mpsc::channel();
    let (tx_playlists, rx_playlists): (
        Sender<Vec<crate::api::Playlist>>,
        Receiver<Vec<crate::api::Playlist>>,
    ) = mpsc::channel();
    let (tx_albums, rx_albums): (Sender<Vec<crate::api::Album>>, Receiver<Vec<crate::api::Album>>) =
        mpsc::channel();
    let (tx_following, rx_following): (
        Sender<Vec<crate::api::Artist>>,
        Receiver<Vec<crate::api::Artist>>,
    ) = mpsc::channel();

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

    let tick_rate = Duration::from_millis(200);
    let mut last_tick = Instant::now();

    loop {
        data.apply_updates(&rx_likes, &rx_playlists, &rx_albums, &rx_following);

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
        let playlists_len = if filter_active && state.selected_subtab == 1 {
            filtered.playlists.len()
        } else {
            data.playlists.len()
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
            playlists_len,
            albums_len,
            following_len,
        );

        let likes_ref = if filter_active && state.selected_subtab == 0 {
            &filtered.likes
        } else {
            &data.likes
        };
        let playlists_ref = if filter_active && state.selected_subtab == 1 {
            &filtered.playlists
        } else {
            &data.playlists
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

        let previous_playing_index = state.playback_history.last().copied();
        terminal.draw(|frame| {
            render(
                frame,
                likes_ref,
                &data.likes,
                &mut data.likes_state,
                playlists_ref,
                &mut data.playlists_state,
                albums_ref,
                &mut data.albums_state,
                following_ref,
                &mut data.following_state,
                state.selected_tab,
                &TAB_TITLES,
                state.selected_subtab,
                &SUBTAB_TITLES,
                state.selected_row,
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
                state.current_playing_index,
                previous_playing_index,
                state.help_visible,
                state.quit_confirm_visible,
                state.quit_confirm_selected,
                state.search_popup_visible,
                &state.search_query,
                state.search_matches.len(),
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

            if player.is_playing() {
                on_tick(&mut data_points, &mut window, &mut signal);
            }

            let current_track = player.current_track();
            if let Some(current_idx) = state.current_playing_index {
                if state.progress >= current_track.duration_ms.saturating_sub(50)
                    && current_track.duration_ms > 0
                {
                    if state.repeat_enabled {
                        if let Some(track) = data.likes.get(current_idx) {
                            player.play(track.clone());
                            if state.selected_tab == 0 && state.selected_subtab == 0 {
                                state.selected_row = current_idx;
                                data.likes_state.select(Some(current_idx));
                            }
                        }
                    } else {
                        if state.manual_queue.is_empty() && state.auto_queue.is_empty() {
                            state.auto_queue =
                                build_queue(current_idx, data.likes.len(), state.shuffle_enabled);
                        }
                        let next_idx = if let Some(idx) = state.manual_queue.pop_front() {
                            Some(idx)
                        } else {
                            state.auto_queue.pop_front()
                        };
                        if let Some(next_idx) = next_idx {
                            if let Some(track) = data.likes.get(next_idx) {
                                state.playback_history.push(current_idx);
                                player.play(track.clone());
                                state.current_playing_index = Some(next_idx);
                                if state.selected_tab == 0 && state.selected_subtab == 0 {
                                    state.selected_row = next_idx;
                                    data.likes_state.select(Some(next_idx));
                                }
                            }
                        } else {
                            player.pause();
                            state.current_playing_index = None;
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
            let playlists_len = if filter_active && state.selected_subtab == 1 {
                filtered.playlists.len()
            } else {
                data.playlists.len()
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
                playlists_len,
                albums_len,
                following_len,
            );

            let likes_ref = if filter_active && state.selected_subtab == 0 {
                &filtered.likes
            } else {
                &data.likes
            };
            let playlists_ref = if filter_active && state.selected_subtab == 1 {
                &filtered.playlists
            } else {
                &data.playlists
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

            let previous_playing_index = state.playback_history.last().copied();
            terminal.draw(|frame| {
                render(
                    frame,
                    likes_ref,
                    &data.likes,
                    &mut data.likes_state,
                    playlists_ref,
                    &mut data.playlists_state,
                    albums_ref,
                    &mut data.albums_state,
                    following_ref,
                    &mut data.following_state,
                    state.selected_tab,
                    &TAB_TITLES,
                    state.selected_subtab,
                    &SUBTAB_TITLES,
                    state.selected_row,
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
                    state.current_playing_index,
                    previous_playing_index,
                    state.help_visible,
                    state.quit_confirm_visible,
                    state.quit_confirm_selected,
                    state.search_popup_visible,
                    &state.search_query,
                    state.search_matches.len(),
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
