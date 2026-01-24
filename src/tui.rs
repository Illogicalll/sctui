use crate::api::{API, Album, Artist, Playlist, Track};
use crate::player::Player;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::{self},
    text::{Span, Text},
    widgets::{
        Axis, Block, BorderType, Borders, Cell, Chart, Dataset, Gauge, Paragraph, Row, Table,
        TableState, Tabs, Clear,
    },
};

use std::result::Result::Ok;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use rand::seq::SliceRandom;
use rand::Rng;

use ratatui_image::{
    StatefulImage,
    errors::Errors,
    picker::Picker,
    thread::{ResizeRequest, ResizeResponse, ThreadProtocol},
};
use reqwest;

static NUM_TABS: usize = 3;
static NUM_SUBTABS: usize = 4;
static NUM_SEARCHFILTERS: usize = 4;
static NUM_FEED_ACTIVITY_COLS: usize = 4;
static NUM_FEED_INFO_COLS: usize = 3;

// part of playing animation
#[derive(Clone)]
struct SinSignal {
    x: f64,
    interval: f64,
    period: f64,
    scale: f64,
}

impl SinSignal {
    const fn new(interval: f64, period: f64, scale: f64) -> Self {
        Self {
            x: 0.0,
            interval,
            period,
            scale,
        }
    }
}

impl Iterator for SinSignal {
    type Item = (f64, f64);
    fn next(&mut self) -> Option<Self::Item> {
        let point = (self.x, (self.x * 1.0 / self.period).sin() * self.scale);
        self.x += self.interval;
        Some(point)
    }
}

// for cover art
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

// function to create a separate thread to fetch from the API
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

// state management and render loop
fn start(
    mut terminal: DefaultTerminal,
    api: &mut Arc<Mutex<API>>,
    player: Player,
) -> anyhow::Result<()> {
    let tab_titles = ["Library", "Search", "Feed"];
    let mut selected_tab = 0;
    let mut selected_subtab = 0;
    let mut selected_row = 0;
    let subtab_titles = ["Likes", "Playlists", "Albums", "Following"];
    let mut query = String::new();
    let searchfilters = ["Tracks", "Albums", "Playlists", "People"];
    let mut selected_searchfilter = 0;
    let mut info_pane_selected = false;
    let mut selected_info_row = 0;

    let mut api_guard = api.lock().unwrap();

    let mut likes: Vec<Track> = api_guard.get_liked_tracks()?.into_iter().collect();
    let mut likes_state = TableState::default();
    likes_state.select(Some(selected_row));

    let mut playlists: Vec<Playlist> = api_guard.get_playlists()?.into_iter().collect();
    let mut playlists_state = TableState::default();
    playlists_state.select(Some(selected_row));

    let mut albums: Vec<Album> = api_guard.get_albums()?.into_iter().collect();
    let mut albums_state = TableState::default();
    albums_state.select(Some(selected_row));

    let mut following: Vec<Artist> = api_guard.get_following()?.into_iter().collect();
    let mut following_state = TableState::default();
    following_state.select(Some(selected_row));

    drop(api_guard);

    let mut signal = SinSignal::new(0.1, 2.0, 10.0);
    let mut data = signal.by_ref().take(200).collect::<Vec<(f64, f64)>>();
    let mut window = [0.0, 20.0];

    let mut progress = 0;
    let mut current_playing_index: Option<usize> = None;
    let mut shuffle_enabled = false;
    let mut repeat_enabled = false;
    let mut playback_history: Vec<usize> = Vec::new();
    let mut manual_queue: VecDeque<usize> = VecDeque::new();
    let mut auto_queue: VecDeque<usize> = VecDeque::new();
    let mut queue_visible = false;
    let mut help_visible = false;
    let mut quit_confirm_visible = false;
    let mut quit_confirm_selected = 0;
    let mut search_popup_visible = false;
    let mut search_query = String::new();
    let mut search_matches: Vec<usize> = Vec::new();

    let get_table_rows_count = |selected_subtab: usize,
                                likes: &Vec<Track>,
                                playlists: &Vec<Playlist>,
                                albums: &Vec<Album>,
                                following: &Vec<Artist>|
     -> usize {
        match selected_subtab {
            0 => likes.len(),
            1 => playlists.len(),
            2 => albums.len(),
            3 => following.len(),
            _ => 0,
        }
    };

    let (tx_likes, rx_likes): (Sender<Vec<Track>>, Receiver<Vec<Track>>) = mpsc::channel();
    let (tx_playlists, rx_playlists): (Sender<Vec<Playlist>>, Receiver<Vec<Playlist>>) =
        mpsc::channel();
    let (tx_albums, rx_albums): (Sender<Vec<Album>>, Receiver<Vec<Album>>) = mpsc::channel();
    let (tx_following, rx_following): (Sender<Vec<Artist>>, Receiver<Vec<Artist>>) =
        mpsc::channel();

    // used to create resize protocols for cover art
    let picker = Picker::from_query_stdio()?;

    // define channels to send & receive requests for resizing/encoding to worker thread
    let (tx_worker, rx_worker) = mpsc::channel::<ResizeRequest>();
    let (tx_main, rx_main) = mpsc::channel::<AppEvent>();

    // worker thread for resizing and encoding image data
    {
        let tx_main_render = tx_main.clone();
        std::thread::spawn(move || {
            loop {
                if let Ok(request) = rx_worker.recv() {
                    tx_main_render
                        .send(AppEvent::Redraw(request.resize_encode()))
                        .unwrap();
                }
            }
        });
    }

    // holds state for the image being rendered
    let mut cover_art_async = ThreadProtocol::new(tx_worker.clone(), None);

    // avoid redundant downloads
    let mut last_artwork_url: Option<String> = None;

    let tick_rate = Duration::from_millis(200); // animation update rate
    let mut last_tick = Instant::now();

    loop {
        // attempt to update application data between frames (to avoid hitching)
        while let Ok(new_likes) = rx_likes.try_recv() {
            likes.extend(new_likes);
        }
        while let Ok(new_playlists) = rx_playlists.try_recv() {
            playlists.extend(new_playlists);
        }
        while let Ok(new_albums) = rx_albums.try_recv() {
            albums.extend(new_albums);
        }
        while let Ok(new_following) = rx_following.try_recv() {
            following.extend(new_following);
        }

        // handle completed image resize/encode results from worker thread
        while let Ok(app_ev) = rx_main.try_recv() {
            match app_ev {
                AppEvent::Redraw(completed) => {
                    let _ = cover_art_async.update_resized_protocol(completed?);
                }
            }
        }

        // if selected track artwork changed, download the image and request resize
        let url = &player.current_track().artwork_url;
        let should_update = match &last_artwork_url {
            Some(prev) => prev != url,
            None => true,
        };

        if should_update {
            if let Ok(resp) = reqwest::blocking::get(url.as_str()) {
                if let Ok(bytes) = resp.bytes() {
                    if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                        let resize_proto = picker.new_resize_protocol(dyn_img);
                        cover_art_async =
                            ThreadProtocol::new(tx_worker.clone(), Some(resize_proto));

                        last_artwork_url = Some(url.clone());
                    }
                }
            }
        }

        let previous_playing_index = playback_history.last().copied();
        let filter_active =
            search_popup_visible && selected_tab == 0 && !search_query.trim().is_empty();
        let filtered_likes = if filter_active && selected_subtab == 0 {
            likes
                .iter()
                .filter(|track| {
                    let q = search_query.trim().to_lowercase();
                    !q.is_empty()
                        && (track.title.to_lowercase().contains(&q)
                            || track.artists.to_lowercase().contains(&q))
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let filtered_playlists = if filter_active && selected_subtab == 1 {
            playlists
                .iter()
                .filter(|playlist| {
                    let q = search_query.trim().to_lowercase();
                    !q.is_empty() && playlist.title.to_lowercase().contains(&q)
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let filtered_albums = if filter_active && selected_subtab == 2 {
            albums
                .iter()
                .filter(|album| {
                    let q = search_query.trim().to_lowercase();
                    !q.is_empty()
                        && (album.title.to_lowercase().contains(&q)
                            || album.artists.to_lowercase().contains(&q))
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let filtered_following = if filter_active && selected_subtab == 3 {
            following
                .iter()
                .filter(|artist| {
                    let q = search_query.trim().to_lowercase();
                    !q.is_empty() && artist.name.to_lowercase().contains(&q)
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let likes_ref = if filter_active && selected_subtab == 0 {
            &filtered_likes
        } else {
            &likes
        };
        let playlists_ref = if filter_active && selected_subtab == 1 {
            &filtered_playlists
        } else {
            &playlists
        };
        let albums_ref = if filter_active && selected_subtab == 2 {
            &filtered_albums
        } else {
            &albums
        };
        let following_ref = if filter_active && selected_subtab == 3 {
            &filtered_following
        } else {
            &following
        };

        if filter_active {
            match selected_subtab {
                0 => {
                    if selected_row >= likes_ref.len() && !likes_ref.is_empty() {
                        selected_row = likes_ref.len() - 1;
                    }
                    likes_state.select(Some(selected_row));
                }
                1 => {
                    if selected_row >= playlists_ref.len() && !playlists_ref.is_empty() {
                        selected_row = playlists_ref.len() - 1;
                    }
                    playlists_state.select(Some(selected_row));
                }
                2 => {
                    if selected_row >= albums_ref.len() && !albums_ref.is_empty() {
                        selected_row = albums_ref.len() - 1;
                    }
                    albums_state.select(Some(selected_row));
                }
                3 => {
                    if selected_row >= following_ref.len() && !following_ref.is_empty() {
                        selected_row = following_ref.len() - 1;
                    }
                    following_state.select(Some(selected_row));
                }
                _ => {}
            }
        }

        terminal.draw(|frame| {
            render(
                frame,
                likes_ref,
                &likes,
                &mut likes_state,
                playlists_ref,
                &mut playlists_state,
                albums_ref,
                &mut albums_state,
                following_ref,
                &mut following_state,
                selected_tab,
                &tab_titles,
                selected_subtab,
                &subtab_titles,
                selected_row,
                &query,
                &searchfilters,
                selected_searchfilter,
                info_pane_selected,
                selected_info_row,
                &mut data,
                &mut window,
                &mut progress,
                player.current_track(),
                &mut cover_art_async,
                player.get_volume(),
                shuffle_enabled,
                repeat_enabled,
                queue_visible,
                &manual_queue,
                &auto_queue,
                current_playing_index,
                previous_playing_index,
                help_visible,
                quit_confirm_visible,
                quit_confirm_selected,
                search_popup_visible,
                &search_query,
                search_matches.len(),
            )
        })?;

        // input handling
        while event::poll(Duration::from_millis(10))? {
            // poll very frequently for responsiveness (separate from animation tick rate)
            if let Event::Key(key) = event::read()? {
                if quit_confirm_visible {
                    match key.code {
                        KeyCode::Esc => {
                            quit_confirm_visible = false;
                        }
                        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                            quit_confirm_selected = if quit_confirm_selected == 0 { 1 } else { 0 };
                        }
                        KeyCode::Enter => {
                            if quit_confirm_selected == 0 {
                                return Ok(());
                            } else {
                                quit_confirm_visible = false;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                if search_popup_visible {
                    let mut handled = true;
                    match key.code {
                        KeyCode::Backspace => {
                            search_query.pop();
                            search_matches = build_search_matches(
                                selected_subtab,
                                &search_query,
                                &likes,
                                &playlists,
                                &albums,
                                &following,
                            );
                        }
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                if c == 'f' || c == 'F' {
                                    search_popup_visible = false;
                                    search_query.clear();
                                    search_matches.clear();
                                } else {
                                    handled = false;
                                }
                            } else {
                                search_query.push(c);
                                search_matches = build_search_matches(
                                    selected_subtab,
                                    &search_query,
                                    &likes,
                                    &playlists,
                                    &albums,
                                    &following,
                                );
                            }
                        }
                        _ => {
                            handled = false;
                        }
                    }
                    if handled {
                        continue;
                    }
                }
                if player.is_seeking() {
                    continue;
                }
                match key.code {
                    KeyCode::Esc => {
                        quit_confirm_visible = true;
                        quit_confirm_selected = 1;
                    }
                    KeyCode::Tab => {
                        selected_tab = (selected_tab + 1) % NUM_TABS;
                        selected_row = 0;
                    }
                    KeyCode::Right => {
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            // fast forward 10 seconds
                            if player.is_playing() || current_playing_index.is_some() {
                                player.fast_forward();
                            }
                        } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                            // play next song
                            if let Some(current_idx) = current_playing_index {
                                if manual_queue.is_empty() && auto_queue.is_empty() {
                                    auto_queue = build_queue(current_idx, likes.len(), shuffle_enabled);
                                }
                                let next_idx = if let Some(idx) = manual_queue.pop_front() {
                                    Some(idx)
                                } else {
                                    auto_queue.pop_front()
                                };
                                if let Some(next_idx) = next_idx {
                                    if let Some(track) = likes.get(next_idx) {
                                        playback_history.push(current_idx);
                                        player.play(track.clone());
                                        current_playing_index = Some(next_idx);
                                        if selected_tab == 0 && selected_subtab == 0 {
                                            selected_row = next_idx;
                                            likes_state.select(Some(next_idx));
                                        }
                                    }
                                }
                            }
                        } else {
                            if selected_tab == 0 {
                                selected_subtab = (selected_subtab + 1) % NUM_SUBTABS;
                                selected_row = 0;
                            } else if selected_tab == 1 {
                                selected_searchfilter =
                                    (selected_searchfilter + 1) % NUM_SEARCHFILTERS;
                                selected_row = 0;
                            } else if selected_tab == 2 {
                                info_pane_selected = !info_pane_selected;
                            }
                        }
                    }
                    KeyCode::Left => {
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            // rewind 10 seconds
                            if player.is_playing() || current_playing_index.is_some() {
                                player.rewind();
                            }
                        } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                            // play previous song from history
                            if let Some(current_idx) = current_playing_index {
                                if let Some(prev_idx) = playback_history.pop() {
                                    if let Some(track) = likes.get(prev_idx) {
                                        player.play(track.clone());
                                        current_playing_index = Some(prev_idx);
                                        if selected_tab == 0 && selected_subtab == 0 {
                                            selected_row = prev_idx;
                                            likes_state.select(Some(prev_idx));
                                        }
                                        auto_queue = build_queue(prev_idx, likes.len(), shuffle_enabled);
                                    }
                                }
                            }
                        } else {
                            if selected_tab == 0 {
                                selected_subtab = if selected_subtab == 0 {
                                    NUM_SUBTABS - 1
                                } else {
                                    selected_subtab - 1
                                };
                                selected_row = 0;
                            } else if selected_tab == 1 {
                                selected_searchfilter = if selected_searchfilter == 0 {
                                    NUM_SEARCHFILTERS - 1
                                } else {
                                    selected_searchfilter - 1
                                };
                                selected_row = 0;
                            } else if selected_tab == 2 {
                                info_pane_selected = !info_pane_selected;
                            }
                        }
                    }
                    KeyCode::Down => {
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            let max_rows = get_table_rows_count(
                                selected_subtab,
                                &likes,
                                &playlists,
                                &albums,
                                &following,
                            );
                            let max_info_rows = get_info_table_rows_count();
                            if selected_tab == 2 && info_pane_selected {
                                if max_info_rows > 0 {
                                    selected_info_row =
                                        (selected_info_row + 10).min(max_info_rows - 1);
                                }
                            } else if max_rows > 0 {
                                selected_row = (selected_row + 10).min(max_rows - 1);
                                match selected_subtab {
                                    0 => likes_state.select(Some(selected_row)),
                                    1 => playlists_state.select(Some(selected_row)),
                                    _ => {}
                                }
                            }
                        } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                            player.volume_down();
                        } else {
                            let max_rows = get_table_rows_count(
                                selected_subtab,
                                &likes,
                                &playlists,
                                &albums,
                                &following,
                            );
                            let max_info_rows = get_info_table_rows_count();
                            if selected_tab == 2
                                && info_pane_selected
                                && selected_info_row + 1 < max_info_rows
                            {
                                selected_info_row += 1;
                            } else if selected_row + 1 < max_rows {
                                selected_row += 1;
                                match selected_subtab {
                                    0 => likes_state.select(Some(selected_row)),
                                    1 => playlists_state.select(Some(selected_row)),
                                    _ => {}
                                }
                            }
                        }
                    }
                    KeyCode::Up => {
                        if key.modifiers.contains(KeyModifiers::ALT) {
                            if selected_tab == 2 && info_pane_selected {
                                selected_info_row = selected_info_row.saturating_sub(10);
                            } else {
                                selected_row = selected_row.saturating_sub(10);
                                match selected_subtab {
                                    0 => likes_state.select(Some(selected_row)),
                                    1 => playlists_state.select(Some(selected_row)),
                                    _ => {}
                                }
                            }
                        } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                            player.volume_up();
                        } else {
                            if selected_tab == 2 && info_pane_selected && selected_info_row > 0 {
                                selected_info_row -= 1;
                            } else if selected_row > 0 {
                                selected_row -= 1;
                                likes_state.select(Some(selected_row));
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(KeyModifiers::SHIFT) {
                            match c {
                                's' | 'S' => {
                                    shuffle_enabled = !shuffle_enabled;
                                    if let Some(current_idx) = current_playing_index {
                                        auto_queue = build_queue(current_idx, likes.len(), shuffle_enabled);
                                    }
                                }
                                'r' | 'R' => {
                                    repeat_enabled = !repeat_enabled;
                                }
                                'a' | 'A' => {
                                    if selected_tab == 0 && selected_subtab == 0 {
                                        let search_active = search_popup_visible
                                            && !search_query.trim().is_empty();
                                        if search_active {
                                            if let Some(&idx) = search_matches.get(selected_row) {
                                                manual_queue.push_back(idx);
                                            }
                                        } else {
                                            manual_queue.push_back(selected_row);
                                        }
                                    }
                                }
                                'f' | 'F' => {
                                    if selected_tab == 0 {
                                        search_popup_visible = true;
                                        search_query.clear();
                                        search_matches = build_search_matches(
                                            selected_subtab,
                                            &search_query,
                                            &likes,
                                            &playlists,
                                            &albums,
                                            &following,
                                        );
                                    }
                                }
                                'h' | 'H' => {
                                    help_visible = !help_visible;
                                }
                                'q' | 'Q' => {
                                    queue_visible = !queue_visible;
                                    if queue_visible {
                                        if let Some(current_idx) = current_playing_index {
                                            if auto_queue.is_empty() {
                                                auto_queue = build_queue(current_idx, likes.len(), shuffle_enabled);
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            if selected_tab == 0 {
                                if c == ' ' {
                                    if player.is_playing() {
                                        player.pause();
                                    } else {
                                        player.resume();
                                    }
                                }
                            } else if selected_tab == 1 {
                                query.push(c);
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        if selected_tab == 1 {
                            query.pop();
                        }
                    }
                    KeyCode::Enter => {
                        if selected_tab == 0 && selected_subtab == 0 {
                            let search_active =
                                search_popup_visible && !search_query.trim().is_empty();
                            let selected_idx = if search_active {
                                search_matches.get(selected_row).copied()
                            } else {
                                Some(selected_row)
                            };
                            if let Some(selected_idx) = selected_idx {
                                if let Some(track) = likes.get(selected_idx) {
                                    // if switching to a new song, add previous to history
                                    if let Some(prev_idx) = current_playing_index {
                                        if prev_idx != selected_idx {
                                            playback_history.push(prev_idx);
                                        }
                                    }
                                    player.play(track.clone());
                                    current_playing_index = Some(selected_idx);
                                    auto_queue =
                                        build_queue(selected_idx, likes.len(), shuffle_enabled);
                                    if !search_active {
                                        if selected_tab == 0 && selected_subtab == 0 {
                                            selected_row = selected_idx;
                                            likes_state.select(Some(selected_row));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // tick playing animation and render
        if last_tick.elapsed() >= tick_rate {
            progress = player.elapsed();
            
            if player.is_playing() {
                on_tick(&mut data, &mut window, &mut signal);
            }
            
            // check if current song has finished and auto-play next song
            let current_track = player.current_track();
            if let Some(current_idx) = current_playing_index {
                // check if song has finished (progress >= duration)
                // use a small buffer (50ms) to account for timing precision
                if progress >= current_track.duration_ms.saturating_sub(50) && current_track.duration_ms > 0 {
                    if repeat_enabled {
                        // repeat: play the same song again
                        if let Some(track) = likes.get(current_idx) {
                            player.play(track.clone());
                            if selected_tab == 0 && selected_subtab == 0 {
                                selected_row = current_idx;
                                likes_state.select(Some(current_idx));
                            }
                        }
                    } else {
                        if manual_queue.is_empty() && auto_queue.is_empty() {
                            auto_queue = build_queue(current_idx, likes.len(), shuffle_enabled);
                        }
                        let next_idx = if let Some(idx) = manual_queue.pop_front() {
                            Some(idx)
                        } else {
                            auto_queue.pop_front()
                        };
                        if let Some(next_idx) = next_idx {
                            if let Some(track) = likes.get(next_idx) {
                                playback_history.push(current_idx);
                                player.play(track.clone());
                                current_playing_index = Some(next_idx);
                                if selected_tab == 0 && selected_subtab == 0 {
                                    selected_row = next_idx;
                                    likes_state.select(Some(next_idx));
                                }
                            }
                        } else {
                            // reached end of list, stop playing
                            player.pause();
                            current_playing_index = None;
                        }
                    }
                }
            }

            let filter_active =
                search_popup_visible && selected_tab == 0 && !search_query.trim().is_empty();
            let filtered_likes = if filter_active && selected_subtab == 0 {
                likes
                    .iter()
                    .filter(|track| {
                        let q = search_query.trim().to_lowercase();
                        !q.is_empty()
                            && (track.title.to_lowercase().contains(&q)
                                || track.artists.to_lowercase().contains(&q))
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let filtered_playlists = if filter_active && selected_subtab == 1 {
                playlists
                    .iter()
                    .filter(|playlist| {
                        let q = search_query.trim().to_lowercase();
                        !q.is_empty() && playlist.title.to_lowercase().contains(&q)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let filtered_albums = if filter_active && selected_subtab == 2 {
                albums
                    .iter()
                    .filter(|album| {
                        let q = search_query.trim().to_lowercase();
                        !q.is_empty()
                            && (album.title.to_lowercase().contains(&q)
                                || album.artists.to_lowercase().contains(&q))
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            let filtered_following = if filter_active && selected_subtab == 3 {
                following
                    .iter()
                    .filter(|artist| {
                        let q = search_query.trim().to_lowercase();
                        !q.is_empty() && artist.name.to_lowercase().contains(&q)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };

            let likes_ref = if filter_active && selected_subtab == 0 {
                &filtered_likes
            } else {
                &likes
            };
            let playlists_ref = if filter_active && selected_subtab == 1 {
                &filtered_playlists
            } else {
                &playlists
            };
            let albums_ref = if filter_active && selected_subtab == 2 {
                &filtered_albums
            } else {
                &albums
            };
            let following_ref = if filter_active && selected_subtab == 3 {
                &filtered_following
            } else {
                &following
            };

            if filter_active {
                match selected_subtab {
                    0 => {
                        if selected_row >= likes_ref.len() && !likes_ref.is_empty() {
                            selected_row = likes_ref.len() - 1;
                        }
                        likes_state.select(Some(selected_row));
                    }
                    1 => {
                        if selected_row >= playlists_ref.len() && !playlists_ref.is_empty() {
                            selected_row = playlists_ref.len() - 1;
                        }
                        playlists_state.select(Some(selected_row));
                    }
                    2 => {
                        if selected_row >= albums_ref.len() && !albums_ref.is_empty() {
                            selected_row = albums_ref.len() - 1;
                        }
                        albums_state.select(Some(selected_row));
                    }
                    3 => {
                        if selected_row >= following_ref.len() && !following_ref.is_empty() {
                            selected_row = following_ref.len() - 1;
                        }
                        following_state.select(Some(selected_row));
                    }
                    _ => {}
                }
            }

            let previous_playing_index = playback_history.last().copied();
            terminal.draw(|frame| {
                render(
                    frame,
                    likes_ref,
                    &likes,
                    &mut likes_state,
                    playlists_ref,
                    &mut playlists_state,
                    albums_ref,
                    &mut albums_state,
                    following_ref,
                    &mut following_state,
                    selected_tab,
                    &tab_titles,
                    selected_subtab,
                    &subtab_titles,
                    selected_row,
                    &query,
                    &searchfilters,
                    selected_searchfilter,
                    info_pane_selected,
                    selected_info_row,
                    &mut data,
                    &mut window,
                    &mut progress,
                    player.current_track(),
                    &mut cover_art_async,
                    player.get_volume(),
                    shuffle_enabled,
                    repeat_enabled,
                    queue_visible,
                    &manual_queue,
                    &auto_queue,
                    current_playing_index,
                    previous_playing_index,
                    help_visible,
                    quit_confirm_visible,
                    quit_confirm_selected,
                    search_popup_visible,
                    &search_query,
                    search_matches.len(),
                )
            })?;

            last_tick = Instant::now();

            match selected_subtab {
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

fn get_info_table_rows_count() -> usize {
    2
}

fn build_queue(current_idx: usize, list_len: usize, shuffle_enabled: bool) -> VecDeque<usize> {
    if list_len == 0 {
        return VecDeque::new();
    }

    if shuffle_enabled {
        let mut indices: Vec<usize> = (0..list_len).filter(|&i| i != current_idx).collect();
        indices.shuffle(&mut rand::thread_rng());
        VecDeque::from(indices)
    } else {
        let indices: Vec<usize> = (current_idx + 1..list_len).collect();
        VecDeque::from(indices)
    }
}

fn build_search_matches(
    selected_subtab: usize,
    query: &str,
    likes: &Vec<Track>,
    playlists: &Vec<Playlist>,
    albums: &Vec<Album>,
    following: &Vec<Artist>,
) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    match selected_subtab {
        0 => likes
            .iter()
            .enumerate()
            .filter(|(_, track)| {
                track.title.to_lowercase().contains(&q)
                    || track.artists.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect(),
        1 => playlists
            .iter()
            .enumerate()
            .filter(|(_, playlist)| playlist.title.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect(),
        2 => albums
            .iter()
            .enumerate()
            .filter(|(_, album)| {
                album.title.to_lowercase().contains(&q)
                    || album.artists.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect(),
        3 => following
            .iter()
            .enumerate()
            .filter(|(_, artist)| artist.name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect(),
        _ => Vec::new(),
    }
}

// styling for table headers
fn styled_header(cells: &[&str]) -> Row<'static> {
    let style = Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let cells: Vec<Cell> = cells
        .iter()
        .map(|&text| Cell::from(text.to_string()).style(style))
        .collect();
    Row::new(cells)
}

// column widths for tables based on the number of columns
fn calculate_column_widths(num_columns: usize) -> Vec<Constraint> {
    if num_columns == 0 {
        return vec![];
    }

    if num_columns > 2 {
        let other_width = 90 / (num_columns as u16 - 1);
        let mut widths = vec![Constraint::Percentage(other_width); num_columns - 1];
        widths.push(Constraint::Percentage(10)); // last column fixed at 10%
        widths
    } else {
        let width = 100 / num_columns as u16;
        (0..num_columns)
            .map(|_| Constraint::Percentage(width))
            .collect()
    }
}

// truncation with ellipsis for text overflow management
fn calculate_min_widths(column_widths: &[Constraint], total_width: usize) -> Vec<usize> {
    column_widths
        .iter()
        .map(|c| match c {
            Constraint::Percentage(p) => (total_width * (*p as usize)) / 100,
            _ => 10,
        })
        .collect()
}

fn truncate_with_ellipsis(s: &str, min_width: usize) -> String {
    if s.chars().count() > min_width && min_width > 3 {
        let truncated: String = s.chars().take(min_width - 3).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

// for playing animation
fn on_tick(data: &mut Vec<(f64, f64)>, window: &mut [f64; 2], signal: &mut SinSignal) {
    data.drain(0..10);
    data.extend(signal.by_ref().take(10));
    window[0] += 1.0;
    window[1] += 1.0;
}

// for progress bar
fn format_duration(duration_ms: u64) -> String {
    let duration_sec = duration_ms / 1000;
    let hours = duration_sec / 3600;
    let minutes = (duration_sec % 3600) / 60;
    let seconds = duration_sec % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

// receives the state and renders the application
fn render(
    frame: &mut Frame,
    likes_view: &Vec<Track>,
    likes_all: &Vec<Track>,
    likes_state: &mut TableState,
    playlists: &Vec<Playlist>,
    playlists_state: &mut TableState,
    albums: &Vec<Album>,
    albums_state: &mut TableState,
    following: &Vec<Artist>,
    following_state: &mut TableState,
    selected_tab: usize,
    tab_titles: &[&str],
    selected_subtab: usize,
    subtab_titles: &[&str],
    selected_row: usize,
    query: &str,
    searchfilters: &[&str],
    selected_searchfilter: usize,
    info_pane_selected: bool,
    selected_info_row: usize,
    data: &mut Vec<(f64, f64)>,
    window: &mut [f64; 2],
    progress: &mut u64,
    selected_track: Track,
    cover_art_async: &mut ThreadProtocol,
    current_volume: f32,
    shuffle_enabled: bool,
    repeat_enabled: bool,
    queue_visible: bool,
    manual_queue: &VecDeque<usize>,
    auto_queue: &VecDeque<usize>,
    current_playing_index: Option<usize>,
    previous_playing_index: Option<usize>,
    help_visible: bool,
    quit_confirm_visible: bool,
    quit_confirm_selected: usize,
    search_popup_visible: bool,
    search_query: &str,
    search_match_count: usize,
) {
    let width = frame.area().width as usize;

    // divide the terminal into 3 'chunks' (tabs, content area, now playing)
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(7),
            ]
            .as_ref(),
        )
        .split(frame.area());

    // main 'library, search, feed' tabs
    let tabs: Vec<_> = tab_titles.iter().map(|t| Span::raw(*t)).collect();
    let tabs_widget = Tabs::new(tabs)
        .block(
            Block::default()
                .title(Span::styled(
                    "sctui",
                    Style::default().add_modifier(Modifier::BOLD),
                ))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs_widget, chunks[0]);

    // change content to be rendered based on the active tab
    //
    // ===========
    // LIBRARY TAB
    // ===========
    //
    if selected_tab == 0 {
        // divide the content area into chunks (subtabs, optional search, table)
        let subchunks = if search_popup_visible {
            Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(chunks[1])
        } else {
            Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
                .split(chunks[1])
        };

        // 'likes, playlists, albums, ...' subtabs
        let subtabs: Vec<_> = subtab_titles.iter().map(|t| Span::raw(*t)).collect();
        let subtabs_widget = Tabs::new(subtabs)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .select(selected_subtab)
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(subtabs_widget, subchunks[0]);

        if search_popup_visible {
            let input = Paragraph::new(search_query.to_string())
                .block(
                    Block::default()
                        .title("search")
                        .title_alignment(Alignment::Center)
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded),
                )
                .alignment(Alignment::Center);
            frame.render_widget(input, subchunks[1]);
        }

        // define headers and column widths for tables for each subtab
        let (header, col_widths) = match selected_subtab {
            0 => (
                styled_header(&["Title", "Artist(s)", "Duration", "Streams"]),
                vec![
                    Constraint::Percentage(55),
                    Constraint::Percentage(25),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                ],
            ),
            1 => (
                styled_header(&["Name", "No. Songs", "Duration"]),
                vec![
                    Constraint::Percentage(80),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                ],
            ),
            2 => (
                styled_header(&["Title", "Artist(s)", "Year", "No. Songs", "Duration"]),
                vec![
                    Constraint::Percentage(50),
                    Constraint::Percentage(20),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                ],
            ),
            3 => (styled_header(&["Name"]), vec![Constraint::Percentage(100)]),
            _ => (
                Row::new(vec![] as Vec<Cell>),
                vec![Constraint::Percentage(100)],
            ),
        };

        // truncation to avoid cut-off text in columns
        let col_min_widths = calculate_min_widths(&col_widths, width);

        // define rows for each subtab
        let rows = match selected_subtab {
            0 => likes_view
                .iter()
                .map(|track| {
                    Row::new(vec![
                        truncate_with_ellipsis(&track.title, col_min_widths[0]),
                        truncate_with_ellipsis(&track.artists, col_min_widths[1]),
                        truncate_with_ellipsis(&track.duration, col_min_widths[2]),
                        truncate_with_ellipsis(&track.playback_count, col_min_widths[3]),
                    ])
                })
                .collect(),
            1 => playlists
                .iter()
                .map(|playlist| {
                    Row::new(vec![
                        truncate_with_ellipsis(&playlist.title, col_min_widths[0]),
                        truncate_with_ellipsis(&playlist.track_count, col_min_widths[1]),
                        truncate_with_ellipsis(&playlist.duration, col_min_widths[2]),
                    ])
                })
                .collect(),
            2 => albums
                .iter()
                .map(|album| {
                    Row::new(vec![
                        truncate_with_ellipsis(&album.title, col_min_widths[0]),
                        truncate_with_ellipsis(&album.artists, col_min_widths[1]),
                        truncate_with_ellipsis(&album.release_year, col_min_widths[2]),
                        truncate_with_ellipsis(&album.track_count, col_min_widths[3]),
                        truncate_with_ellipsis(&album.duration, col_min_widths[4]),
                    ])
                })
                .collect(),
            3 => following
                .iter()
                .map(|artist| {
                    Row::new(vec![truncate_with_ellipsis(
                        &artist.name,
                        col_min_widths[0],
                    )])
                })
                .collect(),
            _ => vec![],
        };

        // highlight selected row
        let rows: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                if i == selected_row {
                    row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
                } else {
                    row
                }
            })
            .collect();

        let state: &mut TableState = match selected_subtab {
            0 => likes_state,
            1 => playlists_state,
            2 => albums_state,
            3 => following_state,
            _ => likes_state,
        };

        // render table
        let table_chunk_idx = if search_popup_visible { 2 } else { 1 };
        let table = Table::new(rows, col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(table, subchunks[table_chunk_idx], state);
    }
    //
    // ==========
    // SEARCH TAB
    // ==========
    //
    else if selected_tab == 1 {
        // divide the content area into 2 'chunks' (search bar and table)
        let subchunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(chunks[1]);

        // search bar
        let input = Paragraph::new(query.to_string())
            .block(
                Block::default()
                    .title("search")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Center);
        frame.render_widget(input, subchunks[0]);

        // define headers for different search filters
        let (header, num_columns) = match selected_searchfilter {
            0 => (
                styled_header(&["Title", "Artist(s)", "Album", "Duration"]),
                4,
            ),
            1 => (
                styled_header(&["Title", "Artist(s)", "Year", "Duration"]),
                4,
            ),
            2 => (styled_header(&["Name", "No. Songs", "Duration"]), 3),
            3 => (styled_header(&["Name"]), 1),
            _ => (Row::new(vec![] as Vec<Cell>), 0),
        };

        // truncation to avoid cut-off text in columns
        let col_widths = calculate_column_widths(num_columns);
        let col_min_widths = calculate_min_widths(&col_widths, width);

        // define rows based on search filter
        let rows = match selected_searchfilter {
            0 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Short song name", col_min_widths[0]),
                    truncate_with_ellipsis("Short artist name", col_min_widths[1]),
                    truncate_with_ellipsis("Short album name", col_min_widths[2]),
                    truncate_with_ellipsis("0:57", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Medium length song name", col_min_widths[0]),
                    truncate_with_ellipsis("Medium length artist name", col_min_widths[1]),
                    truncate_with_ellipsis("Medium length album name", col_min_widths[2]),
                    truncate_with_ellipsis("12:54", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis(
                        "Really really really long song name",
                        col_min_widths[0],
                    ),
                    truncate_with_ellipsis(
                        "Really really really long artist name",
                        col_min_widths[1],
                    ),
                    truncate_with_ellipsis(
                        "Really really really long album name",
                        col_min_widths[2],
                    ),
                    truncate_with_ellipsis("12:59:30", col_min_widths[3]),
                ]),
            ],
            1 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Album One", col_min_widths[0]),
                    truncate_with_ellipsis("Artist X", col_min_widths[1]),
                    truncate_with_ellipsis("1997", col_min_widths[2]),
                    truncate_with_ellipsis("45:02", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Album Two", col_min_widths[0]),
                    truncate_with_ellipsis("Artist Y", col_min_widths[1]),
                    truncate_with_ellipsis("2009", col_min_widths[2]),
                    truncate_with_ellipsis("16:03", col_min_widths[3]),
                ]),
            ],
            2 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Playlist 1", col_min_widths[0]),
                    truncate_with_ellipsis("15", col_min_widths[1]),
                    truncate_with_ellipsis("30:00", col_min_widths[2]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Playlist 2", col_min_widths[0]),
                    truncate_with_ellipsis("1", col_min_widths[1]),
                    truncate_with_ellipsis("2:30", col_min_widths[2]),
                ]),
            ],
            3 => vec![
                Row::new(vec![truncate_with_ellipsis(
                    "Following Artist A",
                    col_min_widths[0],
                )]),
                Row::new(vec![truncate_with_ellipsis(
                    "Following Artist B",
                    col_min_widths[0],
                )]),
            ],
            _ => vec![],
        };

        // highlight selected row
        let rows: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                if i == selected_row {
                    row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
                } else {
                    row
                }
            })
            .collect();

        // render table
        let table = Table::new(rows, col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_widget(table, subchunks[1]);

        // center search filters
        let tab_width = width / NUM_SEARCHFILTERS;
        fn center_text_in_width(text: &str, width: usize) -> String {
            let total_padding = width - text.chars().count();
            let padding = (total_padding / 2) - 1;
            format!("{}{}{}", " ".repeat(padding), text, " ".repeat(padding))
        }

        // render search filters
        let searchfilter: Vec<Span<'static>> = searchfilters
            .iter()
            .map(|filter| Span::raw(center_text_in_width(filter, tab_width)))
            .collect();
        let searchfilter_widget = Tabs::new(searchfilter)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("filter")
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded),
            )
            .select(selected_searchfilter)
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(searchfilter_widget, subchunks[2]);
    }
    //
    // ========
    // FEED TAB
    // ========
    //
    else {
        // split the content area vertically
        let subchunks = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(chunks[1]);

        let activity_header = styled_header(&["User", "Action", "Media Type", "Age"]);

        // truncation to avoid cut-off text in columns
        let activity_col_widths = calculate_column_widths(NUM_FEED_ACTIVITY_COLS);
        let activity_col_min_widths = calculate_min_widths(&activity_col_widths, width / 2);

        // define activity table content
        let activity_rows = vec![
            vec![
                truncate_with_ellipsis("User 1", activity_col_min_widths[0]),
                truncate_with_ellipsis("Post", activity_col_min_widths[1]),
                truncate_with_ellipsis("Track", activity_col_min_widths[2]),
                truncate_with_ellipsis("2d", activity_col_min_widths[3]),
            ],
            vec![
                truncate_with_ellipsis("User 2", activity_col_min_widths[0]),
                truncate_with_ellipsis("Repost", activity_col_min_widths[1]),
                truncate_with_ellipsis("Album", activity_col_min_widths[2]),
                truncate_with_ellipsis("5d", activity_col_min_widths[3]),
            ],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, cols)| {
            let row = Row::new(cols);
            if i == selected_row && info_pane_selected {
                row.style(Style::default().bg(Color::Gray).fg(Color::Black))
            } else if i == selected_row && !info_pane_selected {
                row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
            } else {
                row
            }
        })
        .collect::<Vec<_>>();

        // render activity table
        let table = Table::new(activity_rows, activity_col_widths)
            .header(activity_header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("activity")
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(if info_pane_selected {
                        Style::default()
                    } else {
                        Style::default().fg(Color::Cyan)
                    }),
            )
            .column_spacing(1);
        frame.render_widget(table, subchunks[0]);

        let info_header = styled_header(&["Title", "Artist", "Dur."]);

        // truncation to avoid cut-off text in columns
        let info_col_widths = calculate_column_widths(NUM_FEED_INFO_COLS);
        let info_col_min_widths = calculate_min_widths(&info_col_widths, width / 2);

        // define info table content
        let info_rows = vec![
            vec![
                truncate_with_ellipsis("Track 1", info_col_min_widths[0]),
                truncate_with_ellipsis("Artist 1", info_col_min_widths[1]),
                truncate_with_ellipsis("1:30", info_col_min_widths[2]),
            ],
            vec![
                truncate_with_ellipsis("Track 2", info_col_min_widths[0]),
                truncate_with_ellipsis("Artist 1", info_col_min_widths[1]),
                truncate_with_ellipsis("2:10", info_col_min_widths[2]),
            ],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, cols)| {
            let row = Row::new(cols);
            if i == selected_info_row && info_pane_selected {
                row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
            } else if i == selected_info_row && !info_pane_selected {
                row.style(Style::default().bg(Color::Gray).fg(Color::Black))
            } else {
                row
            }
        })
        .collect::<Vec<_>>();

        // render info table
        let table = Table::new(info_rows, info_col_widths)
            .header(info_header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("info")
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(if info_pane_selected {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    }),
            )
            .column_spacing(1);

        frame.render_widget(table, subchunks[1]);
    }

    //
    // ===================
    // NOW PLAYING DISPLAY
    // ===================
    //

    let subchunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(15),
                Constraint::Percentage(70),
                Constraint::Percentage(15),
            ]
            .as_ref(),
        )
        .split(chunks[2]);

    let now_playing = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded);

    frame.render_widget(now_playing, subchunks[1]);

    let padding = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage(1),
            Constraint::Percentage(98),
            Constraint::Percentage(1),
        ])
        .split(subchunks[1]);

    let horizontal_split = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage(1),
            Constraint::Length(12),
            Constraint::Percentage(5),
            Constraint::Min(0),
            Constraint::Percentage(5),
            Constraint::Length(9),
            Constraint::Percentage(1),
        ])
        .split(padding[1]);

    let image_padding = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(horizontal_split[1]);

    let subsubchunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(20),
        ])
        .split(horizontal_split[3]);

    frame.render_stateful_widget(StatefulImage::new(), image_padding[1], cover_art_async);

    let song_name = Paragraph::new(selected_track.title.clone())
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center);

    frame.render_widget(song_name, subsubchunks[1]);

    let artist = Paragraph::new(selected_track.artists.clone()).alignment(Alignment::Center);

    frame.render_widget(artist, subsubchunks[3]);

    let max_time: f64 = selected_track.duration_ms.clone() as f64;

    let progress_float = *progress as f64;

    let label = Span::styled(
        format!(
            "{} / {}",
            format_duration(*progress),
            selected_track.duration.clone()
        ),
        Style::default().fg(Color::White),
    );

    let ratio = (progress_float / max_time).min(1.0).max(0.0);

    let progress_bar = Gauge::default()
        .style(Style::default().bg(Color::LightBlue))
        .gauge_style(Color::Cyan)
        .ratio(ratio)
        .label(label);

    frame.render_widget(progress_bar, subsubchunks[5]);

    let shuffle_indicator = if shuffle_enabled { "✔︎" } else { "×" };
    let repeat_indicator = if repeat_enabled { "✔︎" } else { "×" };
    
    let lines = vec![
        "".to_string(),
        "".to_string(),
        format!("shf:   {}", shuffle_indicator),
        format!("vol: {:.1}", current_volume),
        format!("rep:   {}", repeat_indicator),
    ];

    let text = Text::from(lines.join("\n"));

    let song_name = Paragraph::new(text)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(Alignment::Right);

    frame.render_widget(song_name, horizontal_split[5]);
    let datasets = vec![
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&data),
    ];

    let chart = Chart::new(datasets)
        .block(Block::default())
        .x_axis(Axis::default().bounds([window[0], window[1]]))
        .y_axis(Axis::default().bounds([-10.0, 10.0]));

    frame.render_widget(&chart, subchunks[2]);
    frame.render_widget(&chart, subchunks[0]);

    if queue_visible {
        let popup_area = centered_rect(70, 60, frame.area());
        frame.render_widget(Clear, popup_area);

        let total_width = popup_area.width as usize;
        let title_width = (total_width * 65) / 100;
        let artist_width = (total_width * 25) / 100;

        let mut rows: Vec<Row> = Vec::new();

        if let Some(prev_idx) = previous_playing_index {
            if let Some(track) = likes_all.get(prev_idx) {
                rows.push(
                    Row::new(vec![
                        truncate_with_ellipsis(&track.title, title_width),
                        truncate_with_ellipsis(&track.artists, artist_width),
                        track.duration.clone(),
                    ])
                    .style(Style::default().fg(Color::DarkGray)),
                );
            }
        } else {
            rows.push(
                Row::new(vec![
                    "Previous: None",
                    "",
                    "",
                ])
                .style(Style::default().fg(Color::DarkGray)),
            );
        }

        if let Some(current_idx) = current_playing_index {
            if let Some(track) = likes_all.get(current_idx) {
                rows.push(
                    Row::new(vec![
                        truncate_with_ellipsis(&track.title, title_width),
                        truncate_with_ellipsis(&track.artists, artist_width),
                        track.duration.clone(),
                    ])
                    .style(Style::default().bg(Color::LightBlue).fg(Color::White)),
                );
            }
        } else {
            rows.push(Row::new(vec!["Now Playing: None", "", ""]));
        }

        let max_rows = popup_area.height.saturating_sub(3) as usize;
        let remaining_slots = max_rows.saturating_sub(rows.len());
        let mut remaining = remaining_slots;
        for idx in manual_queue.iter() {
            if remaining == 0 {
                break;
            }
            if let Some(track) = likes_all.get(*idx) {
                rows.push(Row::new(vec![
                    truncate_with_ellipsis(&track.title, title_width),
                    truncate_with_ellipsis(&track.artists, artist_width),
                    track.duration.clone(),
                ]));
                remaining -= 1;
            }
        }

        for idx in auto_queue.iter() {
            if remaining == 0 {
                break;
            }
            if let Some(track) = likes_all.get(*idx) {
                rows.push(Row::new(vec![
                    truncate_with_ellipsis(&track.title, title_width),
                    truncate_with_ellipsis(&track.artists, artist_width),
                    track.duration.clone(),
                ]));
                remaining -= 1;
            }
        }

        if rows.len() <= 2 {
            rows.push(Row::new(vec!["Queue is empty", "", ""]));
        }

        let header = styled_header(&["Title", "Artist", "Duration"]);
        let table = Table::new(
            rows,
            vec![
                Constraint::Percentage(65),
                Constraint::Percentage(25),
                Constraint::Percentage(10),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title("Queue")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .column_spacing(1);
        frame.render_widget(table, popup_area);
    }

    if help_visible {
        let popup_area = centered_rect(70, 70, frame.area());
        frame.render_widget(Clear, popup_area);

        let mut rows: Vec<Row> = vec![
            Row::new(vec!["Esc", "Quit"]),
            Row::new(vec!["Tab", "Cycle main tabs"]),
            Row::new(vec!["Left/Right", "Change sub-tab"]),
            Row::new(vec!["Up/Down", "Move selector"]),
            Row::new(vec!["Space", "Play/Pause"]),
            Row::new(vec!["Enter", "Play selected track"]),
            Row::new(vec!["Shift + Right", "Skip song"]),
            Row::new(vec!["Shift + Left", "Go back a song"]),
            Row::new(vec!["Option + Right", "Fast forward 10s"]),
            Row::new(vec!["Option + Left", "Rewind 10s"]),
            Row::new(vec!["Option + Up/Down", "Jump 10 items"]),
            Row::new(vec!["Shift + Up/Down", "Volume up/down"]),
            Row::new(vec!["Shift + S", "Toggle shuffle queue"]),
            Row::new(vec!["Shift + R", "Toggle repeat same song"]),
            Row::new(vec!["Shift + A", "Add selected song to queue"]),
            Row::new(vec!["Shift + F", "Search current view (only works in library)"]),
            Row::new(vec!["Shift + Q", "Toggle queue popup"]),
            Row::new(vec!["Shift + H", "Toggle help popup"]),
        ];

        let max_rows = popup_area.height.saturating_sub(3) as usize;
        if rows.len() > max_rows {
            rows.truncate(max_rows);
        }

        let table = Table::new(
            rows,
            vec![Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .header(styled_header(&["Action", "Description"]))
        .block(
            Block::default()
                .title("Help")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .column_spacing(1);

        frame.render_widget(table, popup_area);
    }

    if quit_confirm_visible {
        let popup_area = centered_rect(25, 10, frame.area());
        frame.render_widget(Clear, popup_area);

        let yes_style = if quit_confirm_selected == 0 {
            Style::default().fg(Color::Black).bg(Color::LightBlue).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let no_style = if quit_confirm_selected == 1 {
            Style::default().fg(Color::Black).bg(Color::LightBlue).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let line = Text::from(vec![
            ratatui::text::Line::from("Are you Sure you Want to Quit?"),
            ratatui::text::Line::from(""),
            ratatui::text::Line::from(vec![
                Span::styled("Yes", yes_style),
                Span::raw("   "),
                Span::styled("No", no_style),
            ]),
        ]);

        let box_widget = Paragraph::new(line)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            );
        frame.render_widget(box_widget, popup_area);
    }

}
