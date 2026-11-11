use ratatui::widgets::TableState;

use crate::api::{Album, Artist, Track};

use super::nav;
use super::state::{AppData, AppState, clamp_row};
use super::utils::build_search_matches;

pub struct FilteredViews {
    pub likes: Vec<Track>,
    pub playlist_tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub following: Vec<Artist>,
}

pub fn is_filter_active(state: &AppState) -> bool {
    state.search_popup_visible
        && state.selected_tab == 0
        && !state.search_query.trim().is_empty()
}

pub fn build_filtered_views(state: &AppState, data: &AppData) -> FilteredViews {
    let filter_active = is_filter_active(state);

    let likes = if filter_active && state.selected_subtab == 0 {
        state
            .search_matches
            .iter()
            .filter_map(|&i| data.likes.get(i).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let playlist_tracks = if filter_active && state.selected_subtab == 1 {
        state
            .search_matches
            .iter()
            .filter_map(|&i| data.playlist_tracks.get(i).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let albums = if filter_active && state.selected_subtab == 2 {
        state
            .search_matches
            .iter()
            .filter_map(|&i| data.albums.get(i).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let following = if filter_active && state.selected_subtab == 3 {
        state
            .search_matches
            .iter()
            .filter_map(|&i| data.following.get(i).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    FilteredViews {
        likes,
        playlist_tracks,
        albums,
        following,
    }
}

fn matches_now(state: &AppState, data: &AppData) -> Vec<usize> {
    build_search_matches(
        state.selected_subtab,
        &state.search_query,
        &data.likes,
        &data.playlist_tracks,
        &data.albums,
        &data.following,
    )
}

/// Identity of each row of the list the filter applies to, in list order.
fn keys<'a>(state: &AppState, data: &'a AppData) -> Box<dyn Iterator<Item = &'a str> + 'a> {
    match state.selected_subtab {
        0 => Box::new(data.likes.iter().map(|t| t.track_urn.as_str())),
        1 => Box::new(data.playlist_tracks.iter().map(|t| t.track_urn.as_str())),
        2 => Box::new(data.albums.iter().map(|a| a.tracks_uri.as_str())),
        _ => Box::new(data.following.iter().map(|a| a.urn.as_str())),
    }
}

/// Recompute the filter's matches from the list as it is now, so they never point at
/// rows that have since moved, and keep the cursor on the item it was on (not the
/// index: rows are inserted and removed above it). Run once per loop pass.
// ponytail: a fuzzy scan of the filtered list on every loop pass while a filter is
// active. Fine for a few thousand rows; if it ever shows in a profile, bump a revision
// counter where the lists change and rescan only when it moved.
pub fn rebuild_matches_if_active(state: &mut AppState, data: &AppData) {
    let Some(pane) = nav::filter_pane(state).filter(|_| is_filter_active(state)) else {
        return;
    };
    let matches = matches_now(state, data);
    let row = nav::cursor(state, pane);
    if matches != state.search_matches {
        state.search_matches = matches;
        // Only while the cursor is still where the anchor was taken; if the user has
        // moved since, their move wins.
        if let Some((_, key)) = state.filter_anchor.as_ref().filter(|(at, _)| *at == row)
            && let Some(raw) = keys(state, data).position(|k| k == key)
            && let Some(now_at) = state.search_matches.iter().position(|&m| m == raw)
        {
            *nav::cursor_mut(state, pane) = now_at;
        }
    }
    let row = nav::cursor(state, pane);
    state.filter_anchor = state
        .search_matches
        .get(row)
        .and_then(|&raw| keys(state, data).nth(raw))
        .map(|key| (row, key.to_string()));
}

/// The filter text was edited: rematch and put the cursor on the best match, or, once
/// the text is empty again, back on `was_on`, the unfiltered index that was highlighted
/// before the edit.
pub fn filter_edited(state: &mut AppState, data: &AppData, was_on: Option<usize>) {
    let Some(pane) = nav::filter_pane(state) else { return };
    state.search_matches = matches_now(state, data);
    *nav::cursor_mut(state, pane) = if is_filter_active(state) { 0 } else { was_on.unwrap_or(0) };
}

/// Drop the Library filter, leaving the cursor on the row that was highlighted.
pub fn clear_filter(state: &mut AppState) {
    if let Some(pane) = nav::filter_pane(state)
        && let Some(raw) = nav::selected_index(state, pane)
    {
        *nav::cursor_mut(state, pane) = raw;
    }
    state.search_popup_visible = false;
    state.search_query.clear();
    state.search_matches.clear();
    state.filter_anchor = None;
}

/// Apply the "hide unplayable tracks" setting to every loaded list: take the unplayable
/// rows out while it is on, and put them back where they were when it is switched off.
/// Runs when fetched rows land and when the setting is flipped.
///
/// Hidden rows are stashed with the index they held in the full list, so restoring is a
/// sequence of in-order inserts and the original order always comes back. Nothing is
/// discarded, so the setting is reversible without refetching anything.
///
/// The playback queue indexes `data.playback_tracks`, a snapshot taken when playback
/// starts, so moving rows around in these lists never disturbs what is playing.
pub fn apply_unplayable_filter(state: &mut AppState, data: &mut AppData) {
    let hide = state.settings.hide_unplayable;

    // Likes and the track search share `selected_row` with other panes, so they move with
    // a scratch cursor and the real one is clamped against whichever list is on screen.
    let (mut likes_row, mut search_row) = (0usize, 0usize);
    // Which list a pane holds: every replacement goes through its fetch task, which
    // takes a new request id each time, and a pane emptied without one loses its key.
    let of = |fetch: &super::state::FetchTask, key: &Option<String>| (fetch.request_id, key.is_some());
    // The order here indexes `data.hidden_tracks`; keep the two in step.
    let panes = [
        (of(&state.playlist_tracks_fetch, &data.playlist_tracks_uri), &mut data.playlist_tracks, &mut data.playlist_tracks_state, &mut state.selected_playlist_track_row),
        (of(&state.album_tracks_fetch, &data.album_tracks_uri), &mut data.album_tracks, &mut data.album_tracks_state, &mut state.selected_album_track_row),
        (of(&state.following_tracks_fetch, &data.following_tracks_user_urn), &mut data.following_tracks, &mut data.following_tracks_state, &mut state.selected_following_track_row),
        (of(&state.following_likes_fetch, &data.following_likes_user_urn), &mut data.following_likes_tracks, &mut data.following_likes_state, &mut state.selected_following_like_row),
        (of(&state.search_playlist_tracks_fetch, &data.search_playlist_tracks_uri), &mut data.search_playlist_tracks, &mut data.search_playlist_tracks_state, &mut state.search_selected_playlist_track_row),
        (of(&state.search_album_tracks_fetch, &data.search_album_tracks_uri), &mut data.search_album_tracks, &mut data.search_album_tracks_state, &mut state.search_selected_album_track_row),
        (of(&state.search_people_tracks_fetch, &data.search_people_tracks_user_urn), &mut data.search_people_tracks, &mut data.search_people_tracks_state, &mut state.search_selected_person_track_row),
        (of(&state.search_people_likes_fetch, &data.search_people_likes_user_urn), &mut data.search_people_likes_tracks, &mut data.search_people_likes_state, &mut state.search_selected_person_like_row),
        (of(&state.feed_tracks_fetch, &data.feed_tracks_key), &mut data.feed_tracks, &mut data.feed_tracks_state, &mut state.selected_info_row),
        // Likes only ever grow; search results are replaced by each new search.
        ((0, true), &mut data.likes, &mut data.likes_state, &mut likes_row),
        ((state.search_results_fetch.request_id, true), &mut data.search_tracks, &mut data.search_tracks_state, &mut search_row),
    ];

    let stashes = &mut data.hidden_tracks;
    if stashes.len() < panes.len() {
        stashes.resize_with(panes.len(), Default::default);
    }
    for ((list, tracks, table, row), stash) in panes.into_iter().zip(stashes.iter_mut()) {
        // Rows hidden from a list this pane no longer shows belong nowhere now.
        if stash.list != list {
            stash.rows.clear();
            stash.list = list;
        }
        if hide {
            hide_list(tracks, &mut stash.rows, table, row);
        } else {
            restore_rows(tracks, &mut stash.rows);
        }
    }

    if hide {
        hide_rows(&mut data.feed, &mut data.hidden_feed, |a| {
            a.track.as_ref().is_none_or(Track::is_playable)
        });
    } else {
        restore_rows(&mut data.feed, &mut data.hidden_feed);
    }

    let shown = match (state.selected_tab, state.selected_subtab, state.selected_searchfilter) {
        (0, 0, _) => Some((data.likes.len(), &mut data.likes_state)),
        (1, _, 0) => Some((data.search_tracks.len(), &mut data.search_tracks_state)),
        (2, _, _) => Some((data.feed.len(), &mut data.feed_state)),
        _ => None,
    };
    if let Some((len, table)) = shown {
        clamp_row(&mut state.selected_row, table, len);
    }
    apply_container_filter(state, data);
}

/// With "hide unplayable tracks" on, a playlist or album known to hold nothing
/// playable is not listed at all (in the Library and in search results). Turning
/// the setting off puts the Library ones back where they were.
///
/// A container is only "known" once the loop has asked the API about it, so rows
/// drop out a moment after a list loads. Empty playlists stay: there is nothing
/// unplayable in them, and the user's own are where new tracks go.
pub fn apply_container_filter(state: &mut AppState, data: &mut AppData) {
    use crate::tui::logic::nav::{self, PaneId};

    // First-seen order, so restored rows can be put back in place.
    let known = &mut data.containers.seq;
    for uri in data.playlists.iter().map(|p| &p.tracks_uri).chain(data.albums.iter().map(|a| &a.tracks_uri)) {
        let next = known.len();
        known.entry(uri.clone()).or_insert(next);
    }

    if !state.settings.hide_unplayable {
        let c = &mut data.containers;
        if !c.hidden_playlists.is_empty() {
            data.playlists.append(&mut c.hidden_playlists);
            data.playlists.sort_by_key(|p| c.seq.get(&p.tracks_uri).copied().unwrap_or(usize::MAX));
        }
        if !c.hidden_albums.is_empty() {
            data.albums.append(&mut c.hidden_albums);
            data.albums.sort_by_key(|a| c.seq.get(&a.tracks_uri).copied().unwrap_or(usize::MAX));
        }
        return;
    }

    let none = &data.containers.none_playable;
    if none.is_empty() {
        return;
    }
    // Take the dead rows out of `rows`, keeping the cursor on the item it was on
    // (or the row that slid into its place when that item is the one removed).
    fn prune<T: Clone>(
        rows: &mut Vec<T>,
        uri: impl Fn(&T) -> &str,
        dead: &std::collections::HashSet<String>,
        cursor: usize,
        mut stash: Option<&mut Vec<T>>,
    ) -> Option<usize> {
        if !rows.iter().any(|r| dead.contains(uri(r))) {
            return None;
        }
        let removed_before = rows.iter().take(cursor).filter(|r| dead.contains(uri(r))).count();
        rows.retain(|r| {
            let keep = !dead.contains(uri(r));
            if !keep && let Some(stash) = stash.as_deref_mut() {
                stash.push(r.clone());
            }
            keep
        });
        Some((cursor - removed_before).min(rows.len().saturating_sub(1)))
    }

    let at = |state: &AppState, id| nav::selected_index(state, id).unwrap_or(0);
    let moved = [
        (PaneId::Playlists, prune(&mut data.playlists, |p| &p.tracks_uri, none, at(state, PaneId::Playlists), Some(&mut data.containers.hidden_playlists))),
        (PaneId::Albums, prune(&mut data.albums, |a| &a.tracks_uri, none, at(state, PaneId::Albums), Some(&mut data.containers.hidden_albums))),
        (PaneId::SearchPlaylists, prune(&mut data.search_playlists, |p| &p.tracks_uri, none, at(state, PaneId::SearchPlaylists), None)),
        (PaneId::SearchAlbums, prune(&mut data.search_albums, |a| &a.tracks_uri, none, at(state, PaneId::SearchAlbums), None)),
    ];
    for (id, cursor) in moved {
        if let Some(cursor) = cursor {
            *nav::cursor_mut(state, id) = cursor;
        }
    }
}

/// The containers worth asking the API about: listed, not empty, not asked yet.
pub fn containers_to_probe(data: &AppData) -> impl Iterator<Item = &str> {
    let playlists = data.playlists.iter().chain(&data.search_playlists).map(|p| (&p.tracks_uri, &p.track_count));
    let albums = data.albums.iter().chain(&data.search_albums).map(|a| (&a.tracks_uri, &a.track_count));
    playlists
        .chain(albums)
        .filter(|(uri, count)| !uri.is_empty() && count.as_str() != "0" && !data.containers.probed.contains(*uri))
        .map(|(uri, _)| uri.as_str())
}

/// Take the unplayable tracks out of one pane, keeping its cursor in range.
fn hide_list(
    tracks: &mut Vec<Track>,
    stash: &mut Vec<(usize, Track)>,
    table: &mut TableState,
    row: &mut usize,
) {
    if tracks.iter().all(Track::is_playable) {
        return;
    }
    hide_rows(tracks, stash, Track::is_playable);
    clamp_row(row, table, tracks.len());
}

/// Move the rows failing `keep` into `stash`, each with the index it holds in the full
/// list. Pages are only ever appended, so everything already stashed sits before every
/// row still on screen, which keeps the stash sorted by index.
fn hide_rows<T: Clone>(rows: &mut Vec<T>, stash: &mut Vec<(usize, T)>, keep: impl Fn(&T) -> bool) {
    let base = stash.len();
    let mut index = 0;
    rows.retain(|row| {
        let keeping = keep(row);
        if !keeping {
            stash.push((base + index, row.clone()));
        }
        index += 1;
        keeping
    });
}

/// Put stashed rows back at the indices they came from. Taken in ascending index order
/// every insert lands correctly, because each earlier row is already back in place.
fn restore_rows<T>(rows: &mut Vec<T>, stash: &mut Vec<(usize, T)>) {
    for (index, row) in stash.drain(..) {
        let at = index.min(rows.len());
        rows.insert(at, row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urns(tracks: &[Track]) -> Vec<&str> {
        tracks.iter().map(|t| t.track_urn.as_str()).collect()
    }

    fn track(urn: &str, access: &str) -> Track {
        Track {
            title: urn.to_string(),
            artists: String::new(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: access.to_string(),
            track_urn: urn.to_string(),
            permalink_url: String::new(),
        }
    }

    #[test]
    fn hiding_drops_unplayable_rows_and_clamps_the_cursor() {
        let mut tracks = vec![
            track("a", "playable"),
            track("b", "blocked"),
            track("c", ""),
            track("d", "preview"),
        ];
        let mut stash = Vec::new();
        let mut table = TableState::default().with_selected(3);
        let mut row = 3;
        hide_list(&mut tracks, &mut stash, &mut table, &mut row);
        assert_eq!(urns(&tracks), ["a", "c"]);
        assert_eq!(row, 1);
        assert_eq!(table.selected(), Some(1));
    }

    #[test]
    fn hiding_leaves_a_playable_pane_and_its_cursor_alone() {
        let mut tracks = vec![track("a", "playable"), track("b", "")];
        let mut stash = Vec::new();
        let mut table = TableState::default().with_selected(1);
        let mut row = 1;
        hide_list(&mut tracks, &mut stash, &mut table, &mut row);
        assert_eq!(tracks.len(), 2);
        assert_eq!(row, 1);
        assert!(stash.is_empty());
    }

    #[test]
    fn restoring_puts_every_hidden_row_back_where_it_was() {
        let original = vec![
            track("a", "blocked"),
            track("b", "playable"),
            track("c", "preview"),
            track("d", ""),
            track("e", "blocked"),
        ];
        let mut tracks = original.clone();
        let mut stash = Vec::new();
        let (mut table, mut row) = (TableState::default().with_selected(0), 0);

        hide_list(&mut tracks, &mut stash, &mut table, &mut row);
        assert_eq!(urns(&tracks), ["b", "d"]);

        restore_rows(&mut tracks, &mut stash);
        assert_eq!(urns(&tracks), urns(&original));
        assert!(stash.is_empty(), "the stash is emptied by restoring");
    }

    #[test]
    fn a_page_appended_while_hidden_still_restores_in_order() {
        // Hide, let another page land on the shortened list, hide again, then restore:
        // the result must be the two pages back to back in their original order.
        let first = vec![track("a", "blocked"), track("b", "playable")];
        let second = [track("c", "playable"), track("d", "preview")];
        let mut tracks = first.clone();
        let mut stash = Vec::new();
        let (mut table, mut row) = (TableState::default().with_selected(0), 0);

        hide_list(&mut tracks, &mut stash, &mut table, &mut row);
        tracks.extend(second.iter().cloned());
        hide_list(&mut tracks, &mut stash, &mut table, &mut row);
        assert_eq!(urns(&tracks), ["b", "c"]);

        restore_rows(&mut tracks, &mut stash);
        assert_eq!(urns(&tracks), ["a", "b", "c", "d"]);
    }

    #[test]
    fn hiding_and_restoring_repeatedly_is_stable() {
        let original = vec![
            track("a", "preview"),
            track("b", "playable"),
            track("c", "blocked"),
        ];
        let mut tracks = original.clone();
        let mut stash = Vec::new();
        let (mut table, mut row) = (TableState::default().with_selected(0), 0);
        for _ in 0..3 {
            hide_list(&mut tracks, &mut stash, &mut table, &mut row);
            assert_eq!(urns(&tracks), ["b"]);
            restore_rows(&mut tracks, &mut stash);
            assert_eq!(urns(&tracks), urns(&original));
        }
    }

    /// T12: rows hidden from one playlist are not restored into the next one.
    #[test]
    fn a_replaced_list_does_not_get_the_old_lists_hidden_rows() {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut state = AppState::default();
        let mut data = AppData::default();
        state.settings.hide_unplayable = true;
        // One pass of the loop over Library > Playlists with playlist `uri` selected,
        // then its tracks landing (if any).
        let show = |state: &mut AppState, data: &mut AppData, uri: Option<&str>, rows: Option<Vec<Track>>| {
            state.playlist_tracks_fetch.refetch(
                nav::PaneId::PlaylistTracks,
                &mut state.child_cursors,
                &mut data.playlist_tracks,
                &mut data.playlist_tracks_state,
                &mut data.playlist_tracks_uri,
                &mut state.selected_playlist_track_row,
                uri.map(str::to_string),
                |_, _| rt.spawn(async {}),
            );
            if let Some(rows) = rows {
                let id = state.playlist_tracks_fetch.request_id;
                state.playlist_tracks_fetch.accept(id, rows, &mut data.playlist_tracks, &mut data.playlist_tracks_state);
                apply_unplayable_filter(state, data);
            }
        };
        let a = || vec![track("a1", "blocked"), track("a2", "playable"), track("a3", "blocked")];
        let b = || vec![track("b1", "playable"), track("b2", "blocked")];

        show(&mut state, &mut data, Some("A"), Some(a()));
        assert_eq!(urns(&data.playlist_tracks), ["a2"]);
        show(&mut state, &mut data, Some("B"), Some(b()));
        assert_eq!(urns(&data.playlist_tracks), ["b1"]);

        state.settings.hide_unplayable = false;
        apply_unplayable_filter(&mut state, &mut data);
        assert_eq!(urns(&data.playlist_tracks), ["b1", "b2"], "only B's rows come back");

        // Still the same list: its own hidden rows return in place.
        state.settings.hide_unplayable = true;
        apply_unplayable_filter(&mut state, &mut data);
        show(&mut state, &mut data, Some("B"), None);
        state.settings.hide_unplayable = false;
        apply_unplayable_filter(&mut state, &mut data);
        assert_eq!(urns(&data.playlist_tracks), ["b1", "b2"]);

        // Back to A before B's stash was ever looked at again, and A reloads: one copy of A.
        state.settings.hide_unplayable = true;
        apply_unplayable_filter(&mut state, &mut data);
        show(&mut state, &mut data, Some("A"), None);
        show(&mut state, &mut data, Some("B"), None);
        show(&mut state, &mut data, Some("A"), Some(a()));
        state.settings.hide_unplayable = false;
        apply_unplayable_filter(&mut state, &mut data);
        assert_eq!(urns(&data.playlist_tracks), ["a1", "a2", "a3"]);

        // Nothing selected: the emptied pane stays empty when the setting goes off.
        state.settings.hide_unplayable = true;
        apply_unplayable_filter(&mut state, &mut data);
        show(&mut state, &mut data, None, None);
        state.settings.hide_unplayable = false;
        apply_unplayable_filter(&mut state, &mut data);
        assert!(data.playlist_tracks.is_empty());
    }

    fn playlist(name: &str, count: &str) -> crate::api::Playlist {
        crate::api::Playlist {
            title: name.to_string(),
            track_count: count.to_string(),
            duration: String::new(),
            created_at: chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00+00:00").unwrap(),
            tracks_uri: format!("uri:{name}"),
            is_owned: true,
            permalink_url: String::new(),
        }
    }

    fn album(name: &str) -> crate::api::Album {
        crate::api::Album {
            title: name.to_string(),
            artists: String::new(),
            release_year: String::new(),
            duration: String::new(),
            track_count: "10".to_string(),
            tracks_uri: format!("uri:{name}"),
            permalink_url: String::new(),
        }
    }

    fn titles<T>(rows: &[T], title: impl Fn(&T) -> &str) -> Vec<&str> {
        rows.iter().map(title).collect()
    }

    #[test]
    fn containers_with_nothing_playable_are_hidden_and_come_back() {
        use crate::tui::logic::nav::{self, PaneId};
        let mut state = AppState::default();
        state.settings.hide_unplayable = true;
        let mut data = AppData {
            playlists: vec![playlist("a", "3"), playlist("dead1", "11"), playlist("b", "2"), playlist("dead2", "4"), playlist("c", "9")],
            albums: vec![album("x"), album("gone"), album("y")],
            search_albums: vec![album("gone"), album("z")],
            ..Default::default()
        };

        // Nothing is known yet: nothing is hidden.
        apply_container_filter(&mut state, &mut data);
        assert_eq!(data.playlists.len(), 5);

        // The cursor is on "c"; two dead rows above it go.
        nav::set_location(&mut state, 0, Some(1));
        *nav::cursor_mut(&mut state, PaneId::Playlists) = 4;
        for dead in ["uri:dead1", "uri:dead2", "uri:gone"] {
            data.containers.none_playable.insert(dead.to_string());
        }
        apply_container_filter(&mut state, &mut data);
        assert_eq!(titles(&data.playlists, |p| &p.title), ["a", "b", "c"]);
        assert_eq!(titles(&data.albums, |a| &a.title), ["x", "y"]);
        assert_eq!(titles(&data.search_albums, |a| &a.title), ["z"], "search results too");
        assert_eq!(nav::cursor(&state, PaneId::Playlists), 2, "the cursor stays on the same playlist");

        // Applying again changes nothing.
        apply_container_filter(&mut state, &mut data);
        assert_eq!(data.playlists.len(), 3);

        // The cursor's own row dies: it lands on the row that slides up, and never past the end.
        data.containers.none_playable.insert("uri:c".to_string());
        apply_container_filter(&mut state, &mut data);
        assert_eq!(titles(&data.playlists, |p| &p.title), ["a", "b"]);
        assert_eq!(nav::cursor(&state, PaneId::Playlists), 1);

        // Setting off: the Library rows return in their original order.
        state.settings.hide_unplayable = false;
        apply_container_filter(&mut state, &mut data);
        assert_eq!(titles(&data.playlists, |p| &p.title), ["a", "dead1", "b", "dead2", "c"]);
        assert_eq!(titles(&data.albums, |a| &a.title), ["x", "gone", "y"]);

        // And back on: gone again without asking the API a second time.
        state.settings.hide_unplayable = true;
        apply_container_filter(&mut state, &mut data);
        assert_eq!(titles(&data.playlists, |p| &p.title), ["a", "b"]);
    }

    #[test]
    fn only_listed_non_empty_unasked_containers_are_probed() {
        let mut data = AppData {
            playlists: vec![playlist("full", "3"), playlist("empty", "0"), playlist("asked", "5")],
            albums: vec![album("x")],
            search_playlists: vec![playlist("found", "7")],
            ..Default::default()
        };
        data.containers.probed.insert("uri:asked".to_string());
        let mut todo: Vec<&str> = containers_to_probe(&data).collect();
        todo.sort_unstable();
        // An empty playlist has nothing unplayable in it, and is where new tracks go.
        assert_eq!(todo, ["uri:found", "uri:full", "uri:x"]);
    }
}
