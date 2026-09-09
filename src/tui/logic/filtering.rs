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

/// Drop unplayable rows from every loaded list, for the "hide unplayable tracks"
/// setting. Runs when fetched rows land and when the setting is switched on.
///
/// Pages are appended, so pruning a fresh page only shortens the tail: indices
/// earlier in the list, and any playback queue built from them, are left alone.
///
/// ponytail: destructive. Switching the setting back off only refills a list on its
/// next fetch, so lists that are already fully paged in need a restart. The
/// alternatives — shadowing every list, or mapping rendered rows back to data
/// indices in each of the twelve panes — both cost far more than the setting is
/// worth.
pub fn prune_unplayable(state: &mut AppState, data: &mut AppData) {
    let panes: [(&mut Vec<Track>, &mut TableState, &mut usize); 9] = [
        (&mut data.playlist_tracks, &mut data.playlist_tracks_state, &mut state.selected_playlist_track_row),
        (&mut data.album_tracks, &mut data.album_tracks_state, &mut state.selected_album_track_row),
        (&mut data.following_tracks, &mut data.following_tracks_state, &mut state.selected_following_track_row),
        (&mut data.following_likes_tracks, &mut data.following_likes_state, &mut state.selected_following_like_row),
        (&mut data.search_playlist_tracks, &mut data.search_playlist_tracks_state, &mut state.search_selected_playlist_track_row),
        (&mut data.search_album_tracks, &mut data.search_album_tracks_state, &mut state.search_selected_album_track_row),
        (&mut data.search_people_tracks, &mut data.search_people_tracks_state, &mut state.search_selected_person_track_row),
        (&mut data.search_people_likes_tracks, &mut data.search_people_likes_state, &mut state.search_selected_person_like_row),
        (&mut data.feed_tracks, &mut data.feed_tracks_state, &mut state.selected_info_row),
    ];
    for (tracks, table, row) in panes {
        prune_list(tracks, table, row);
    }

    // Likes, search results and the feed share `selected_row`, so they are pruned with
    // a scratch cursor and the real one is clamped against whichever list is on screen.
    let mut scratch = 0;
    prune_list(&mut data.likes, &mut data.likes_state, &mut scratch);
    prune_list(&mut data.search_tracks, &mut data.search_tracks_state, &mut scratch);
    data.feed.retain(|a| a.track.as_ref().is_none_or(Track::is_playable));

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

/// Drop the unplayable tracks from one pane, keeping its cursor in range.
fn prune_list(tracks: &mut Vec<Track>, table: &mut TableState, row: &mut usize) {
    if tracks.iter().all(Track::is_playable) {
        return;
    }
    tracks.retain(Track::is_playable);
    clamp_row(row, table, tracks.len());
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn prune_list_drops_unplayable_and_clamps_the_cursor() {
        let mut tracks = vec![
            track("a", "playable"),
            track("b", "blocked"),
            track("c", ""),
            track("d", "preview"),
        ];
        let mut table = TableState::default().with_selected(3);
        let mut row = 3;
        prune_list(&mut tracks, &mut table, &mut row);
        assert_eq!(
            tracks.iter().map(|t| t.track_urn.as_str()).collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert_eq!(row, 1);
        assert_eq!(table.selected(), Some(1));
    }

    #[test]
    fn prune_list_leaves_a_playable_pane_and_its_cursor_alone() {
        let mut tracks = vec![track("a", "playable"), track("b", "")];
        let mut table = TableState::default().with_selected(1);
        let mut row = 1;
        prune_list(&mut tracks, &mut table, &mut row);
        assert_eq!(tracks.len(), 2);
        assert_eq!(row, 1);
    }
}
