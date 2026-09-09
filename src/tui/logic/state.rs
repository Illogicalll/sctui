use crate::api::{API, Activity, Album, Artist, Playlist, Track};
use crate::api::Lyrics;
use crate::keymap::Keymap;
use crate::theme::{Overrides, Theme};
use ratatui::widgets::TableState;
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackSource {
    #[default]
    Likes,
    Playlist,
    Album,
    FollowingPublished,
    FollowingLikes,
    Feed,
    /// Related-track chain started by Shift+Enter or by a list running out.
    Radio,
}

/// Lyrics for the track that is playing.
#[derive(Clone, Default)]
pub enum LyricsStatus {
    #[default]
    Idle,
    Loading,
    NotFound,
    Found(Lyrics),
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum VisualizerMode {
    #[default]
    Oscilloscope,
    SpectrumBars,
    MirrorSpectrum,
    RadialSpectrum,
    FilledCurve,
    LedMatrix,
    Ridgeline,
    SpectrumRings,
    Seismograph,
    StackedScope,
    ParticleFountain,
    InterferenceField,
    NowPlaying,
    Lyrics,
}

impl VisualizerMode {
    pub const ALL: [Self; 14] = [
        Self::Oscilloscope,
        Self::SpectrumBars,
        Self::MirrorSpectrum,
        Self::RadialSpectrum,
        Self::FilledCurve,
        Self::LedMatrix,
        Self::Ridgeline,
        Self::SpectrumRings,
        Self::Seismograph,
        Self::StackedScope,
        Self::ParticleFountain,
        Self::InterferenceField,
        Self::NowPlaying,
        Self::Lyrics,
    ];

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|m| *m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
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

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum FollowingTracksFocus {
    #[default]
    Published,
    Likes,
    /// The artist name list itself, not either track pane.
    Artists,
}

/// A like/follow request; the same value comes back once the server accepted it
/// and `apply` folds it into the local lists.
#[derive(Clone)]
pub enum Engagement {
    LikeTrack { track: Track, track_id: u64 },
    UnlikeTrack { track_urn: String, track_id: u64 },
    LikePlaylist { playlist: Playlist, playlist_id: u64 },
    UnlikePlaylist { tracks_uri: String, playlist_id: u64 },
    LikeAlbum { album: Album, playlist_id: u64 },
    UnlikeAlbum { tracks_uri: String, playlist_id: u64 },
    FollowUser { artist: Artist, user_id: u64 },
    UnfollowUser { urn: String, user_id: u64 },
}

/// A destructive playlist action awaiting Yes/No in the confirm popup.
#[derive(Clone)]
pub enum ConfirmAction {
    RemoveTrack { playlist_idx: usize, track: Track },
    DeletePlaylist { playlist_idx: usize },
}

impl ConfirmAction {
    pub fn message(&self, data: &AppData) -> String {
        let playlist_title = |idx: usize| {
            data.playlists
                .get(idx)
                .map(|p| p.title.as_str())
                .unwrap_or("this playlist")
        };
        match self {
            ConfirmAction::RemoveTrack { playlist_idx, track } => format!(
                "Remove \"{}\" from \"{}\"?",
                track.title,
                playlist_title(*playlist_idx)
            ),
            ConfirmAction::DeletePlaylist { playlist_idx } => format!(
                "Delete playlist \"{}\"? This cannot be undone.",
                playlist_title(*playlist_idx)
            ),
        }
    }
}

/// A playlist write waiting to be sent. The UI has already been updated.
pub enum PlaylistEdit {
    Add { playlist_id: u64, tracks_uri: String, track_urn: String },
    Remove { playlist_id: u64, tracks_uri: String, track_urn: String },
    Create { title: String, track_urn: Option<String>, public: bool },
    Delete { playlist_id: u64 },
}

/// Clamp a list's cursor after rows were removed from it.
pub(crate) fn clamp_row(row: &mut usize, table: &mut TableState, len: usize) {
    if len == 0 {
        *row = 0;
    } else if *row >= len {
        *row = len - 1;
    } else {
        return;
    }
    table.select(Some(*row));
}

impl Engagement {
    /// Skips when the user has since toggled the same item back (its optimistic set no
    /// longer agrees with this result).
    pub fn apply(self, state: &mut AppState, data: &mut AppData) {
        let library = |subtab: usize| state.selected_tab == 0 && state.selected_subtab == subtab;
        match self {
            Engagement::LikeTrack { track, .. } => {
                if !data.liked_track_urns.contains(&track.track_urn) {
                    return;
                }
                if !data.likes.iter().any(|t| t.track_urn == track.track_urn) {
                    data.likes.insert(0, track);
                }
            }
            Engagement::UnlikeTrack { track_urn, .. } => {
                if data.liked_track_urns.contains(&track_urn) {
                    return;
                }
                data.likes.retain(|t| t.track_urn != track_urn);
                if library(0) {
                    clamp_row(&mut state.selected_row, &mut data.likes_state, data.likes.len());
                }
            }
            Engagement::LikePlaylist { playlist, .. } => {
                if !data.liked_playlist_uris.contains(&playlist.tracks_uri) {
                    return;
                }
                let exists = data
                    .playlists
                    .iter()
                    .any(|p| p.tracks_uri == playlist.tracks_uri && !p.is_owned);
                if !exists {
                    data.playlists.insert(0, playlist);
                }
            }
            Engagement::UnlikePlaylist { tracks_uri, .. } => {
                if data.liked_playlist_uris.contains(&tracks_uri) {
                    return;
                }
                data.playlists
                    .retain(|p| !(p.tracks_uri == tracks_uri && !p.is_owned));
                if library(1) {
                    clamp_row(&mut state.selected_row, &mut data.playlists_state, data.playlists.len());
                }
            }
            Engagement::LikeAlbum { album, .. } => {
                if !data.liked_album_uris.contains(&album.tracks_uri) {
                    return;
                }
                if !data.albums.iter().any(|a| a.tracks_uri == album.tracks_uri) {
                    data.albums.insert(0, album);
                }
            }
            Engagement::UnlikeAlbum { tracks_uri, .. } => {
                if data.liked_album_uris.contains(&tracks_uri) {
                    return;
                }
                data.albums.retain(|a| a.tracks_uri != tracks_uri);
                if library(2) {
                    clamp_row(&mut state.selected_row, &mut data.albums_state, data.albums.len());
                }
            }
            Engagement::FollowUser { artist, .. } => {
                if !data.followed_user_urns.contains(&artist.urn) {
                    return;
                }
                if !data.following.iter().any(|a| a.urn == artist.urn) {
                    data.following.insert(0, artist);
                }
            }
            Engagement::UnfollowUser { urn, .. } => {
                if data.followed_user_urns.contains(&urn) {
                    return;
                }
                data.following.retain(|a| a.urn != urn);
                if library(3) {
                    clamp_row(&mut state.selected_row, &mut data.following_state, data.following.len());
                }
            }
        }
    }
}

/// One in-flight fetch: the id that gates its result and the handle used to abort it.
#[derive(Default)]
pub struct FetchTask {
    pub request_id: u64,
    pub task: Option<tokio::task::JoinHandle<()>>,
}

impl FetchTask {
    /// Aborts the running fetch (if any) so its late result is ignored.
    pub fn cancel(&mut self) {
        if let Some(handle) = self.task.take() {
            handle.abort();
        }
        self.request_id = self.request_id.wrapping_add(1);
    }

    /// Stores a result if it belongs to the current request.
    pub fn accept<T>(
        &self,
        request_id: u64,
        items: Vec<T>,
        dst: &mut Vec<T>,
        table: &mut TableState,
    ) {
        if request_id == self.request_id {
            *dst = items;
            table.select(Some(0));
        }
    }

    /// Fetch-on-select for one tracks pane. `selected` is the key (tracks_uri or user urn)
    /// of the highlighted parent row, `None` when there is none. Refetches when the key
    /// changed, clears the pane when nothing is selected, then clamps `row`.
    /// Returns true when the pane was reset (refetched or cleared).
    pub fn refetch(
        &mut self,
        tracks: &mut Vec<Track>,
        table: &mut TableState,
        key: &mut Option<String>,
        row: &mut usize,
        selected: Option<String>,
        spawn: impl FnOnce(u64, String) -> tokio::task::JoinHandle<()>,
    ) -> bool {
        let reset = match selected {
            Some(new_key) if key.as_deref() == Some(new_key.as_str()) => false,
            Some(new_key) => {
                self.cancel();
                *key = Some(new_key.clone());
                tracks.clear();
                table.select(Some(0));
                *row = 0;
                self.task = Some(spawn(self.request_id, new_key));
                true
            }
            None => {
                tracks.clear();
                table.select(Some(0));
                *key = None;
                *row = 0;
                true
            }
        };
        if !tracks.is_empty() && *row >= tracks.len() {
            *row = tracks.len() - 1;
            table.select(Some(*row));
        }
        reset
    }
}

#[derive(Default)]
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
    pub playlist_tracks_fetch: FetchTask,
    pub album_tracks_fetch: FetchTask,
    pub following_tracks_fetch: FetchTask,
    pub following_likes_fetch: FetchTask,
    pub search_results_fetch: FetchTask,
    pub search_playlist_tracks_fetch: FetchTask,
    pub search_album_tracks_fetch: FetchTask,
    pub search_people_tracks_fetch: FetchTask,
    pub search_people_likes_fetch: FetchTask,
    pub feed_tracks_fetch: FetchTask,
    /// Background walk over later feed items that keeps extending the queue after Enter.
    pub feed_queue_fetch: FetchTask,
    pub radio_fetch: FetchTask,
    /// Seed URN of the last related-tracks fetch, so each seed is fetched once.
    pub radio_fetched_for: Option<String>,
    /// The radio queue ran dry before related tracks arrived; play as soon as they do.
    pub radio_waiting: bool,
    /// Set by Shift+Enter on a focused artist row; the loop fetches its station tracks from here.
    pub artist_station_request: Option<Artist>,
    pub artist_station_fetch: FetchTask,
    /// Set by Enter in the feed: index of the activity being played; the loop expands from there.
    pub feed_expand_from: Option<usize>,
    pub progress: u64,
    pub tick: f64,
    pub current_playing_index: Option<usize>,
    pub playback_source: PlaybackSource,
    pub shuffle_enabled: bool,
    pub repeat_enabled: bool,
    pub playback_history: Vec<QueuedTrack>,
    pub manual_queue: VecDeque<QueuedTrack>,
    pub auto_queue: VecDeque<usize>,
    pub override_playing: Option<QueuedTrack>,
    pub engagement_queue: VecDeque<Engagement>,
    pub following_tracks_focus: FollowingTracksFocus,
    pub keymap: Keymap,
    pub settings: crate::config::Settings,
    pub theme_name: String,
    pub theme_overrides: Overrides,
    pub theme_picker_visible: bool,
    pub theme_picker_selected: usize,
    /// Theme to restore if the picker is cancelled.
    pub theme_picker_previous: Option<Theme>,
    pub lyrics: LyricsStatus,
    /// Track the lyrics above belong to (or are being fetched for).
    pub lyrics_track_urn: Option<String>,
    /// Key editor (the `?` popup): highlighted action, pending capture, last message.
    pub help_selected: usize,
    /// The `?` popup's settings page (Tab switches to it) and its highlighted row.
    pub help_settings: bool,
    pub help_settings_selected: usize,
    pub help_capture: Option<crate::keymap::Action>,
    pub help_message: Option<String>,
    /// Search tab: printable keys go to the query until Enter/Esc.
    pub search_typing: bool,
    pub queue_visible: bool,
    pub history_visible: bool,
    pub playlist_picker_visible: bool,
    /// Row in the picker: 0 = "+ New playlist…", then owned playlists in order.
    pub playlist_picker_selected: usize,
    /// `Some` while a new playlist name is being typed.
    pub playlist_picker_title: Option<String>,
    pub playlist_picker_track: Option<Track>,
    /// Visibility chosen in the new-playlist prompt. Private by default.
    pub playlist_picker_public: bool,
    pub confirm: Option<ConfirmAction>,
    /// 0 = Yes, 1 = No.
    pub confirm_selected: usize,
    pub playlist_edit_queue: VecDeque<PlaylistEdit>,
    /// Row in the history popup, 0 = newest.
    pub history_selected: usize,
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
    pub feed: Vec<Activity>,
    pub feed_state: TableState,
    pub feed_tracks: Vec<Track>,
    pub feed_tracks_state: TableState,
    pub feed_tracks_key: Option<String>,
}

impl AppData {
    pub fn new(api: &mut API, selected_row: usize) -> anyhow::Result<Self> {
        let likes: Vec<Track> = api.get_liked_tracks()?.into_iter().collect();
        let liked_track_urns: HashSet<String> =
            likes.iter().map(|t| t.track_urn.clone()).collect();

        let playlists: Vec<Playlist> = api.get_playlists()?.into_iter().collect();
        let liked_playlist_uris: HashSet<String> = playlists
            .iter()
            .filter(|p| !p.is_owned)
            .map(|p| p.tracks_uri.clone())
            .collect();

        let albums: Vec<Album> = api.get_albums()?.into_iter().collect();
        let liked_album_uris: HashSet<String> =
            albums.iter().map(|a| a.tracks_uri.clone()).collect();

        let following: Vec<Artist> = api.get_following()?.into_iter().collect();
        let followed_user_urns: HashSet<String> =
            following.iter().map(|a| a.urn.clone()).collect();

        Ok(Self {
            likes,
            likes_state: TableState::default().with_selected(selected_row),
            liked_track_urns,
            playlists,
            playlists_state: TableState::default().with_selected(selected_row),
            liked_playlist_uris,
            playlist_tracks: Vec::new(),
            playlist_tracks_state: TableState::default().with_selected(0),
            playlist_tracks_uri: None,
            album_tracks: Vec::new(),
            album_tracks_state: TableState::default().with_selected(0),
            album_tracks_uri: None,
            following_tracks: Vec::new(),
            following_tracks_state: TableState::default().with_selected(0),
            following_tracks_user_urn: None,
            following_likes_tracks: Vec::new(),
            following_likes_state: TableState::default().with_selected(0),
            following_likes_user_urn: None,
            playback_tracks: Vec::new(),
            playback_playlist_uri: None,
            playback_album_uri: None,
            playback_following_user_urn: None,
            albums,
            albums_state: TableState::default().with_selected(selected_row),
            liked_album_uris,
            following,
            following_state: TableState::default().with_selected(selected_row),
            followed_user_urns,
            search_tracks: Vec::new(),
            search_tracks_state: TableState::default().with_selected(0),
            search_playlists: Vec::new(),
            search_playlists_state: TableState::default().with_selected(0),
            search_playlist_tracks: Vec::new(),
            search_playlist_tracks_state: TableState::default().with_selected(0),
            search_playlist_tracks_uri: None,
            search_albums: Vec::new(),
            search_albums_state: TableState::default().with_selected(0),
            search_album_tracks: Vec::new(),
            search_album_tracks_state: TableState::default().with_selected(0),
            search_album_tracks_uri: None,
            search_people: Vec::new(),
            search_people_state: TableState::default().with_selected(0),
            search_people_tracks: Vec::new(),
            search_people_tracks_state: TableState::default().with_selected(0),
            search_people_tracks_user_urn: None,
            search_people_likes_tracks: Vec::new(),
            search_people_likes_state: TableState::default().with_selected(0),
            search_people_likes_user_urn: None,
            feed: Vec::new(),
            feed_state: TableState::default().with_selected(0),
            feed_tracks: Vec::new(),
            feed_tracks_state: TableState::default().with_selected(0),
            feed_tracks_key: None,
        })
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

pub const TAB_TITLES: [&str; 3] = ["Library", "Search", "Feed"];
pub const SUBTAB_TITLES: [&str; 4] = ["Likes", "Playlists", "Albums", "Following"];
pub const SEARCHFILTERS: [&str; 4] = ["Tracks", "Albums", "Playlists", "People"];

/// The tabs the user can reach. Feed is the last one, so hiding it is a shorter slice.
pub fn visible_tabs(state: &AppState) -> &'static [&'static str] {
    if state.settings.hide_feed_tab {
        &TAB_TITLES[..2]
    } else {
        &TAB_TITLES
    }
}
