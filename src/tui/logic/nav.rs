//! The navigation model: where the user is (main tab + sub-tab), which panes that
//! location shows, which pane has focus, and every pane's cursor. All movement and all
//! selection-based actions go through here; the `*_row` fields on `AppState` are
//! storage only, and nothing outside this module writes the legacy focus flags.

use ratatui::widgets::TableState;

use crate::api::{Playlist, Track};
use crate::config::Layout;

use super::filtering::{FilteredViews, clear_filter, is_filter_active};
use super::state::{AppData, AppState, FollowingTracksFocus, SUBTAB_TITLES, TAB_TITLES};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaneKind {
    Tracks,
    Playlists,
    Albums,
    People,
    Feed,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PaneId {
    Likes,
    Playlists,
    PlaylistTracks,
    Albums,
    AlbumTracks,
    Following,
    FollowingTracks,
    FollowingLikes,
    SearchTracks,
    SearchAlbums,
    SearchAlbumTracks,
    SearchPlaylists,
    SearchPlaylistTracks,
    SearchPeople,
    SearchPeopleTracks,
    SearchPeopleLikes,
    Feed,
    FeedTracks,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
    Top,
    Bottom,
}

use PaneId::*;

impl PaneId {
    pub const ALL: [PaneId; 18] = [
        Likes, Playlists, PlaylistTracks, Albums, AlbumTracks, Following, FollowingTracks,
        FollowingLikes, SearchTracks, SearchAlbums, SearchAlbumTracks, SearchPlaylists,
        SearchPlaylistTracks, SearchPeople, SearchPeopleTracks, SearchPeopleLikes, Feed,
        FeedTracks,
    ];

    pub fn kind(self) -> PaneKind {
        match self {
            Playlists | SearchPlaylists => PaneKind::Playlists,
            Albums | SearchAlbums => PaneKind::Albums,
            Following | SearchPeople => PaneKind::People,
            Feed => PaneKind::Feed,
            _ => PaneKind::Tracks,
        }
    }

    /// A pane whose rows open into the pane to its right rather than play.
    pub fn is_container(self) -> bool {
        self.kind() != PaneKind::Tracks
    }
}

const LIBRARY: [&[PaneId]; 4] = [
    &[Likes],
    &[Playlists, PlaylistTracks],
    &[Albums, AlbumTracks],
    &[Following, FollowingTracks, FollowingLikes],
];
const SEARCH: [&[PaneId]; 4] = [
    &[SearchTracks],
    &[SearchAlbums, SearchAlbumTracks],
    &[SearchPlaylists, SearchPlaylistTracks],
    &[SearchPeople, SearchPeopleTracks, SearchPeopleLikes],
];
const FEED: &[PaneId] = &[Feed, FeedTracks];

fn panes_at(tab: usize, sub: usize) -> &'static [PaneId] {
    match tab {
        0 => LIBRARY[sub.min(3)],
        1 => SEARCH[sub.min(3)],
        _ => FEED,
    }
}

/// Main tab and its sub-tab (the search type on Search, always 0 on Feed).
pub fn location(state: &AppState) -> (usize, usize) {
    match state.selected_tab {
        0 => (0, state.selected_subtab.min(3)),
        1 => (1, state.selected_searchfilter.min(3)),
        _ => (2, 0),
    }
}

/// The panes of the current location, left to right.
pub fn panes(state: &AppState) -> &'static [PaneId] {
    let (tab, sub) = location(state);
    panes_at(tab, sub)
}

/// Index into [`panes`] of the focused pane; never a pane the layout does not draw.
pub fn focus(state: &AppState) -> usize {
    let (tab, sub) = location(state);
    state.focus[tab][sub].min(panes_at(tab, sub).len() - 1).max(first_pane(state))
}

/// The first pane the layout draws: 1 on Library › Playlists in Sidebar, whose
/// navigation column lists the playlists itself, otherwise 0.
pub fn first_pane(state: &AppState) -> usize {
    (state.settings.layout == Layout::Sidebar && location(state) == (0, 1)) as usize
}

pub fn focused(state: &AppState) -> PaneId {
    panes(state)[focus(state)]
}

pub fn set_focus(state: &mut AppState, idx: usize) {
    let (tab, sub) = location(state);
    state.focus[tab][sub] = idx.min(panes_at(tab, sub).len() - 1);
    sync_flags(state);
}

/// Focus the pane to the left; `false` at the edge.
pub fn focus_left(state: &mut AppState) -> bool {
    let at = focus(state);
    if at <= first_pane(state) {
        return false;
    }
    set_focus(state, at - 1);
    true
}

/// Focus the pane to the right; `false` at the edge.
pub fn focus_right(state: &mut AppState) -> bool {
    let at = focus(state);
    if at + 1 >= panes(state).len() {
        return false;
    }
    set_focus(state, at + 1);
    true
}

/// Layouts with a navigation column to the left of the panes.
pub fn has_nav(layout: Layout) -> bool {
    matches!(layout, Layout::Sidebar | Layout::Columns)
}

/// The navigation column has focus. Only ever true in a layout that has one,
/// whatever `nav_focused` was left at.
pub fn in_nav(state: &AppState) -> bool {
    state.nav_focused && has_nav(state.settings.layout)
}

/// `[`: focus the pane to the left, or the navigation column from the leftmost pane
/// of a layout that has one.
pub fn pane_left(state: &mut AppState) {
    if !in_nav(state) && !focus_left(state) && has_nav(state.settings.layout) {
        state.nav_focused = true;
    }
}

/// `]` or Enter in the navigation column: focus goes to the first pane drawn.
pub fn leave_nav(state: &mut AppState) {
    state.nav_focused = false;
    let first = first_pane(state);
    set_focus(state, first);
}

/// One entry of the navigation column: a Library section, Search, the Feed, or (in
/// Sidebar) one of the user's playlists, which sits on Library › Playlists.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NavItem {
    pub label: String,
    pub tab: usize,
    /// `None` keeps the sub-tab the main tab was last on.
    pub sub: Option<usize>,
    /// `tracks_uri` of the playlist a playlist entry stands for.
    pub playlist_uri: Option<String>,
}

/// The navigation column, top to bottom: Likes, Playlists (with the playlists listed
/// beneath it when `with_playlists`), Albums, Following, Search, and the Feed unless
/// it is hidden.
pub fn nav_items(state: &AppState, data: &AppData, with_playlists: bool) -> Vec<NavItem> {
    let section = |label: &str, tab, sub| NavItem { label: label.to_string(), tab, sub, playlist_uri: None };
    let mut out = vec![section(SUBTAB_TITLES[0], 0, Some(0)), section(SUBTAB_TITLES[1], 0, Some(1))];
    if with_playlists {
        out.extend(data.playlists.iter().map(|p| NavItem {
            label: p.title.clone(),
            tab: 0,
            sub: Some(1),
            playlist_uri: Some(p.tracks_uri.clone()),
        }));
    }
    out.extend([section(SUBTAB_TITLES[2], 0, Some(2)), section(SUBTAB_TITLES[3], 0, Some(3)), section(TAB_TITLES[1], 1, None)]);
    if !state.settings.hide_feed_tab {
        out.push(section(TAB_TITLES[2], 2, None));
    }
    out
}

/// Which of `items` is the current one: the entry for the location, or, on Library ›
/// Playlists when the column lists playlists and was last on one, the selected playlist.
pub fn nav_index(state: &AppState, data: &AppData, items: &[NavItem]) -> usize {
    let (tab, sub) = location(state);
    if (tab, sub) == (0, 1)
        && !state.nav_on_header
        && let Some(first) = items.iter().position(|i| i.playlist_uri.is_some())
    {
        // By position, not uri: a playlist the user both owns and likes is listed twice.
        let at = first + cursor(state, Playlists).min(data.playlists.len().saturating_sub(1));
        if items.get(at).is_some_and(|i| i.playlist_uri.is_some()) {
            return at;
        }
    }
    items
        .iter()
        .position(|i| i.playlist_uri.is_none() && i.tab == tab && i.sub.is_none_or(|s| s == sub))
        .unwrap_or(0)
}

/// Go where a navigation entry points. A playlist entry also selects its playlist in
/// the Playlists pane, by identity, and drops any filter on the tracks it had open.
pub fn nav_go(state: &mut AppState, data: &AppData, item: &NavItem) {
    set_location(state, item.tab, item.sub);
    state.nav_on_header = item.playlist_uri.is_none();
    if let Some(uri) = &item.playlist_uri
        && let Some(at) = data.playlists.iter().position(|p| p.tracks_uri == *uri)
    {
        clear_filter(state);
        *cursor_mut(state, Playlists) = at;
    }
}

/// The playlist under the navigation column's cursor while the column has focus: the
/// row container actions (delete, like) act on there.
pub fn nav_playlist<'a>(state: &AppState, data: &'a AppData) -> Option<&'a Playlist> {
    if in_nav(state) && first_pane(state) == 1 && !state.nav_on_header {
        data.playlists.get(cursor(state, Playlists))
    } else {
        None
    }
}

/// Keep the flags the older handlers and renderers read in step with `focus`.
fn sync_flags(state: &mut AppState) {
    let people = |at: usize| match at {
        0 => FollowingTracksFocus::Artists,
        1 => FollowingTracksFocus::Published,
        _ => FollowingTracksFocus::Likes,
    };
    state.following_tracks_focus = people(state.focus[0][3]);
    state.search_people_tracks_focus = people(state.focus[1][3]);
    state.info_pane_selected = state.focus[2][0] > 0;
}

/// The location whose first pane is `id`. That pane's cursor is `selected_row` while
/// its location is on screen and is parked in `primary_rows` while it is not.
fn home(id: PaneId) -> Option<(usize, usize)> {
    (0..3)
        .flat_map(|tab| (0..4).map(move |sub| (tab, sub)))
        .find(|&(tab, sub)| panes_at(tab, sub)[0] == id)
}

/// Where a pane's cursor is stored; `$($r)+` is `&` or `&mut`.
macro_rules! slot {
    ($state:expr, $id:expr, $($r:tt)+) => {
        match $id {
            PlaylistTracks => $($r)+ $state.selected_playlist_track_row,
            AlbumTracks => $($r)+ $state.selected_album_track_row,
            FollowingTracks => $($r)+ $state.selected_following_track_row,
            FollowingLikes => $($r)+ $state.selected_following_like_row,
            SearchAlbumTracks => $($r)+ $state.search_selected_album_track_row,
            SearchPlaylistTracks => $($r)+ $state.search_selected_playlist_track_row,
            SearchPeopleTracks => $($r)+ $state.search_selected_person_track_row,
            SearchPeopleLikes => $($r)+ $state.search_selected_person_like_row,
            FeedTracks => $($r)+ $state.selected_info_row,
            first => match home(first) {
                Some((tab, sub)) if (tab, sub) != location($state) => {
                    $($r)+ $state.primary_rows[tab][sub]
                }
                _ => $($r)+ $state.selected_row,
            },
        }
    };
}

/// The selected row of a pane, as an index into what the pane shows (its filtered
/// rows while a filter is active).
pub fn cursor(state: &AppState, id: PaneId) -> usize {
    *slot!(state, id, &)
}

pub(crate) fn cursor_mut(state: &mut AppState, id: PaneId) -> &mut usize {
    slot!(state, id, &mut)
}

/// The Library pane `/` filters; none on Search and Feed.
pub fn filter_pane(state: &AppState) -> Option<PaneId> {
    (state.selected_tab == 0).then(|| [Likes, PlaylistTracks, Albums, Following][state.selected_subtab.min(3)])
}

fn filtered(state: &AppState, id: PaneId) -> bool {
    is_filter_active(state) && filter_pane(state) == Some(id)
}

/// The selected row of a pane as an index into its unfiltered list. `None` when a
/// filter is active and matches nothing.
pub fn selected_index(state: &AppState, id: PaneId) -> Option<usize> {
    let row = cursor(state, id);
    if filtered(state, id) {
        state.search_matches.get(row).copied()
    } else {
        Some(row)
    }
}

fn tracks(data: &AppData, id: PaneId) -> Option<&Vec<Track>> {
    Some(match id {
        Likes => &data.likes,
        PlaylistTracks => &data.playlist_tracks,
        AlbumTracks => &data.album_tracks,
        FollowingTracks => &data.following_tracks,
        FollowingLikes => &data.following_likes_tracks,
        SearchTracks => &data.search_tracks,
        SearchAlbumTracks => &data.search_album_tracks,
        SearchPlaylistTracks => &data.search_playlist_tracks,
        SearchPeopleTracks => &data.search_people_tracks,
        SearchPeopleLikes => &data.search_people_likes_tracks,
        FeedTracks => &data.feed_tracks,
        _ => return None,
    })
}

/// How many rows a pane shows (its filtered length while a filter is active).
pub fn len(state: &AppState, data: &AppData, views: &FilteredViews, id: PaneId) -> usize {
    if filtered(state, id) {
        return match id {
            Likes => views.likes.len(),
            PlaylistTracks => views.playlist_tracks.len(),
            Albums => views.albums.len(),
            _ => views.following.len(),
        };
    }
    match id {
        Playlists => data.playlists.len(),
        Albums => data.albums.len(),
        Following => data.following.len(),
        SearchAlbums => data.search_albums.len(),
        SearchPlaylists => data.search_playlists.len(),
        SearchPeople => data.search_people.len(),
        Feed => data.feed.len(),
        _ => tracks(data, id).map_or(0, Vec::len),
    }
}

pub fn table(data: &mut AppData, id: PaneId) -> &mut TableState {
    match id {
        Likes => &mut data.likes_state,
        Playlists => &mut data.playlists_state,
        PlaylistTracks => &mut data.playlist_tracks_state,
        Albums => &mut data.albums_state,
        AlbumTracks => &mut data.album_tracks_state,
        Following => &mut data.following_state,
        FollowingTracks => &mut data.following_tracks_state,
        FollowingLikes => &mut data.following_likes_state,
        SearchTracks => &mut data.search_tracks_state,
        SearchAlbums => &mut data.search_albums_state,
        SearchAlbumTracks => &mut data.search_album_tracks_state,
        SearchPlaylists => &mut data.search_playlists_state,
        SearchPlaylistTracks => &mut data.search_playlist_tracks_state,
        SearchPeople => &mut data.search_people_state,
        SearchPeopleTracks => &mut data.search_people_tracks_state,
        SearchPeopleLikes => &mut data.search_people_likes_state,
        Feed => &mut data.feed_state,
        FeedTracks => &mut data.feed_tracks_state,
    }
}

/// Move the focused pane's cursor by `delta` rows, stopping at either end.
pub fn move_by(state: &mut AppState, data: &mut AppData, views: &FilteredViews, delta: isize) {
    let id = focused(state);
    let last = len(state, data, views, id).saturating_sub(1);
    let row = cursor_mut(state, id);
    *row = row.saturating_add_signed(delta).min(last);
    let row = *row;
    table(data, id).select(Some(row));
}

pub fn move_to(state: &mut AppState, data: &mut AppData, views: &FilteredViews, end: Edge) {
    let delta = match end {
        Edge::Top => isize::MIN,
        Edge::Bottom => isize::MAX,
    };
    move_by(state, data, views, delta);
}

/// Rows a page key moves: the focused pane's visible rows once a render has measured
/// them, ten before that.
pub fn page_rows(state: &AppState) -> usize {
    if state.page_rows == 0 { 10 } else { state.page_rows }
}

/// Pull the cursors of the panes on screen back inside their lists.
pub fn clamp(state: &mut AppState, data: &AppData, views: &FilteredViews) {
    for &id in panes(state) {
        let last = len(state, data, views, id).saturating_sub(1);
        let row = cursor_mut(state, id);
        *row = (*row).min(last);
    }
}

/// Point every `TableState` at its pane's cursor. Run once per loop pass, before drawing.
pub fn sync_tables(state: &AppState, data: &mut AppData) {
    for id in PaneId::ALL {
        table(data, id).select(Some(cursor(state, id)));
    }
}

/// The selected track of the focused pane; `None` when a container pane has focus.
pub fn selected_track(state: &AppState, data: &AppData, views: &FilteredViews) -> Option<Track> {
    let id = focused(state);
    let row = cursor(state, id);
    let rows = match id {
        Likes if filtered(state, id) => &views.likes,
        PlaylistTracks if filtered(state, id) => &views.playlist_tracks,
        _ => tracks(data, id)?,
    };
    rows.get(row).cloned()
}

/// Go to a main tab and sub-tab (`None` = the one that tab was last on). Leaving a
/// location closes its filter and any text entry and parks its first pane's cursor;
/// arriving restores the one parked there. Changing the search type refetches.
pub fn set_location(state: &mut AppState, tab: usize, sub: Option<usize>) {
    let from = location(state);
    let tab = tab.min(2);
    let sub = match (tab, sub) {
        (2, _) => 0,
        (_, Some(sub)) => sub.min(3),
        (0, None) => state.selected_subtab.min(3),
        (_, None) => state.selected_searchfilter.min(3),
    };
    if (tab, sub) == from {
        return;
    }
    clear_filter(state);
    state.search_typing = false;
    state.primary_rows[from.0][from.1] = state.selected_row;
    state.selected_tab = tab;
    if tab == 0 {
        state.selected_subtab = sub;
    } else if tab == 1 && state.selected_searchfilter != sub {
        state.selected_searchfilter = sub;
        state.search_needs_fetch = true;
        reset_search(state);
    }
    state.selected_row = state.primary_rows[tab][sub];
    sync_flags(state);
}

/// A new search is about to replace every search list: cursors and focus go back to the start.
pub fn reset_search(state: &mut AppState) {
    if state.selected_tab == 1 {
        state.selected_row = 0;
    }
    state.primary_rows[1] = [0; 4];
    state.search_selected_playlist_track_row = 0;
    state.search_selected_album_track_row = 0;
    state.search_selected_person_track_row = 0;
    state.search_selected_person_like_row = 0;
    state.focus[1] = [0; 4];
    sync_flags(state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{Artist, Playlist};
    use crate::keymap::Action;
    use crate::tui::logic::filtering::{build_filtered_views, rebuild_matches_if_active};
    use crate::tui::logic::input::commands::{run_command, run_nav_command};
    use crate::tui::logic::input::playback::focused_artist;
    use crate::tui::logic::state::{ConfirmAction, Engagement};

    fn track(id: u64, title: &str) -> Track {
        Track {
            title: title.to_string(),
            artists: String::new(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: format!("soundcloud:tracks:{id}"),
        }
    }

    fn tracks_named(titles: &[&str]) -> Vec<Track> {
        titles.iter().enumerate().map(|(i, t)| track(i as u64, t)).collect()
    }

    fn playlist(id: u64) -> Playlist {
        Playlist {
            title: format!("playlist {id}"),
            track_count: "0".to_string(),
            duration: String::new(),
            created_at: Default::default(),
            tracks_uri: format!("https://api.soundcloud.com/playlists/{id}/tracks"),
            is_owned: true,
        }
    }

    fn artist(id: u64, name: &str) -> Artist {
        Artist { name: name.to_string(), urn: format!("soundcloud:users:{id}") }
    }

    fn at(tab: usize, sub: usize) -> AppState {
        let mut state = AppState::default();
        set_location(&mut state, tab, Some(sub));
        state
    }

    /// Type a Library filter the way the insert handler leaves it.
    fn filter(state: &mut AppState, data: &AppData, query: &str) {
        state.search_popup_visible = true;
        state.search_query = query.to_string();
        rebuild_matches_if_active(state, data);
    }

    #[test]
    fn panes_for_every_location() {
        let expect: [(usize, usize, &[PaneId]); 9] = [
            (0, 0, &[Likes]),
            (0, 1, &[Playlists, PlaylistTracks]),
            (0, 2, &[Albums, AlbumTracks]),
            (0, 3, &[Following, FollowingTracks, FollowingLikes]),
            (1, 0, &[SearchTracks]),
            (1, 1, &[SearchAlbums, SearchAlbumTracks]),
            (1, 2, &[SearchPlaylists, SearchPlaylistTracks]),
            (1, 3, &[SearchPeople, SearchPeopleTracks, SearchPeopleLikes]),
            (2, 0, &[Feed, FeedTracks]),
        ];
        for (tab, sub, want) in expect {
            let state = at(tab, sub);
            assert_eq!(location(&state), (tab, sub));
            assert_eq!(panes(&state), want, "location {tab}/{sub}");
            assert_eq!(focused(&state), want[0], "focus starts on the first pane");
            assert_eq!(want[0].is_container(), want.len() > 1);
            assert!(want[1..].iter().all(|p| p.kind() == PaneKind::Tracks));
        }
        // Each main tab remembers its own sub-tab.
        let mut state = at(0, 3);
        set_location(&mut state, 1, Some(2));
        set_location(&mut state, 0, None);
        assert_eq!(location(&state), (0, 3));
        set_location(&mut state, 1, None);
        assert_eq!(location(&state), (1, 2));
    }

    #[test]
    fn focus_is_per_location_and_clamped() {
        let mut state = at(0, 3);
        set_focus(&mut state, 9);
        assert_eq!(focus(&state), 2, "clamped to the last pane");
        set_location(&mut state, 0, Some(1));
        assert_eq!(focus(&state), 0, "another location has its own focus");
        set_focus(&mut state, 1);
        set_location(&mut state, 0, Some(3));
        assert_eq!(focused(&state), FollowingLikes);
        set_location(&mut state, 0, Some(1));
        assert_eq!(focused(&state), PlaylistTracks);
        // A stored index past the end of a one-pane location is still valid to read.
        state.focus[0][0] = 5;
        set_location(&mut state, 0, Some(0));
        assert_eq!(focused(&state), Likes);
    }

    #[test]
    fn bracket_focus_stops_at_edges() {
        let mut state = at(0, 3);
        assert!(!focus_left(&mut state));
        assert_eq!(state.following_tracks_focus, FollowingTracksFocus::Artists);
        assert!(focus_right(&mut state));
        assert_eq!(state.following_tracks_focus, FollowingTracksFocus::Published);
        assert!(focus_right(&mut state));
        assert_eq!(state.following_tracks_focus, FollowingTracksFocus::Likes);
        assert!(!focus_right(&mut state), "no wrap");
        assert_eq!(focused(&state), FollowingLikes);
        assert!(focus_left(&mut state));
        assert_eq!(focused(&state), FollowingTracks);

        let mut state = at(2, 0);
        assert!(!state.info_pane_selected);
        assert!(focus_right(&mut state) && state.info_pane_selected);
        assert!(!focus_right(&mut state));
        assert!(focus_left(&mut state) && !state.info_pane_selected);

        let mut state = at(1, 3);
        assert!(focus_right(&mut state));
        assert_eq!(state.search_people_tracks_focus, FollowingTracksFocus::Published);
        assert_eq!(state.following_tracks_focus, FollowingTracksFocus::Artists, "Library untouched");
    }

    #[test]
    fn move_by_only_moves_focused_pane() {
        let mut state = at(0, 1);
        let mut data = AppData {
            playlists: vec![playlist(1), playlist(2), playlist(3)],
            playlist_tracks: tracks_named(&["a", "b", "c", "d", "e"]),
            ..Default::default()
        };
        let views = build_filtered_views(&state, &data);

        move_by(&mut state, &mut data, &views, 1);
        assert_eq!((cursor(&state, Playlists), cursor(&state, PlaylistTracks)), (1, 0));
        assert_eq!(data.playlists_state.selected(), Some(1));

        focus_right(&mut state);
        move_by(&mut state, &mut data, &views, 2);
        assert_eq!((cursor(&state, Playlists), cursor(&state, PlaylistTracks)), (1, 2));
        move_by(&mut state, &mut data, &views, 99);
        assert_eq!(cursor(&state, PlaylistTracks), 4, "stops at the last row");
        move_to(&mut state, &mut data, &views, Edge::Top);
        assert_eq!(cursor(&state, PlaylistTracks), 0);
        move_to(&mut state, &mut data, &views, Edge::Bottom);
        assert_eq!(cursor(&state, PlaylistTracks), 4);
        assert_eq!(cursor(&state, Playlists), 1);

        // An empty pane: every move is a no-op, never an underflow.
        data.playlist_tracks.clear();
        move_by(&mut state, &mut data, &views, 1);
        move_to(&mut state, &mut data, &views, Edge::Bottom);
        assert_eq!(cursor(&state, PlaylistTracks), 0);
        assert_eq!(selected_track(&state, &data, &views).map(|t| t.title), None);
    }

    #[test]
    fn tab_switch_restores_cursor_and_table_offset() {
        let mut state = AppState::default();
        let mut data = AppData {
            likes: (0..400).map(|i| track(i, "t")).collect(),
            playlists: vec![playlist(1), playlist(2)],
            ..Default::default()
        };
        let views = build_filtered_views(&state, &data);
        move_by(&mut state, &mut data, &views, 300);

        set_location(&mut state, 0, Some(1));
        assert_eq!(cursor(&state, Playlists), 0);
        assert_eq!(cursor(&state, Likes), 300, "still known while off screen");
        move_by(&mut state, &mut data, &views, 1);
        clamp(&mut state, &data, &views);
        sync_tables(&state, &mut data);
        assert_eq!(data.likes_state.selected(), Some(300));

        set_location(&mut state, 0, Some(0));
        sync_tables(&state, &mut data);
        assert_eq!(cursor(&state, Likes), 300);
        assert_eq!(data.likes_state.selected(), Some(300));
        assert_eq!(cursor(&state, Playlists), 1);

        // The same across main tabs.
        set_location(&mut state, 2, None);
        assert_eq!(cursor(&state, Feed), 0);
        set_location(&mut state, 0, None);
        assert_eq!(cursor(&state, Likes), 300);
    }

    #[test]
    fn filter_closes_on_tab_switch() {
        let mut state = AppState::default();
        let mut data = AppData { likes: tracks_named(&["alpha", "beta", "alto"]), ..Default::default() };
        filter(&mut state, &data, "al");
        state.search_typing = true;
        let views = build_filtered_views(&state, &data);
        move_by(&mut state, &mut data, &views, 1);
        assert_eq!(selected_index(&state, Likes), Some(2));

        set_location(&mut state, 1, None);
        assert!(!state.search_popup_visible && !state.search_typing);
        assert!(state.search_query.is_empty() && state.search_matches.is_empty());
        // Back in Likes the cursor is on the row that was highlighted in the filter.
        set_location(&mut state, 0, None);
        assert_eq!(cursor(&state, Likes), 2);
    }

    #[test]
    fn filter_matches_follow_list_changes() {
        let mut state = at(0, 1);
        let mut data = AppData { playlist_tracks: tracks_named(&["alpha", "beta", "alto"]), ..Default::default() };
        filter(&mut state, &data, "al");
        assert_eq!(state.search_matches, [0, 2]);

        // Another playlist's tracks land in the pane.
        data.playlist_tracks = tracks_named(&["beta", "gamma", "alpine"]);
        rebuild_matches_if_active(&mut state, &data);
        assert_eq!(state.search_matches, [2]);
        let views = build_filtered_views(&state, &data);
        assert_eq!(views.playlist_tracks[0].title, "alpine");
        assert_eq!(len(&state, &data, &views, PlaylistTracks), 1);

        // A row arriving above the highlighted one (a new like), then leaving again (an
        // unlike), never moves the highlight off its item.
        let mut state = AppState::default();
        data.likes = tracks_named(&["zz", "alto"]);
        filter(&mut state, &data, "al");
        let highlighted = |state: &AppState, data: &AppData| {
            selected_index(state, Likes).map(|i| data.likes[i].title.clone())
        };
        assert_eq!(highlighted(&state, &data).as_deref(), Some("alto"));
        data.likes.insert(0, track(9, "al"));
        rebuild_matches_if_active(&mut state, &data);
        assert_eq!(state.search_matches, [0, 2]);
        assert_eq!(highlighted(&state, &data).as_deref(), Some("alto"));
        data.likes.remove(0);
        rebuild_matches_if_active(&mut state, &data);
        assert_eq!(highlighted(&state, &data).as_deref(), Some("alto"));
        // A page landing after the user has moved leaves the cursor where they put it.
        data.likes.insert(0, track(9, "al"));
        rebuild_matches_if_active(&mut state, &data);
        let views = build_filtered_views(&state, &data);
        move_by(&mut state, &mut data, &views, -1);
        data.likes.push(track(10, "also"));
        rebuild_matches_if_active(&mut state, &data);
        assert_eq!(highlighted(&state, &data).as_deref(), Some("al"));
    }

    #[test]
    fn artist_station_uses_filtered_row() {
        let mut state = at(0, 3);
        let data = AppData { following: vec![artist(1, "Zed"), artist(2, "Amy"), artist(3, "Bob")], ..Default::default() };
        filter(&mut state, &data, "bob");
        assert_eq!(focused_artist(&state, &data).map(|a| a.name).as_deref(), Some("Bob"));
        // Only the artist row itself starts an artist station.
        focus_right(&mut state);
        assert!(focused_artist(&state, &data).is_none());
    }

    #[test]
    fn like_on_track_pane_targets_track_not_parent() {
        let mut state = at(0, 1);
        let mut data = AppData {
            playlists: vec![playlist(1)],
            playlist_tracks: vec![track(7, "song")],
            ..Default::default()
        };

        focus_right(&mut state);
        run_command(Action::ToggleLike, &mut state, &mut data);
        assert!(matches!(
            state.engagement_queue.pop_front(),
            Some(Engagement::LikeTrack { track_id: 7, .. })
        ));
        assert!(state.engagement_queue.is_empty());
        assert!(data.liked_playlist_uris.is_empty());

        // With the playlist row focused the same key likes the playlist.
        focus_left(&mut state);
        run_command(Action::ToggleLike, &mut state, &mut data);
        assert!(matches!(
            state.engagement_queue.pop_front(),
            Some(Engagement::LikePlaylist { playlist_id: 1, .. })
        ));
    }

    #[test]
    fn container_action_ignored_from_tracks_pane() {
        let mut state = at(0, 1);
        let mut data = AppData::default();
        data.playlists = vec![playlist(1)];
        data.playlist_tracks = vec![track(7, "song")];
        data.playlist_tracks_uri = Some(data.playlists[0].tracks_uri.clone());

        focus_right(&mut state);
        run_command(Action::DeletePlaylist, &mut state, &mut data);
        assert!(state.confirm.is_none(), "delete needs the playlist row focused");
        run_command(Action::RemoveFromPlaylist, &mut state, &mut data);
        assert!(matches!(state.confirm.take(), Some(ConfirmAction::RemoveTrack { .. })));

        focus_left(&mut state);
        for action in [Action::RemoveFromPlaylist, Action::AddToQueue, Action::PlayNext, Action::AddToPlaylist] {
            run_command(action, &mut state, &mut data);
        }
        assert!(state.confirm.is_none() && state.manual_queue.is_empty() && !state.playlist_picker_visible);
        run_command(Action::DeletePlaylist, &mut state, &mut data);
        assert!(matches!(state.confirm, Some(ConfirmAction::DeletePlaylist { .. })));

        // An empty library: every selection action is a no-op.
        let mut state = AppState::default();
        let mut data = AppData::default();
        for tab in 0..3 {
            for sub in 0..4 {
                set_location(&mut state, tab, Some(sub));
                for focus in 0..3 {
                    set_focus(&mut state, focus);
                    for action in [
                        Action::ToggleLike, Action::AddToQueue, Action::PlayNext, Action::AddToPlaylist,
                        Action::RemoveFromPlaylist, Action::DeletePlaylist,
                    ] {
                        run_command(action, &mut state, &mut data);
                    }
                    assert!(focused_artist(&state, &data).is_none());
                }
            }
        }
        assert!(state.confirm.is_none() && state.engagement_queue.is_empty());
    }

    fn in_layout(layout: Layout, tab: usize, sub: usize) -> AppState {
        let mut state = at(tab, sub);
        state.settings.layout = layout;
        state
    }

    #[test]
    fn bracket_left_from_first_pane_focuses_nav_only_in_nav_layouts() {
        for layout in Layout::ALL {
            let mut state = in_layout(layout, 0, 3);
            set_focus(&mut state, 1);
            pane_left(&mut state);
            assert_eq!((focus(&state), in_nav(&state)), (0, false), "{layout:?}: one pane left first");
            pane_left(&mut state);
            assert_eq!(in_nav(&state), has_nav(layout), "{layout:?}");
            assert_eq!(state.nav_focused, has_nav(layout), "{layout:?}: no flag left behind");
            pane_left(&mut state);
            assert_eq!((focus(&state), in_nav(&state)), (0, has_nav(layout)), "{layout:?}: nothing further left");
        }
        assert_eq!(Layout::ALL.map(has_nav), [false, false, true, true, false]);

        // `]` and Enter leave it for the first pane drawn, wherever focus was before.
        let mut data = AppData::default();
        for leave in [Action::PaneRight, Action::PlaySelected] {
            let mut state = in_layout(Layout::Sidebar, 0, 3);
            set_focus(&mut state, 2);
            state.nav_focused = true;
            assert!(run_nav_command(Action::PaneLeft, &mut state, &mut data), "[ is swallowed, not passed on");
            assert!(run_nav_command(leave, &mut state, &mut data));
            assert_eq!((in_nav(&state), focus(&state)), (false, 0), "{leave:?}");
        }
        // Library › Playlists in Sidebar: the list pane is the column, so focus starts
        // on the tracks and `[` from there is already the column.
        let mut state = in_layout(Layout::Sidebar, 0, 1);
        assert_eq!((first_pane(&state), focus(&state), focused(&state)), (1, 1, PlaylistTracks));
        assert!(!focus_left(&mut state));
        pane_left(&mut state);
        assert!(in_nav(&state));
        run_nav_command(Action::PlaySelected, &mut state, &mut data);
        assert_eq!((in_nav(&state), focused(&state)), (false, PlaylistTracks));
        // The same location anywhere else keeps its Playlists pane.
        state.settings.layout = Layout::Columns;
        assert_eq!((first_pane(&state), focus_left(&mut state), focused(&state)), (0, true, Playlists));
        // A flag left set means nothing in a layout without the column.
        state.nav_focused = true;
        state.settings.layout = Layout::Tighten;
        assert!(!in_nav(&state));
    }

    #[test]
    fn nav_move_changes_location() {
        let mut data = AppData {
            playlists: vec![playlist(1), playlist(2)],
            likes: tracks_named(&["a", "b", "c"]),
            ..Default::default()
        };
        let mut state = in_layout(Layout::Sidebar, 1, 2);
        state.search_needs_fetch = false;
        set_location(&mut state, 0, Some(0));
        *cursor_mut(&mut state, Likes) = 2;
        state.nav_focused = true;
        let labels: Vec<String> = nav_items(&state, &data, true).into_iter().map(|i| i.label).collect();
        assert_eq!(labels, ["Likes", "Playlists", "playlist 1", "playlist 2", "Albums", "Following", "Search", "Feed"]);

        let mut step = |state: &mut AppState, action: Action| {
            assert!(run_nav_command(action, state, &mut data), "{action:?}");
            let items = nav_items(state, &data, state.settings.layout == Layout::Sidebar);
            (nav_index(state, &data, &items), location(state))
        };
        // Down the column: each step is a change of location, at once.
        assert_eq!(step(&mut state, Action::Down), (1, (0, 1)));
        assert!(state.nav_on_header, "the Playlists entry itself");
        assert_eq!(step(&mut state, Action::Down), (2, (0, 1)));
        assert_eq!(step(&mut state, Action::Down), (3, (0, 1)));
        assert_eq!(cursor(&state, Playlists), 1);
        assert_eq!(step(&mut state, Action::Down), (4, (0, 2)));
        assert_eq!(step(&mut state, Action::Down), (5, (0, 3)));
        assert_eq!(step(&mut state, Action::Down), (6, (1, 2)), "Search keeps its own type");
        assert!(!state.search_needs_fetch);
        assert_eq!(step(&mut state, Action::Down), (7, (2, 0)));
        assert_eq!(step(&mut state, Action::Down), (7, (2, 0)), "stops at the end");
        // Back up through the playlists and past the Playlists entry.
        for want in [(6, (1, 2)), (5, (0, 3)), (4, (0, 2)), (3, (0, 1)), (2, (0, 1)), (1, (0, 1)), (0, (0, 0)), (0, (0, 0))] {
            assert_eq!(step(&mut state, Action::Up), want);
        }
        assert_eq!(cursor(&state, Likes), 2, "the panes' own cursors are left alone");
        assert_eq!(step(&mut state, Action::Bottom), (7, (2, 0)));
        assert_eq!(step(&mut state, Action::HalfPageUp), (2, (0, 1)));
        assert_eq!(step(&mut state, Action::Top), (0, (0, 0)));
        assert_eq!(step(&mut state, Action::PageDown), (7, (2, 0)));
        assert_eq!(step(&mut state, Action::PageUp), (0, (0, 0)));
        // A hidden Feed is not an entry.
        state.settings.hide_feed_tab = true;
        assert_eq!(step(&mut state, Action::Bottom), (6, (1, 2)));
        // Columns lists the sections only.
        state.settings.layout = Layout::Columns;
        assert_eq!(step(&mut state, Action::Top), (0, (0, 0)));
        assert_eq!(step(&mut state, Action::Down), (1, (0, 1)));
        assert_eq!(step(&mut state, Action::Down), (2, (0, 2)));

        // Sub-tab, tab and playback keys are not the column's: they work as anywhere.
        for action in [Action::SubTabRight, Action::NextTab, Action::TabSearch, Action::PlayPause, Action::ToggleQueue, Action::Help] {
            assert!(!run_nav_command(action, &mut state, &mut data), "{action:?}");
        }
        assert!(state.nav_focused);
        // The filter prompt belongs to a pane: it takes the focus from the column.
        assert!(!run_nav_command(Action::Search, &mut state, &mut data));
        assert!(!state.nav_focused);
        // An empty library still has its sections.
        let mut state = in_layout(Layout::Sidebar, 0, 1);
        state.nav_focused = true;
        let mut data = AppData::default();
        assert!(run_nav_command(Action::Down, &mut state, &mut data));
        assert_eq!(location(&state), (0, 2));
        assert!(run_nav_command(Action::Up, &mut state, &mut data));
        assert_eq!((location(&state), state.nav_on_header), ((0, 1), true));
    }

    #[test]
    fn selecting_playlist_entry_sets_playlists_location_and_hides_list_pane() {
        let mut data = AppData {
            playlists: vec![playlist(1), playlist(2), playlist(3)],
            likes: tracks_named(&["rave", "ambient"]),
            ..Default::default()
        };
        let mut state = in_layout(Layout::Sidebar, 0, 0);
        filter(&mut state, &data, "amb");
        let items = nav_items(&state, &data, true);
        let entry = items[3].clone();
        assert_eq!((entry.label.as_str(), entry.playlist_uri.as_deref()), ("playlist 2", Some(data.playlists[1].tracks_uri.as_str())));

        nav_go(&mut state, &data, &entry);
        assert_eq!((location(&state), cursor(&state, Playlists)), ((0, 1), 1));
        assert!(!state.search_popup_visible && state.search_query.is_empty(), "the Likes filter is closed");
        assert_eq!(nav_index(&state, &data, &items), 3);
        // The column is the playlists pane: the first pane drawn, and focused, is the tracks.
        assert_eq!((first_pane(&state), focus(&state), focused(&state)), (1, 1, PlaylistTracks));
        set_focus(&mut state, 0);
        assert_eq!(focused(&state), PlaylistTracks, "focus cannot land on the hidden pane");

        // By identity: the list shifts, the entry still means the same playlist.
        data.playlists.insert(0, playlist(9));
        nav_go(&mut state, &data, &items[2]);
        assert_eq!(data.playlists[cursor(&state, Playlists)].title, "playlist 1");
        // A filter on the open playlist's tracks does not follow to the next playlist.
        data.playlist_tracks = tracks_named(&["one", "two"]);
        filter(&mut state, &data, "two");
        nav_go(&mut state, &data, &entry);
        assert!(!state.search_popup_visible);
        assert_eq!(data.playlists[cursor(&state, Playlists)].title, "playlist 2");
        // A playlist that has gone leaves the selection where it was.
        let gone = NavItem { playlist_uri: Some("nope".into()), ..entry.clone() };
        nav_go(&mut state, &data, &gone);
        assert_eq!((location(&state), data.playlists[cursor(&state, Playlists)].title.as_str()), ((0, 1), "playlist 2"));
        // The Playlists entry itself keeps the selected playlist and marks the header.
        nav_go(&mut state, &data, &items[1]);
        let items = nav_items(&state, &data, true);
        assert_eq!((nav_index(&state, &data, &items), cursor(&state, Playlists)), (1, 2));
        // Other layouts draw the Playlists pane as ever.
        state.settings.layout = Layout::Tighten;
        assert_eq!((first_pane(&state), focus(&state)), (0, 0));
    }

    #[test]
    fn delete_and_like_act_on_nav_playlist_entry() {
        let mut data = AppData::default();
        let liked = Playlist { is_owned: false, ..playlist(2) };
        data.liked_playlist_uris.insert(liked.tracks_uri.clone());
        data.playlists = vec![playlist(1), liked.clone()];
        data.playlist_tracks = vec![track(7, "song")];
        data.playlist_tracks_uri = Some(data.playlists[0].tracks_uri.clone());
        let mut state = in_layout(Layout::Sidebar, 0, 1);

        // With the tracks focused the playlist keys do nothing: there is no playlist row.
        run_command(Action::DeletePlaylist, &mut state, &mut data);
        assert!(state.confirm.is_none());

        // On the entry of a playlist of the user's own, delete asks, naming it by identity.
        state.nav_focused = true;
        assert_eq!(nav_playlist(&state, &data).map(|p| p.title.as_str()), Some("playlist 1"));
        assert!(run_nav_command(Action::DeletePlaylist, &mut state, &mut data));
        match state.confirm.take() {
            Some(ConfirmAction::DeletePlaylist { tracks_uri }) => assert_eq!(tracks_uri, data.playlists[0].tracks_uri),
            _ => panic!("no confirm for the nav entry's playlist"),
        }
        // Nothing that needs a track fires from the column, even with tracks beside it.
        for action in [Action::AddToQueue, Action::PlayNext, Action::AddToPlaylist, Action::RemoveFromPlaylist, Action::StartStation] {
            assert!(run_nav_command(action, &mut state, &mut data), "{action:?} fell through");
        }
        assert!(state.manual_queue.is_empty() && !state.playlist_picker_visible && state.confirm.is_none());
        assert!(state.toasts.is_empty() && state.artist_station_request.is_none());

        // A liked playlist cannot be deleted, and like toggles it.
        *cursor_mut(&mut state, Playlists) = 1;
        run_nav_command(Action::DeletePlaylist, &mut state, &mut data);
        assert!(state.confirm.is_none());
        assert!(run_nav_command(Action::ToggleLike, &mut state, &mut data));
        assert!(matches!(
            state.engagement_queue.pop_front(),
            Some(Engagement::UnlikePlaylist { tracks_uri, playlist_id: 2 }) if tracks_uri == liked.tracks_uri
        ));
        assert!(!data.liked_playlist_uris.contains(&liked.tracks_uri));

        // On the Playlists entry itself, on another section, or in Columns (no playlist
        // entries) there is no playlist under the cursor.
        state.nav_on_header = true;
        assert!(nav_playlist(&state, &data).is_none());
        state.nav_on_header = false;
        state.settings.layout = Layout::Columns;
        assert!(nav_playlist(&state, &data).is_none());
        state.settings.layout = Layout::Sidebar;
        set_location(&mut state, 0, Some(2));
        for action in [Action::DeletePlaylist, Action::ToggleLike] {
            assert!(run_nav_command(action, &mut state, &mut data));
        }
        assert!(state.confirm.is_none() && state.engagement_queue.is_empty());
    }
}
