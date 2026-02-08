use crate::api::{API, Album, Artist, Playlist, Track};
use ratatui::widgets::TableState;
use std::collections::{HashSet, VecDeque};
use std::sync::mpsc::Receiver;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PlaybackSource {
    Likes,
    Playlist,
    Album,
    FollowingPublished,
    FollowingLikes,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum VisualizerMode {
    Oscilloscope,
    SpectrumBars,
}

impl VisualizerMode {
    pub fn next(self) -> Self {
        match self {
            Self::Oscilloscope => Self::SpectrumBars,
            Self::SpectrumBars => Self::Oscilloscope,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Oscilloscope => "Oscilloscope",
            Self::SpectrumBars => "Cava",
        }
    }
}

#[derive(Clone)]
pub struct QueuedTrack {
    pub source: PlaybackSource,
    pub index: usize,
    pub track: Track,
    pub tracks_snapshot: Option<Vec<Track>>,
    pub playlist_uri: Option<String>,
    pub album_uri: Option<String>,
    pub following_user_urn: Option<String>,
    pub user_added: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FollowingTracksFocus {
    Published,
    Likes,
}

#[derive(Clone)]
pub enum EngagementAction {
    LikeTrack { track: Track, track_id: u64 },
    UnlikeTrack { track_urn: String, track_id: u64 },
    LikePlaylist { playlist: Playlist, playlist_id: u64 },
    UnlikePlaylist { tracks_uri: String, playlist_id: u64 },
    LikeAlbum { album: Album, playlist_id: u64 },
    UnlikeAlbum { tracks_uri: String, playlist_id: u64 },
    FollowUser { artist: Artist, user_id: u64 },
    UnfollowUser { urn: String, user_id: u64 },
}

#[derive(Clone)]
pub enum EngagementDone {
    LikedTrack(Track),
    UnlikedTrack { track_urn: String },
    LikedPlaylist(Playlist),
    UnlikedPlaylist { tracks_uri: String },
    LikedAlbum(Album),
    UnlikedAlbum { tracks_uri: String },
    FollowedUser(Artist),
    UnfollowedUser { urn: String },
}

pub struct AppState {
    pub selected_tab: usize,
    pub selected_subtab: usize,
    pub selected_row: usize,
    pub selected_playlist_row: usize,
    pub selected_album_row: usize,
    pub query: String,
    pub selected_searchfilter: usize,
    pub search_needs_fetch: bool,
    pub info_pane_selected: bool,
    pub selected_info_row: usize,
    pub selected_playlist_track_row: usize,
    pub selected_album_track_row: usize,
    pub selected_following_track_row: usize,
    pub selected_following_like_row: usize,
    pub search_selected_playlist_track_row: usize,
    pub search_selected_album_track_row: usize,
    pub search_selected_person_track_row: usize,
    pub search_selected_person_like_row: usize,
    pub search_people_tracks_focus: FollowingTracksFocus,
    pub playlist_tracks_request_id: u64,
    pub album_tracks_request_id: u64,
    pub following_tracks_request_id: u64,
    pub following_likes_request_id: u64,
    pub search_results_request_id: u64,
    pub search_playlist_tracks_request_id: u64,
    pub search_album_tracks_request_id: u64,
    pub search_people_tracks_request_id: u64,
    pub search_people_likes_request_id: u64,
    pub playlist_tracks_task: Option<tokio::task::JoinHandle<()>>,
    pub album_tracks_task: Option<tokio::task::JoinHandle<()>>,
    pub following_tracks_task: Option<tokio::task::JoinHandle<()>>,
    pub following_likes_task: Option<tokio::task::JoinHandle<()>>,
    pub search_results_task: Option<tokio::task::JoinHandle<()>>,
    pub search_playlist_tracks_task: Option<tokio::task::JoinHandle<()>>,
    pub search_album_tracks_task: Option<tokio::task::JoinHandle<()>>,
    pub search_people_tracks_task: Option<tokio::task::JoinHandle<()>>,
    pub search_people_likes_task: Option<tokio::task::JoinHandle<()>>,
    pub progress: u64,
    pub current_playing_index: Option<usize>,
    pub playback_source: PlaybackSource,
    pub shuffle_enabled: bool,
    pub repeat_enabled: bool,
    pub playback_history: Vec<QueuedTrack>,
    pub manual_queue: VecDeque<QueuedTrack>,
    pub auto_queue: VecDeque<usize>,
    pub override_playing: Option<QueuedTrack>,
    pub engagement_queue: VecDeque<EngagementAction>,
    pub following_tracks_focus: FollowingTracksFocus,
    pub queue_visible: bool,
    pub help_visible: bool,
    pub quit_confirm_visible: bool,
    pub quit_confirm_selected: usize,
    pub search_popup_visible: bool,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub visualizer_mode: bool,
    pub visualizer_view: VisualizerMode,
    pub end_handled_track_urn: Option<String>,
    pub preload_triggered_for_track_urn: Option<String>,
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
            search_needs_fetch: false,
            info_pane_selected: false,
            selected_info_row: 0,
            selected_playlist_track_row: 0,
            playlist_tracks_request_id: 0,
            playlist_tracks_task: None,
            selected_album_track_row: 0,
            album_tracks_request_id: 0,
            album_tracks_task: None,
            selected_following_track_row: 0,
            selected_following_like_row: 0,
            following_tracks_request_id: 0,
            following_likes_request_id: 0,
            following_tracks_task: None,
            following_likes_task: None,
            search_selected_playlist_track_row: 0,
            search_selected_album_track_row: 0,
            search_selected_person_track_row: 0,
            search_selected_person_like_row: 0,
            search_people_tracks_focus: FollowingTracksFocus::Published,
            search_results_request_id: 0,
            search_playlist_tracks_request_id: 0,
            search_album_tracks_request_id: 0,
            search_people_tracks_request_id: 0,
            search_people_likes_request_id: 0,
            search_results_task: None,
            search_playlist_tracks_task: None,
            search_album_tracks_task: None,
            search_people_tracks_task: None,
            search_people_likes_task: None,
            progress: 0,
            current_playing_index: None,
            playback_source: PlaybackSource::Likes,
            shuffle_enabled: false,
            repeat_enabled: false,
            playback_history: Vec::new(),
            manual_queue: VecDeque::new(),
            auto_queue: VecDeque::new(),
            override_playing: None,
            engagement_queue: VecDeque::new(),
            following_tracks_focus: FollowingTracksFocus::Published,
            queue_visible: false,
            help_visible: false,
            quit_confirm_visible: false,
            quit_confirm_selected: 0,
            search_popup_visible: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            visualizer_mode: false,
            visualizer_view: VisualizerMode::Oscilloscope,
            end_handled_track_urn: None,
            preload_triggered_for_track_urn: None,
        }
    }
}

pub struct AppData {
    pub likes: Vec<Track>,
    pub likes_state: TableState,
    pub liked_track_urns: HashSet<String>,
    pub playlists: Vec<Playlist>,
    pub playlists_state: TableState,
    pub liked_playlist_uris: HashSet<String>,
    pub playlist_tracks: Vec<Track>,
    pub playlist_tracks_state: TableState,
    pub playlist_tracks_uri: Option<String>,
    pub album_tracks: Vec<Track>,
    pub album_tracks_state: TableState,
    pub album_tracks_uri: Option<String>,
    pub following_tracks: Vec<Track>,
    pub following_tracks_state: TableState,
    pub following_tracks_user_urn: Option<String>,
    pub following_likes_tracks: Vec<Track>,
    pub following_likes_state: TableState,
    pub following_likes_user_urn: Option<String>,
    pub playback_tracks: Vec<Track>,
    pub playback_playlist_uri: Option<String>,
    pub playback_album_uri: Option<String>,
    pub playback_following_user_urn: Option<String>,
    pub albums: Vec<Album>,
    pub albums_state: TableState,
    pub liked_album_uris: HashSet<String>,
    pub following: Vec<Artist>,
    pub following_state: TableState,
    pub followed_user_urns: HashSet<String>,
    pub search_tracks: Vec<Track>,
    pub search_tracks_state: TableState,
    pub search_playlists: Vec<Playlist>,
    pub search_playlists_state: TableState,
    pub search_playlist_tracks: Vec<Track>,
    pub search_playlist_tracks_state: TableState,
    pub search_playlist_tracks_uri: Option<String>,
    pub search_albums: Vec<Album>,
    pub search_albums_state: TableState,
    pub search_album_tracks: Vec<Track>,
    pub search_album_tracks_state: TableState,
    pub search_album_tracks_uri: Option<String>,
    pub search_people: Vec<Artist>,
    pub search_people_state: TableState,
    pub search_people_tracks: Vec<Track>,
    pub search_people_tracks_state: TableState,
    pub search_people_tracks_user_urn: Option<String>,
    pub search_people_likes_tracks: Vec<Track>,
    pub search_people_likes_state: TableState,
    pub search_people_likes_user_urn: Option<String>,
}

impl AppData {
    pub fn new(api: &mut API, selected_row: usize) -> anyhow::Result<Self> {
        let likes: Vec<Track> = api.get_liked_tracks()?.into_iter().collect();
        let mut likes_state = TableState::default();
        likes_state.select(Some(selected_row));
        let liked_track_urns: HashSet<String> =
            likes.iter().map(|t| t.track_urn.clone()).collect();

        let playlists: Vec<Playlist> = api.get_playlists()?.into_iter().collect();
        let mut playlists_state = TableState::default();
        playlists_state.select(Some(selected_row));
        let liked_playlist_uris: HashSet<String> = playlists
            .iter()
            .filter(|p| !p.is_owned)
            .map(|p| p.tracks_uri.clone())
            .collect();

        let playlist_tracks: Vec<Track> = Vec::new();
        let mut playlist_tracks_state = TableState::default();
        playlist_tracks_state.select(Some(0));

        let album_tracks: Vec<Track> = Vec::new();
        let mut album_tracks_state = TableState::default();
        album_tracks_state.select(Some(0));

        let albums: Vec<Album> = api.get_albums()?.into_iter().collect();
        let mut albums_state = TableState::default();
        albums_state.select(Some(selected_row));
        let liked_album_uris: HashSet<String> =
            albums.iter().map(|a| a.tracks_uri.clone()).collect();

        let following_tracks: Vec<Track> = Vec::new();
        let mut following_tracks_state = TableState::default();
        following_tracks_state.select(Some(0));

        let following_likes_tracks: Vec<Track> = Vec::new();
        let mut following_likes_state = TableState::default();
        following_likes_state.select(Some(0));

        let following: Vec<Artist> = api.get_following()?.into_iter().collect();
        let mut following_state = TableState::default();
        following_state.select(Some(selected_row));
        let followed_user_urns: HashSet<String> =
            following.iter().map(|a| a.urn.clone()).collect();

        let search_tracks: Vec<Track> = Vec::new();
        let mut search_tracks_state = TableState::default();
        search_tracks_state.select(Some(0));

        let search_playlists: Vec<Playlist> = Vec::new();
        let mut search_playlists_state = TableState::default();
        search_playlists_state.select(Some(0));

        let search_playlist_tracks: Vec<Track> = Vec::new();
        let mut search_playlist_tracks_state = TableState::default();
        search_playlist_tracks_state.select(Some(0));

        let search_albums: Vec<Album> = Vec::new();
        let mut search_albums_state = TableState::default();
        search_albums_state.select(Some(0));

        let search_album_tracks: Vec<Track> = Vec::new();
        let mut search_album_tracks_state = TableState::default();
        search_album_tracks_state.select(Some(0));

        let search_people: Vec<Artist> = Vec::new();
        let mut search_people_state = TableState::default();
        search_people_state.select(Some(0));

        let search_people_tracks: Vec<Track> = Vec::new();
        let mut search_people_tracks_state = TableState::default();
        search_people_tracks_state.select(Some(0));

        let search_people_likes_tracks: Vec<Track> = Vec::new();
        let mut search_people_likes_state = TableState::default();
        search_people_likes_state.select(Some(0));

        Ok(Self {
            likes,
            likes_state,
            liked_track_urns,
            playlists,
            playlists_state,
            liked_playlist_uris,
            playlist_tracks,
            playlist_tracks_state,
            playlist_tracks_uri: None,
            album_tracks,
            album_tracks_state,
            album_tracks_uri: None,
            following_tracks,
            following_tracks_state,
            following_tracks_user_urn: None,
            following_likes_tracks,
            following_likes_state,
            following_likes_user_urn: None,
            playback_tracks: Vec::new(),
            playback_playlist_uri: None,
            playback_album_uri: None,
            playback_following_user_urn: None,
            albums,
            albums_state,
            liked_album_uris,
            following,
            following_state,
            followed_user_urns,
            search_tracks,
            search_tracks_state,
            search_playlists,
            search_playlists_state,
            search_playlist_tracks,
            search_playlist_tracks_state,
            search_playlist_tracks_uri: None,
            search_albums,
            search_albums_state,
            search_album_tracks,
            search_album_tracks_state,
            search_album_tracks_uri: None,
            search_people,
            search_people_state,
            search_people_tracks,
            search_people_tracks_state,
            search_people_tracks_user_urn: None,
            search_people_likes_tracks,
            search_people_likes_state,
            search_people_likes_user_urn: None,
        })
    }

    pub fn apply_updates(
        &mut self,
        rx_likes: &Receiver<Vec<Track>>,
        rx_playlists: &Receiver<Vec<Playlist>>,
        rx_playlist_tracks: &Receiver<(u64, Vec<Track>)>,
        rx_album_tracks: &Receiver<(u64, Vec<Track>)>,
        rx_following_tracks: &Receiver<(u64, Vec<Track>)>,
        rx_following_likes: &Receiver<(u64, Vec<Track>)>,
        rx_albums: &Receiver<Vec<Album>>,
        rx_following: &Receiver<Vec<Artist>>,
        playlist_tracks_request_id: u64,
        album_tracks_request_id: u64,
        following_tracks_request_id: u64,
        following_likes_request_id: u64,
    ) {
        while let Ok(new_likes) = rx_likes.try_recv() {
            for t in &new_likes {
                self.liked_track_urns.insert(t.track_urn.clone());
            }
            self.likes.extend(new_likes);
        }
        while let Ok(new_playlists) = rx_playlists.try_recv() {
            for p in &new_playlists {
                if !p.is_owned {
                    self.liked_playlist_uris.insert(p.tracks_uri.clone());
                }
            }
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
        while let Ok((request_id, new_tracks)) = rx_following_tracks.try_recv() {
            if request_id == following_tracks_request_id {
                self.following_tracks = new_tracks;
                self.following_tracks_state.select(Some(0));
            }
        }
        while let Ok((request_id, new_tracks)) = rx_following_likes.try_recv() {
            if request_id == following_likes_request_id {
                self.following_likes_tracks = new_tracks;
                self.following_likes_state.select(Some(0));
            }
        }
        while let Ok(new_albums) = rx_albums.try_recv() {
            for a in &new_albums {
                self.liked_album_uris.insert(a.tracks_uri.clone());
            }
            self.albums.extend(new_albums);
        }
        while let Ok(new_following) = rx_following.try_recv() {
            for a in &new_following {
                self.followed_user_urns.insert(a.urn.clone());
            }
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
