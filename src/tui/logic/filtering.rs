use ratatui::widgets::TableState;

use crate::api::{Album, Artist, Track};

use super::state::{AppData, AppState, clamp_row};

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

pub fn clamp_selection(
    state: &mut AppState,
    data: &mut AppData,
    filter_active: bool,
    likes_len: usize,
    playlist_tracks_len: usize,
    albums_len: usize,
    following_len: usize,
) {
    if !filter_active {
        return;
    }

    match state.selected_subtab {
        0 => {
            if state.selected_row >= likes_len && likes_len > 0 {
                state.selected_row = likes_len - 1;
            }
            data.likes_state.select(Some(state.selected_row));
        }
        1 => {
            if state.selected_playlist_track_row >= playlist_tracks_len && playlist_tracks_len > 0 {
                state.selected_playlist_track_row = playlist_tracks_len - 1;
            }
            data.playlist_tracks_state
                .select(Some(state.selected_playlist_track_row));
        }
        2 => {
            if state.selected_row >= albums_len && albums_len > 0 {
                state.selected_row = albums_len - 1;
            }
            data.albums_state.select(Some(state.selected_row));
        }
        3 => {
            if state.selected_row >= following_len && following_len > 0 {
                state.selected_row = following_len - 1;
            }
            data.following_state.select(Some(state.selected_row));
        }
        _ => {}
    }
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
    // The order here indexes `data.hidden_tracks`; keep the two in step.
    let panes: [(&mut Vec<Track>, &mut TableState, &mut usize); 11] = [
        (&mut data.playlist_tracks, &mut data.playlist_tracks_state, &mut state.selected_playlist_track_row),
        (&mut data.album_tracks, &mut data.album_tracks_state, &mut state.selected_album_track_row),
        (&mut data.following_tracks, &mut data.following_tracks_state, &mut state.selected_following_track_row),
        (&mut data.following_likes_tracks, &mut data.following_likes_state, &mut state.selected_following_like_row),
        (&mut data.search_playlist_tracks, &mut data.search_playlist_tracks_state, &mut state.search_selected_playlist_track_row),
        (&mut data.search_album_tracks, &mut data.search_album_tracks_state, &mut state.search_selected_album_track_row),
        (&mut data.search_people_tracks, &mut data.search_people_tracks_state, &mut state.search_selected_person_track_row),
        (&mut data.search_people_likes_tracks, &mut data.search_people_likes_state, &mut state.search_selected_person_like_row),
        (&mut data.feed_tracks, &mut data.feed_tracks_state, &mut state.selected_info_row),
        (&mut data.likes, &mut data.likes_state, &mut likes_row),
        (&mut data.search_tracks, &mut data.search_tracks_state, &mut search_row),
    ];

    let stashes = &mut data.hidden_tracks;
    if stashes.len() < panes.len() {
        stashes.resize_with(panes.len(), Vec::new);
    }
    for ((tracks, table, row), stash) in panes.into_iter().zip(stashes.iter_mut()) {
        if hide {
            hide_list(tracks, stash, table, row);
        } else {
            restore_rows(tracks, stash);
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
}
