use crate::api::{API, Activity, Album, Artist, Playlist, Track};
use crate::api::Lyrics;
use crate::keymap::Keymap;
use super::nav::PaneId;
use crate::theme::{Overrides, Theme};
use ratatui::widgets::TableState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

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

/// What the lower part of the Player panel layout's panel shows.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum PanelPage {
    #[default]
    UpNext,
    Lyrics,
    Spectrum,
    History,
}

impl PanelPage {
    pub const ALL: [Self; 4] = [Self::UpNext, Self::Lyrics, Self::Spectrum, Self::History];

    pub fn label(self) -> &'static str {
        match self {
            Self::UpNext => "Up next",
            Self::Lyrics => "Lyrics",
            Self::Spectrum => "Spectrum",
            Self::History => "History",
        }
    }

    pub fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }
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

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum FollowingTracksFocus {
    Published,
    Likes,
    /// The artist name list itself, not either track pane.
    #[default]
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

/// A destructive playlist action awaiting Yes/No in the confirm popup. The playlist
/// is named by its `tracks_uri`, not its row: the list can shift while the popup is open.
#[derive(Clone)]
pub enum ConfirmAction {
    RemoveTrack { tracks_uri: String, track: Track },
    DeletePlaylist { tracks_uri: String },
}

impl ConfirmAction {
    pub fn message(&self, data: &AppData) -> String {
        let playlist_title = |tracks_uri: &str| {
            data.playlists
                .iter()
                .find(|p| p.tracks_uri == tracks_uri)
                .map(|p| p.title.as_str())
                .unwrap_or("this playlist")
        };
        match self {
            ConfirmAction::RemoveTrack { tracks_uri, track } => format!(
                "Remove \"{}\" from \"{}\"?",
                track.title,
                playlist_title(tracks_uri)
            ),
            ConfirmAction::DeletePlaylist { tracks_uri } => format!(
                "Delete playlist \"{}\"? This cannot be undone.",
                playlist_title(tracks_uri)
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
    /// The server refused this request: take the optimistic change back out of the
    /// liked / followed set and say so. Nothing else was touched yet (`apply` only
    /// runs on success), and if the user has since toggled the item back the set
    /// already reads as it should.
    pub fn failed(&self, why: &str, state: &mut AppState, data: &mut AppData) {
        let (set, key, added, what) = match self {
            Engagement::LikeTrack { track, .. } => (&mut data.liked_track_urns, &track.track_urn, true, "Like"),
            Engagement::UnlikeTrack { track_urn, .. } => (&mut data.liked_track_urns, track_urn, false, "Unlike"),
            Engagement::LikePlaylist { playlist, .. } => (&mut data.liked_playlist_uris, &playlist.tracks_uri, true, "Like"),
            Engagement::UnlikePlaylist { tracks_uri, .. } => (&mut data.liked_playlist_uris, tracks_uri, false, "Unlike"),
            Engagement::LikeAlbum { album, .. } => (&mut data.liked_album_uris, &album.tracks_uri, true, "Like"),
            Engagement::UnlikeAlbum { tracks_uri, .. } => (&mut data.liked_album_uris, tracks_uri, false, "Unlike"),
            Engagement::FollowUser { artist, .. } => (&mut data.followed_user_urns, &artist.urn, true, "Follow"),
            Engagement::UnfollowUser { urn, .. } => (&mut data.followed_user_urns, urn, false, "Unfollow"),
        };
        if added {
            set.remove(key);
        } else {
            set.insert(key.clone());
        }
        state.toast_err(format!("{what} failed: {why}"));
    }

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
    /// Cursor to put back once the rows being fetched arrive (see `refetch`).
    pub restore: Option<usize>,
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
    /// The pane's cursor is remembered per parent: leaving a key files `row` under
    /// (`pane`, key) in `cursors`, and coming back restores it once the rows are in.
    /// Returns true when the pane was reset (refetched or cleared).
    #[allow(clippy::too_many_arguments)]
    pub fn refetch(
        &mut self,
        pane: PaneId,
        cursors: &mut HashMap<(PaneId, String), usize>,
        tracks: &mut Vec<Track>,
        table: &mut TableState,
        key: &mut Option<String>,
        row: &mut usize,
        selected: Option<String>,
        spawn: impl FnOnce(u64, String) -> tokio::task::JoinHandle<()>,
    ) -> bool {
        let same = selected.is_some() && selected == *key;
        if !same && let Some(old_key) = key.take() {
            // Rows that never arrived leave the cursor they were waiting for on file.
            cursors.insert((pane, old_key), self.restore.take().unwrap_or(*row));
        }
        let reset = match selected {
            Some(_) if same => false,
            Some(new_key) => {
                self.cancel();
                self.restore = cursors.get(&(pane, new_key.clone())).copied();
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
        if !tracks.is_empty()
            && let Some(saved) = self.restore.take()
        {
            *row = saved;
            table.select(Some(*row));
        }
        if !tracks.is_empty() && *row >= tracks.len() {
            *row = tracks.len() - 1;
            table.select(Some(*row));
        }
        reset
    }
}

/// A movement key still counts as held while its repeats keep arriving this close together;
/// a longer pause is a tap. Terminal key-repeat rates are ~15-30/s, so this is generous.
const HOLD_GAP: Duration = Duration::from_millis(150);
/// Grace period before a hold starts accelerating, so short holds stay one row per repeat.
const RAMP_DELAY: Duration = Duration::from_millis(800);
/// How long the ramp from one row to [`MAX_STEP`] takes once it starts.
const RAMP_SPAN: Duration = Duration::from_millis(1200);
/// Fastest step, matching the Alt+Up/Down page jump.
const MAX_STEP: usize = 10;

/// The current run of same-direction movement keypresses, so a held key scrolls
/// faster the longer it is held.
#[derive(Default)]
pub struct MoveAccel {
    /// Pane, direction, when the run started, and when its last press arrived.
    run: Option<(PaneId, isize, Instant, Instant)>,
}

impl MoveAccel {
    /// Records a movement press in pane `lane` in `dir` at `now` and returns how many rows
    /// it should move. A pause longer than [`HOLD_GAP`], a change of direction, or a change
    /// of pane starts a new run at one row.
    // ponytail: crossterm events carry no arrival timestamp, so `now` is when the press is
    // handled, not when it was typed. A frame that stalls for longer than HOLD_GAP therefore
    // resets the ramp (it falls back to single rows, never overshoots). Upgrade path: stamp
    // each event with `Instant::now()` in the poll loop and pass that in.
    pub fn step_rows(&mut self, lane: PaneId, dir: isize, now: Instant) -> usize {
        let started = match self.run {
            Some((l, d, started, last))
                if l == lane && d == dir && now.duration_since(last) <= HOLD_GAP =>
            {
                started
            }
            _ => now,
        };
        self.run = Some((lane, dir, started, now));
        let ramped = now.duration_since(started).saturating_sub(RAMP_DELAY);
        let progress = (ramped.as_secs_f32() / RAMP_SPAN.as_secs_f32()).min(1.0);
        1 + ((MAX_STEP - 1) as f32 * progress) as usize
    }
}

/// Pages of the `?` overlay, in the order Tab walks them.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HelpPage {
    #[default]
    Keys,
    Settings,
    Equalizer,
}

impl HelpPage {
    /// The popup's tab bar, in order.
    pub const TITLES: [&'static str; 3] = ["Keys", "Settings", "Equaliser"];

    pub fn index(self) -> usize {
        match self {
            Self::Keys => 0,
            Self::Settings => 1,
            Self::Equalizer => 2,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Keys => Self::Settings,
            Self::Settings => Self::Equalizer,
            Self::Equalizer => Self::Keys,
        }
    }
}

#[derive(Default)]
pub struct AppState {
    pub selected_tab: usize,
    pub selected_subtab: usize,
    /// Cursor of the first pane of the location on screen. Like every `*_row` field
    /// below it is storage for `nav`; read and write cursors through `nav`.
    pub selected_row: usize,
    /// Focused pane index per location, `[main tab][sub-tab]`. Read through `nav::focus`.
    pub focus: [[usize; 4]; 3],
    /// The navigation column (Sidebar, Columns) has focus instead of a pane. Read it
    /// through `nav::in_nav`, which also checks the layout has one.
    pub nav_focused: bool,
    /// On Library › Playlists, the navigation column's cursor is on the `Playlists`
    /// entry itself rather than on the selected playlist beneath it.
    pub nav_on_header: bool,
    /// Where each child pane's cursor was for each parent item it has shown (playlist
    /// or album `tracks_uri`, user urn, feed key). Written by `FetchTask::refetch`.
    pub child_cursors: HashMap<(PaneId, String), usize>,
    /// `selected_row` of each location that is not on screen, `[main tab][sub-tab]`.
    pub primary_rows: [[usize; 4]; 3],
    /// Visible rows of the focused pane, written by the renderer; 0 until it has drawn
    /// (`nav::page_rows` then answers 10).
    pub page_rows: usize,
    /// The page the Player panel layout's panel is on. Only `Action::PanelPage` moves it.
    pub panel_page: PanelPage,
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
    /// Which page of the `?` popup is showing (Tab cycles), and the highlighted
    /// row on each of the pages behind the key list.
    pub help_page: HelpPage,
    pub help_settings_selected: usize,
    pub help_eq_selected: usize,
    /// Equaliser gains in dB; `player::eq` holds the copy the audio thread reads.
    pub eq: crate::config::Equalizer,
    /// The action the key editor is capturing a binding for. The keys captured so far
    /// wait in `pending_keys`.
    pub help_capture: Option<crate::keymap::Action>,
    /// Keys of a sequence typed so far (`g` of `g g`) and when the last one arrived.
    /// Fed through `input::helpers::lookup`; the loop drops them after a second.
    pub pending_keys: Vec<crate::keymap::Chord>,
    pub pending_since: Option<Instant>,
    pub help_message: Option<String>,
    /// Notices on screen, oldest first. Added with `toast` / `toast_err`; the loop
    /// drops each when its time is up.
    pub toasts: Vec<super::toast::Toast>,
    /// `config::Loaded::load_failed`: saving the config is refused while set.
    pub config_load_failed: bool,
    /// Insert mode: keys type into the Search query (Search tab) or the Library
    /// filter (`search_popup_visible`) until Enter/Esc.
    pub search_typing: bool,
    pub queue_visible: bool,
    /// Upcoming tracks scrolled off the top of the queue popup.
    pub queue_scroll: usize,
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
    /// The command palette, while it is open.
    pub palette: Option<super::palette::PaletteState>,
    /// The palette's `Setup wizard` command was run; the wizard picks this up.
    pub wizard_requested: bool,
    pub quit_confirm_visible: bool,
    pub quit_confirm_selected: usize,
    /// A Library filter exists (being typed or kept); its box is drawn.
    pub search_popup_visible: bool,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    /// Row and identity (urn / tracks_uri) of the highlighted filtered row as of the last
    /// loop pass, so the highlight can follow that item when the list changes under it.
    pub filter_anchor: Option<(usize, String)>,
    pub visualizer_mode: bool,
    pub visualizer_view: VisualizerMode,
    /// Ramp for held Up/Down, so long lists scroll faster the longer the key is held.
    pub move_accel: MoveAccel,
    pub end_handled_track_urn: Option<String>,
    pub preload_triggered_for_track_urn: Option<String>,
}

#[derive(Default)]
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
    /// Rows taken out by the "hide unplayable tracks" setting, each with the index it
    /// held in its full list, so switching the setting back off puts them back where
    /// they were. One slot per pane, in the order `filtering::apply_unplayable_filter`
    /// lists them.
    pub hidden_tracks: Vec<Vec<(usize, Track)>>,
    pub hidden_feed: Vec<(usize, Activity)>,
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
            hidden_tracks: Vec::new(),
            hidden_feed: Vec::new(),
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// T11: a refused like or follow is taken back and reported, not left looking done.
    #[test]
    fn engagement_failure_rolls_back_and_toasts() {
        use crate::tui::logic::toast::ToastKind;
        let mut state = AppState::default();
        let mut data = AppData::default();
        let artist = Artist { name: "Someone".into(), urn: "soundcloud:users:7".into() };
        // What the follow key does, then the server saying no.
        data.followed_user_urns.insert(artist.urn.clone());
        Engagement::FollowUser { artist: artist.clone(), user_id: 7 }.failed("403 Forbidden", &mut state, &mut data);
        assert!(data.followed_user_urns.is_empty() && data.following.is_empty());
        let toast = state.toasts.last().unwrap();
        assert_eq!((toast.kind, toast.msg.as_str()), (ToastKind::Err, "Follow failed: 403 Forbidden"));
        // A refused unfollow or unlike puts the item back.
        Engagement::UnfollowUser { urn: artist.urn.clone(), user_id: 7 }.failed("offline", &mut state, &mut data);
        assert!(data.followed_user_urns.contains(&artist.urn));
        Engagement::UnlikeTrack { track_urn: "soundcloud:tracks:1".into(), track_id: 1 }.failed("offline", &mut state, &mut data);
        assert!(data.liked_track_urns.contains("soundcloud:tracks:1"));
        assert_eq!(state.toasts.last().unwrap().msg, "Unlike failed: offline");
    }

    /// Identity, not position: each parent item has its own cursor in the child pane.
    #[test]
    fn child_cursor_is_remembered_per_parent() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut state = AppState::default();
        let mut data = AppData::default();
        let rows = |n: usize| -> Vec<Track> {
            let track = |i| Track {
                title: format!("t{i}"),
                artists: String::new(),
                duration: String::new(),
                duration_ms: 0,
                playback_count: String::new(),
                artwork_url: String::new(),
                access: String::new(),
                track_urn: format!("soundcloud:tracks:{i}"),
            };
            (0..n).map(track).collect()
        };
        // One pass of the loop over Library › Playlists with playlist `uri` selected.
        let pass = |state: &mut AppState, data: &mut AppData, uri: Option<&str>| {
            state.playlist_tracks_fetch.refetch(
                PaneId::PlaylistTracks,
                &mut state.child_cursors,
                &mut data.playlist_tracks,
                &mut data.playlist_tracks_state,
                &mut data.playlist_tracks_uri,
                &mut state.selected_playlist_track_row,
                uri.map(str::to_string),
                |_, _| rt.spawn(async {}),
            )
        };

        // Playlist A: its rows arrive, the cursor goes to row 5.
        assert!(pass(&mut state, &mut data, Some("a")));
        data.playlist_tracks = rows(10);
        assert!(!pass(&mut state, &mut data, Some("a")));
        state.selected_playlist_track_row = 5;
        // Playlist B starts at the top, and gets its own cursor.
        assert!(pass(&mut state, &mut data, Some("b")));
        assert_eq!((state.selected_playlist_track_row, data.playlist_tracks.len()), (0, 0));
        data.playlist_tracks = rows(4);
        pass(&mut state, &mut data, Some("b"));
        assert_eq!(state.selected_playlist_track_row, 0);
        state.selected_playlist_track_row = 2;
        // Back to A: row 0 while loading, row 5 once its rows are in.
        assert!(pass(&mut state, &mut data, Some("a")));
        assert_eq!(state.selected_playlist_track_row, 0);
        pass(&mut state, &mut data, Some("a"));
        assert_eq!(state.selected_playlist_track_row, 0, "nothing to restore onto yet");
        data.playlist_tracks = rows(10);
        pass(&mut state, &mut data, Some("a"));
        assert_eq!(state.selected_playlist_track_row, 5);
        assert_eq!(data.playlist_tracks_state.selected(), Some(5));
        // Restored once: the cursor is the user's again afterwards.
        state.selected_playlist_track_row = 7;
        pass(&mut state, &mut data, Some("a"));
        assert_eq!(state.selected_playlist_track_row, 7);

        // Leaving before the rows arrive does not forget where the cursor was.
        pass(&mut state, &mut data, Some("b"));
        pass(&mut state, &mut data, Some("a"));
        pass(&mut state, &mut data, Some("b"));
        data.playlist_tracks = rows(4);
        pass(&mut state, &mut data, Some("b"));
        assert_eq!(state.selected_playlist_track_row, 2);
        pass(&mut state, &mut data, Some("a"));
        data.playlist_tracks = rows(10);
        pass(&mut state, &mut data, Some("a"));
        assert_eq!(state.selected_playlist_track_row, 7);
        // A list that came back shorter clamps the remembered row.
        pass(&mut state, &mut data, Some("b"));
        pass(&mut state, &mut data, Some("a"));
        data.playlist_tracks = rows(3);
        pass(&mut state, &mut data, Some("a"));
        assert_eq!(state.selected_playlist_track_row, 2);
        // Nothing selected clears the pane; the cursor is still on file for later.
        assert!(pass(&mut state, &mut data, None));
        assert_eq!((state.selected_playlist_track_row, data.playlist_tracks_uri.clone()), (0, None));
        assert_eq!(state.child_cursors.get(&(PaneId::PlaylistTracks, "a".to_string())), Some(&2));
        // Another pane's cursor for the same key is its own.
        assert_eq!(state.child_cursors.get(&(PaneId::SearchPlaylistTracks, "a".to_string())), None);
    }

    #[test]
    fn held_movement_key_ramps_then_resets() {
        let mut accel = MoveAccel::default();
        let t0 = Instant::now();
        let press = |accel: &mut MoveAccel, dir, ms: u64| {
            accel.step_rows(PaneId::Likes, dir, t0 + Duration::from_millis(ms))
        };
        // Full speed is reached RAMP_DELAY + RAMP_SPAN after the run starts.
        let to_max = (RAMP_DELAY + RAMP_SPAN).as_millis() as u64;

        // A tap, and the start of a hold, move a single row.
        assert_eq!(press(&mut accel, 1, 0), 1);
        assert_eq!(press(&mut accel, 1, 100), 1);

        // Keep the repeats coming at 50ms: the step ramps up and tops out at MAX_STEP.
        let rows: Vec<usize> = (150..=to_max).step_by(50).map(|ms| press(&mut accel, 1, ms)).collect();
        assert!(rows.windows(2).all(|w| w[0] <= w[1]), "not monotonic: {rows:?}");
        assert!(rows.iter().any(|&r| r > 1 && r < MAX_STEP), "no ramp: {rows:?}");
        assert_eq!(rows.last(), Some(&MAX_STEP));
        // The grace period really is one: a hold shorter than RAMP_DELAY never speeds up.
        let mut grace = MoveAccel::default();
        let grace_rows: Vec<usize> = (0..=RAMP_DELAY.as_millis() as u64)
            .step_by(50)
            .map(|ms| grace.step_rows(PaneId::Likes, 1, t0 + Duration::from_millis(ms)))
            .collect();
        assert!(grace_rows.iter().all(|&r| r == 1), "accelerated too early: {grace_rows:?}");

        // Letting go (a gap longer than HOLD_GAP) drops back to single rows...
        let released = to_max + HOLD_GAP.as_millis() as u64 + 1;
        assert_eq!(press(&mut accel, 1, released), 1);
        // ...and so does reversing direction mid-hold.
        let resumed = released + 50;
        let rows: Vec<usize> = (resumed..=resumed + to_max)
            .step_by(50)
            .map(|ms| press(&mut accel, 1, ms))
            .collect();
        assert_eq!(rows.last(), Some(&MAX_STEP));
        let reversed = resumed + to_max + 50;
        assert_eq!(press(&mut accel, -1, reversed), 1);

        // A different cursor starts its own run, and coming back starts the first one over.
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut lane_ms = reversed;
        for _ in 0..((to_max / 50) + 2) {
            accel.step_rows(PaneId::Playlists, 1, at(lane_ms));
            lane_ms += 50;
        }
        assert_eq!(accel.step_rows(PaneId::Playlists, 1, at(lane_ms)), MAX_STEP);
        assert_eq!(accel.step_rows(PaneId::PlaylistTracks, 1, at(lane_ms + 50)), 1);
        assert_eq!(accel.step_rows(PaneId::Playlists, 1, at(lane_ms + 100)), 1);
    }
}
