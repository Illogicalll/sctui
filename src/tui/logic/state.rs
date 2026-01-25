use crate::api::{API, Album, Artist, Playlist, Track};
use ratatui::widgets::TableState;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlaybackSource {
    Likes,
    Playlist,
    Album,
}

pub struct AppState {
    pub selected_tab: usize,
    pub selected_subtab: usize,
    pub selected_row: usize,
    pub selected_playlist_row: usize,
    pub selected_album_row: usize,
    pub query: String,
    pub selected_searchfilter: usize,
    pub info_pane_selected: bool,
    pub selected_info_row: usize,
    pub selected_playlist_track_row: usize,
    pub selected_album_track_row: usize,
    pub playlist_tracks_request_id: u64,
    pub album_tracks_request_id: u64,
    pub playlist_tracks_task: Option<tokio::task::JoinHandle<()>>,
    pub album_tracks_task: Option<tokio::task::JoinHandle<()>>,
    pub progress: u64,
    pub current_playing_index: Option<usize>,
    pub playback_source: PlaybackSource,
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
            selected_playlist_row: 0,
            selected_album_row: 0,
            query: String::new(),
            selected_searchfilter: 0,
            info_pane_selected: false,
            selected_info_row: 0,
            selected_playlist_track_row: 0,
            playlist_tracks_request_id: 0,
            playlist_tracks_task: None,
            selected_album_track_row: 0,
            album_tracks_request_id: 0,
            album_tracks_task: None,
            progress: 0,
            current_playing_index: None,
            playback_source: PlaybackSource::Likes,
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
    pub playlist_tracks: Vec<Track>,
    pub playlist_tracks_state: TableState,
    pub playlist_tracks_uri: Option<String>,
    pub album_tracks: Vec<Track>,
    pub album_tracks_state: TableState,
    pub album_tracks_uri: Option<String>,
    pub playback_tracks: Vec<Track>,
    pub playback_playlist_uri: Option<String>,
    pub playback_album_uri: Option<String>,
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

        let playlist_tracks: Vec<Track> = Vec::new();
        let mut playlist_tracks_state = TableState::default();
        playlist_tracks_state.select(Some(0));

        let album_tracks: Vec<Track> = Vec::new();
        let mut album_tracks_state = TableState::default();
        album_tracks_state.select(Some(0));

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
            playlist_tracks,
            playlist_tracks_state,
            playlist_tracks_uri: None,
            album_tracks,
            album_tracks_state,
            album_tracks_uri: None,
            playback_tracks: Vec::new(),
            playback_playlist_uri: None,
            playback_album_uri: None,
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
        rx_playlist_tracks: &Receiver<(u64, Vec<Track>)>,
        rx_album_tracks: &Receiver<(u64, Vec<Track>)>,
        rx_albums: &Receiver<Vec<Album>>,
        rx_following: &Receiver<Vec<Artist>>,
        playlist_tracks_request_id: u64,
        album_tracks_request_id: u64,
    ) {
        while let Ok(new_likes) = rx_likes.try_recv() {
            self.likes.extend(new_likes);
        }
        while let Ok(new_playlists) = rx_playlists.try_recv() {
            self.playlists.extend(new_playlists);
        }
        while let Ok((request_id, new_tracks)) = rx_playlist_tracks.try_recv() {
            if request_id == playlist_tracks_request_id {
                self.playlist_tracks = new_tracks;
                self.playlist_tracks_state.select(Some(0));
            }
        }
        while let Ok((request_id, new_tracks)) = rx_album_tracks.try_recv() {
            if request_id == album_tracks_request_id {
                self.album_tracks = new_tracks;
                self.album_tracks_state.select(Some(0));
            }
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
