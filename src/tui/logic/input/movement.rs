use ratatui::crossterm::event::{KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;

use super::InputOutcome;
use crate::tui::logic::state::{AppData, AppState, table_rows_count, FollowingTracksFocus};

/// Single-row move (`delta` = ±1). Moves only if the target row exists (down) or `row > 0` (up);
/// never clamps a stale row and re-selects only when it actually moved. Returns whether it moved.
fn step(row: &mut usize, len: usize, delta: isize, state: &mut TableState) -> bool {
    let new = row.saturating_add_signed(delta);
    if new == *row || (delta > 0 && new >= len) {
        return false;
    }
    *row = new;
    state.select(Some(new));
    true
}

/// Ten-row jump (`delta` = ±10). Down: no-op on an empty list, otherwise clamps to the last row.
/// Up: always saturates towards 0, with no length check. Either way it re-selects even if the row
/// did not change. Returns whether it acted.
fn page(row: &mut usize, len: usize, delta: isize, state: &mut TableState) -> bool {
    if delta > 0 && len == 0 {
        return false;
    }
    let new = row.saturating_add_signed(delta);
    *row = if delta < 0 { new } else { new.min(len - 1) };
    state.select(Some(*row));
    true
}

fn main_state(subtab: usize, data: &mut AppData) -> &mut TableState {
    match subtab {
        1 => &mut data.playlists_state,
        2 => &mut data.albums_state,
        3 => &mut data.following_state,
        _ => &mut data.likes_state,
    }
}

pub(crate) fn handle_down_key(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
) -> InputOutcome {
    if state.selected_tab == 1 {
        handle_search_down(key, state, data);
    } else if state.selected_tab == 0 && state.selected_subtab == 1 {
        handle_playlist_down(key, state, data);
    } else if state.selected_tab == 0 && state.selected_subtab == 2 {
        handle_album_down(key, state, data);
    } else if state.selected_tab == 0 && state.selected_subtab == 3 {
        handle_following_down(key, state, data);
    } else if state.selected_tab == 2 {
        handle_feed_move(key, state, data, 1);
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        handle_alt_down(state, data);
    } else {
        handle_normal_down(state, data);
    }
    InputOutcome::Continue
}

/// Moves within whichever feed pane has focus; Shift targets (and focuses) the info pane;
/// Alt jumps ten rows.
fn handle_feed_move(key: KeyEvent, state: &mut AppState, data: &mut AppData, dir: isize) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        state.info_pane_selected = true;
    }
    let (row, len, table) = if state.info_pane_selected {
        (&mut state.selected_info_row, data.feed_tracks.len(), &mut data.feed_tracks_state)
    } else {
        (&mut state.selected_row, data.feed.len(), &mut data.feed_state)
    };
    if key.modifiers.contains(KeyModifiers::ALT) {
        page(row, len, dir * 10, table);
    } else {
        step(row, len, dir, table);
    }
}

pub(crate) fn handle_up_key(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
) -> InputOutcome {
    if state.selected_tab == 1 {
        handle_search_up(key, state, data);
    } else if state.selected_tab == 0 && state.selected_subtab == 1 {
        handle_playlist_up(key, state, data);
    } else if state.selected_tab == 0 && state.selected_subtab == 2 {
        handle_album_up(key, state, data);
    } else if state.selected_tab == 0 && state.selected_subtab == 3 {
        handle_following_up(key, state, data);
    } else if state.selected_tab == 2 {
        handle_feed_move(key, state, data, -1);
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        handle_alt_up(state, data);
    } else {
        handle_normal_up(state, data);
    }
    InputOutcome::Continue
}

fn handle_playlist_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    let filter_active = state.search_popup_visible && !state.search_query.trim().is_empty();
    let playlist_tracks_len = if filter_active {
        state.search_matches.len()
    } else {
        data.playlist_tracks.len()
    };
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if step(&mut state.selected_row, data.playlists.len(), 1, &mut data.playlists_state) {
            state.selected_playlist_row = state.selected_row;
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        page(&mut state.selected_playlist_track_row, playlist_tracks_len, 10, &mut data.playlist_tracks_state);
    } else {
        step(&mut state.selected_playlist_track_row, playlist_tracks_len, 1, &mut data.playlist_tracks_state);
    }
}

fn handle_album_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if step(&mut state.selected_row, data.albums.len(), 1, &mut data.albums_state) {
            state.selected_album_row = state.selected_row;
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        page(&mut state.selected_album_track_row, data.album_tracks.len(), 10, &mut data.album_tracks_state);
    } else {
        step(&mut state.selected_album_track_row, data.album_tracks.len(), 1, &mut data.album_tracks_state);
    }
}

fn handle_following_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if step(&mut state.selected_row, data.following.len(), 1, &mut data.following_state) {
            state.following_tracks_focus = FollowingTracksFocus::Artists;
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        if page(&mut state.selected_following_track_row, data.following_tracks.len(), 10, &mut data.following_tracks_state) {
            state.following_tracks_focus = FollowingTracksFocus::Published;
        }
    } else if step(&mut state.selected_following_track_row, data.following_tracks.len(), 1, &mut data.following_tracks_state) {
        state.following_tracks_focus = FollowingTracksFocus::Published;
    }
}

fn handle_alt_down(state: &mut AppState, data: &mut AppData) {
    let max_rows = table_rows_count(state.selected_subtab, data);
    page(&mut state.selected_row, max_rows, 10, main_state(state.selected_subtab, data));
}

fn handle_normal_down(state: &mut AppState, data: &mut AppData) {
    let max_rows = table_rows_count(state.selected_subtab, data);
    if step(&mut state.selected_row, max_rows, 1, main_state(state.selected_subtab, data)) {
        if state.selected_subtab == 1 && state.selected_tab == 0 {
            state.selected_playlist_row = state.selected_row;
        }
        if state.selected_subtab == 2 && state.selected_tab == 0 {
            state.selected_album_row = state.selected_row;
        }
    }
}

fn handle_playlist_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    let filter_active = state.search_popup_visible && !state.search_query.trim().is_empty();
    let playlist_tracks_len = if filter_active {
        state.search_matches.len()
    } else {
        data.playlist_tracks.len()
    };
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if step(&mut state.selected_row, data.playlists.len(), -1, &mut data.playlists_state) {
            state.selected_playlist_row = state.selected_row;
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        if playlist_tracks_len > 0 {
            page(&mut state.selected_playlist_track_row, playlist_tracks_len, -10, &mut data.playlist_tracks_state);
        }
    } else if playlist_tracks_len > 0 {
        step(&mut state.selected_playlist_track_row, playlist_tracks_len, -1, &mut data.playlist_tracks_state);
    }
}

fn handle_album_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if step(&mut state.selected_row, data.albums.len(), -1, &mut data.albums_state) {
            state.selected_album_row = state.selected_row;
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        page(&mut state.selected_album_track_row, data.album_tracks.len(), -10, &mut data.album_tracks_state);
    } else {
        step(&mut state.selected_album_track_row, data.album_tracks.len(), -1, &mut data.album_tracks_state);
    }
}

fn handle_following_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if step(&mut state.selected_row, data.following.len(), -1, &mut data.following_state) {
            state.following_tracks_focus = FollowingTracksFocus::Artists;
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        state.following_tracks_focus = FollowingTracksFocus::Published;
        page(&mut state.selected_following_track_row, data.following_tracks.len(), -10, &mut data.following_tracks_state);
    } else if step(&mut state.selected_following_track_row, data.following_tracks.len(), -1, &mut data.following_tracks_state) {
        state.following_tracks_focus = FollowingTracksFocus::Published;
    }
}

fn handle_alt_up(state: &mut AppState, data: &mut AppData) {
    let max_rows = table_rows_count(state.selected_subtab, data);
    page(&mut state.selected_row, max_rows, -10, main_state(state.selected_subtab, data));
}

fn handle_normal_up(state: &mut AppState, data: &mut AppData) {
    let max_rows = table_rows_count(state.selected_subtab, data);
    if step(&mut state.selected_row, max_rows, -1, main_state(state.selected_subtab, data)) {
        if state.selected_subtab == 1 && state.selected_tab == 0 {
            state.selected_playlist_row = state.selected_row;
        }
        if state.selected_subtab == 2 && state.selected_tab == 0 {
            state.selected_album_row = state.selected_row;
        }
    }
}

fn handle_search_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    match state.selected_searchfilter {
        0 => {
            let max_rows = data.search_tracks.len();
            if key.modifiers.contains(KeyModifiers::ALT) {
                page(&mut state.selected_row, max_rows, 10, &mut data.search_tracks_state);
            } else {
                step(&mut state.selected_row, max_rows, 1, &mut data.search_tracks_state);
            }
            data.search_tracks_state.select(Some(state.selected_row));
        }
        1 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                step(&mut state.selected_row, data.search_albums.len(), 1, &mut data.search_albums_state);
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                page(&mut state.search_selected_album_track_row, data.search_album_tracks.len(), 10, &mut data.search_album_tracks_state);
            } else {
                step(&mut state.search_selected_album_track_row, data.search_album_tracks.len(), 1, &mut data.search_album_tracks_state);
            }
        }
        2 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                step(&mut state.selected_row, data.search_playlists.len(), 1, &mut data.search_playlists_state);
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                page(&mut state.search_selected_playlist_track_row, data.search_playlist_tracks.len(), 10, &mut data.search_playlist_tracks_state);
            } else {
                step(&mut state.search_selected_playlist_track_row, data.search_playlist_tracks.len(), 1, &mut data.search_playlist_tracks_state);
            }
        }
        3 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if step(&mut state.selected_row, data.search_people.len(), 1, &mut data.search_people_state) {
                    state.search_people_tracks_focus = FollowingTracksFocus::Artists;
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                if page(&mut state.search_selected_person_track_row, data.search_people_tracks.len(), 10, &mut data.search_people_tracks_state) {
                    state.search_people_tracks_focus = FollowingTracksFocus::Published;
                }
            } else if step(&mut state.search_selected_person_track_row, data.search_people_tracks.len(), 1, &mut data.search_people_tracks_state) {
                state.search_people_tracks_focus = FollowingTracksFocus::Published;
            }
        }
        _ => {}
    }
}

fn handle_search_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    match state.selected_searchfilter {
        0 => {
            if key.modifiers.contains(KeyModifiers::ALT) {
                page(&mut state.selected_row, data.search_tracks.len(), -10, &mut data.search_tracks_state);
            } else {
                step(&mut state.selected_row, data.search_tracks.len(), -1, &mut data.search_tracks_state);
            }
            data.search_tracks_state.select(Some(state.selected_row));
        }
        1 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                step(&mut state.selected_row, data.search_albums.len(), -1, &mut data.search_albums_state);
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                page(&mut state.search_selected_album_track_row, data.search_album_tracks.len(), -10, &mut data.search_album_tracks_state);
            } else {
                step(&mut state.search_selected_album_track_row, data.search_album_tracks.len(), -1, &mut data.search_album_tracks_state);
            }
        }
        2 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                step(&mut state.selected_row, data.search_playlists.len(), -1, &mut data.search_playlists_state);
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                page(&mut state.search_selected_playlist_track_row, data.search_playlist_tracks.len(), -10, &mut data.search_playlist_tracks_state);
            } else {
                step(&mut state.search_selected_playlist_track_row, data.search_playlist_tracks.len(), -1, &mut data.search_playlist_tracks_state);
            }
        }
        3 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if step(&mut state.selected_row, data.search_people.len(), -1, &mut data.search_people_state) {
                    state.search_people_tracks_focus = FollowingTracksFocus::Artists;
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                state.search_people_tracks_focus = FollowingTracksFocus::Published;
                page(&mut state.search_selected_person_track_row, data.search_people_tracks.len(), -10, &mut data.search_people_tracks_state);
            } else if step(&mut state.search_selected_person_track_row, data.search_people_tracks.len(), -1, &mut data.search_people_tracks_state) {
                state.search_people_tracks_focus = FollowingTracksFocus::Published;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_and_page_keep_legacy_semantics() {
        let mut ts = TableState::default();
        // ±1: bounds-checked, never clamps a stale row, selects only when it moved.
        let mut row = 0;
        assert!(!step(&mut row, 3, -1, &mut ts) && ts.selected().is_none());
        assert!(step(&mut row, 3, 1, &mut ts) && row == 1 && ts.selected() == Some(1));
        row = 2;
        assert!(!step(&mut row, 3, 1, &mut ts) && row == 2 && ts.selected() == Some(1));
        row = 5;
        assert!(!step(&mut row, 3, 1, &mut ts) && row == 5);
        assert!(step(&mut row, 3, -1, &mut ts) && row == 4);
        assert!(!step(&mut row, 0, 1, &mut ts) && row == 4);
        // +10: no-op on empty, clamps to the last row, re-selects even when unchanged.
        let mut ts = TableState::default();
        let mut row = 3;
        assert!(!page(&mut row, 0, 10, &mut ts) && row == 3 && ts.selected().is_none());
        assert!(page(&mut row, 8, 10, &mut ts) && row == 7 && ts.selected() == Some(7));
        assert!(page(&mut row, 8, 10, &mut ts) && row == 7);
        // -10: no length check at all, saturates to 0, always re-selects.
        assert!(page(&mut row, 8, -10, &mut ts) && row == 0 && ts.selected() == Some(0));
        ts.select(Some(9));
        assert!(page(&mut row, 8, -10, &mut ts) && row == 0 && ts.selected() == Some(0));
        row = 15;
        assert!(page(&mut row, 0, -10, &mut ts) && row == 5 && ts.selected() == Some(5));
    }
}
