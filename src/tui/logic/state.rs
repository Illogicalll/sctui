use crate::api::{API, Album, Artist, Playlist, Track};
use ratatui::widgets::TableState;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;

pub struct AppState {
    pub selected_tab: usize,
    pub selected_subtab: usize,
    pub selected_row: usize,
    pub query: String,
    pub selected_searchfilter: usize,
    pub info_pane_selected: bool,
    pub selected_info_row: usize,
    pub progress: u64,
    pub current_playing_index: Option<usize>,
    pub shuffle_enabled: bool,
    pub repeat_enabled: bool,
    pub playback_history: Vec<usize>,
    pub manual_queue: VecDeque<usize>,
    pub auto_queue: VecDeque<usize>,
    pub queue_visible: bool,
    pub help_visible: bool,
    pub quit_confirm_visible: bool,
    pub quit_confirm_selected: usize,
    pub search_popup_visible: bool,
    pub search_query: String,
    pub search_matches: Vec<usize>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            selected_tab: 0,
            selected_subtab: 0,
            selected_row: 0,
            query: String::new(),
            selected_searchfilter: 0,
            info_pane_selected: false,
            selected_info_row: 0,
            progress: 0,
            current_playing_index: None,
            shuffle_enabled: false,
            repeat_enabled: false,
            playback_history: Vec::new(),
            manual_queue: VecDeque::new(),
            auto_queue: VecDeque::new(),
            queue_visible: false,
            help_visible: false,
            quit_confirm_visible: false,
            quit_confirm_selected: 0,
            search_popup_visible: false,
            search_query: String::new(),
            search_matches: Vec::new(),
        }
    }
}

pub struct AppData {
    pub likes: Vec<Track>,
    pub likes_state: TableState,
    pub playlists: Vec<Playlist>,
    pub playlists_state: TableState,
    pub albums: Vec<Album>,
    pub albums_state: TableState,
    pub following: Vec<Artist>,
    pub following_state: TableState,
}

impl AppData {
    pub fn new(api: &mut API, selected_row: usize) -> anyhow::Result<Self> {
        let likes: Vec<Track> = api.get_liked_tracks()?.into_iter().collect();
        let mut likes_state = TableState::default();
        likes_state.select(Some(selected_row));

        let playlists: Vec<Playlist> = api.get_playlists()?.into_iter().collect();
        let mut playlists_state = TableState::default();
        playlists_state.select(Some(selected_row));

        let albums: Vec<Album> = api.get_albums()?.into_iter().collect();
        let mut albums_state = TableState::default();
        albums_state.select(Some(selected_row));

        let following: Vec<Artist> = api.get_following()?.into_iter().collect();
        let mut following_state = TableState::default();
        following_state.select(Some(selected_row));

        Ok(Self {
            likes,
            likes_state,
            playlists,
            playlists_state,
            albums,
            albums_state,
            following,
            following_state,
        })
    }

    pub fn apply_updates(
        &mut self,
        rx_likes: &Receiver<Vec<Track>>,
        rx_playlists: &Receiver<Vec<Playlist>>,
        rx_albums: &Receiver<Vec<Album>>,
        rx_following: &Receiver<Vec<Artist>>,
    ) {
        while let Ok(new_likes) = rx_likes.try_recv() {
            self.likes.extend(new_likes);
        }
        while let Ok(new_playlists) = rx_playlists.try_recv() {
            self.playlists.extend(new_playlists);
        }
        while let Ok(new_albums) = rx_albums.try_recv() {
            self.albums.extend(new_albums);
        }
        while let Ok(new_following) = rx_following.try_recv() {
            self.following.extend(new_following);
        }
    }
}

pub fn table_rows_count(selected_subtab: usize, data: &AppData) -> usize {
    match selected_subtab {
        0 => data.likes.len(),
        1 => data.playlists.len(),
        2 => data.albums.len(),
        3 => data.following.len(),
        _ => 0,
    }
}

pub fn info_table_rows_count() -> usize {
    2
}
