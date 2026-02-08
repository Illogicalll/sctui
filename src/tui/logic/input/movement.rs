use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

use super::InputOutcome;
use crate::tui::logic::state::{AppData, AppState, info_table_rows_count, table_rows_count, FollowingTracksFocus};

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
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        handle_alt_down(key, state, data);
    } else {
        handle_normal_down(key, state, data);
    }
    InputOutcome::Continue
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
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        handle_alt_up(key, state, data);
    } else {
        handle_normal_up(key, state, data);
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
        if state.selected_row + 1 < data.playlists.len() {
            state.selected_row += 1;
            state.selected_playlist_row = state.selected_row;
            data.playlists_state.select(Some(state.selected_row));
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        if playlist_tracks_len > 0 {
            state.selected_playlist_track_row = (state.selected_playlist_track_row + 10)
                .min(playlist_tracks_len - 1);
            data.playlist_tracks_state.select(Some(state.selected_playlist_track_row));
        }
    } else if state.selected_playlist_track_row + 1 < playlist_tracks_len {
        state.selected_playlist_track_row += 1;
        data.playlist_tracks_state.select(Some(state.selected_playlist_track_row));
    }
}

fn handle_album_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if state.selected_row + 1 < data.albums.len() {
            state.selected_row += 1;
            state.selected_album_row = state.selected_row;
            data.albums_state.select(Some(state.selected_row));
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        if !data.album_tracks.is_empty() {
            state.selected_album_track_row =
                (state.selected_album_track_row + 10).min(data.album_tracks.len() - 1);
            data.album_tracks_state.select(Some(state.selected_album_track_row));
        }
    } else if state.selected_album_track_row + 1 < data.album_tracks.len() {
        state.selected_album_track_row += 1;
        data.album_tracks_state.select(Some(state.selected_album_track_row));
    }
}

fn handle_following_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if state.selected_row + 1 < data.following.len() {
            state.selected_row += 1;
            data.following_state.select(Some(state.selected_row));
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        if !data.following_tracks.is_empty() {
            state.selected_following_track_row =
                (state.selected_following_track_row + 10).min(data.following_tracks.len() - 1);
            state.following_tracks_focus = FollowingTracksFocus::Published;
            data.following_tracks_state.select(Some(state.selected_following_track_row));
        }
    } else if state.selected_following_track_row + 1 < data.following_tracks.len() {
        state.selected_following_track_row += 1;
        state.following_tracks_focus = FollowingTracksFocus::Published;
        data.following_tracks_state.select(Some(state.selected_following_track_row));
    }
}

fn handle_alt_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    let max_rows = table_rows_count(state.selected_subtab, data);
    let max_info_rows = info_table_rows_count();
    if state.selected_tab == 2 && state.info_pane_selected {
        if max_info_rows > 0 {
            state.selected_info_row = (state.selected_info_row + 10).min(max_info_rows - 1);
        }
    } else if max_rows > 0 {
        state.selected_row = (state.selected_row + 10).min(max_rows - 1);
        match state.selected_subtab {
            0 => data.likes_state.select(Some(state.selected_row)),
            1 => data.playlists_state.select(Some(state.selected_row)),
            2 => data.albums_state.select(Some(state.selected_row)),
            3 => data.following_state.select(Some(state.selected_row)),
            _ => {}
        }
    }
}

fn handle_normal_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    let max_rows = table_rows_count(state.selected_subtab, data);
    let max_info_rows = info_table_rows_count();
    if state.selected_tab == 2
        && state.info_pane_selected
        && state.selected_info_row + 1 < max_info_rows
    {
        state.selected_info_row += 1;
    } else if state.selected_row + 1 < max_rows {
        state.selected_row += 1;
        if state.selected_subtab == 1 && state.selected_tab == 0 {
            state.selected_playlist_row = state.selected_row;
        }
        if state.selected_subtab == 2 && state.selected_tab == 0 {
            state.selected_album_row = state.selected_row;
        }
        match state.selected_subtab {
            0 => data.likes_state.select(Some(state.selected_row)),
            1 => data.playlists_state.select(Some(state.selected_row)),
            2 => data.albums_state.select(Some(state.selected_row)),
            3 => data.following_state.select(Some(state.selected_row)),
            _ => {}
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
        if state.selected_row > 0 {
            state.selected_row -= 1;
            state.selected_playlist_row = state.selected_row;
            data.playlists_state.select(Some(state.selected_row));
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        if playlist_tracks_len > 0 {
            state.selected_playlist_track_row =
                state.selected_playlist_track_row.saturating_sub(10);
            data.playlist_tracks_state.select(Some(state.selected_playlist_track_row));
        }
    } else if state.selected_playlist_track_row > 0 && playlist_tracks_len > 0 {
        state.selected_playlist_track_row -= 1;
        data.playlist_tracks_state.select(Some(state.selected_playlist_track_row));
    }
}

fn handle_album_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if state.selected_row > 0 {
            state.selected_row -= 1;
            state.selected_album_row = state.selected_row;
            data.albums_state.select(Some(state.selected_row));
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        state.selected_album_track_row = state.selected_album_track_row.saturating_sub(10);
        data.album_tracks_state.select(Some(state.selected_album_track_row));
    } else if state.selected_album_track_row > 0 {
        state.selected_album_track_row -= 1;
        data.album_tracks_state.select(Some(state.selected_album_track_row));
    }
}

fn handle_following_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if state.selected_row > 0 {
            state.selected_row -= 1;
            data.following_state.select(Some(state.selected_row));
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        state.selected_following_track_row =
            state.selected_following_track_row.saturating_sub(10);
        state.following_tracks_focus = FollowingTracksFocus::Published;
        data.following_tracks_state.select(Some(state.selected_following_track_row));
    } else if state.selected_following_track_row > 0 {
        state.selected_following_track_row -= 1;
        state.following_tracks_focus = FollowingTracksFocus::Published;
        data.following_tracks_state.select(Some(state.selected_following_track_row));
    }
}

fn handle_alt_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if state.selected_tab == 2 && state.info_pane_selected {
        state.selected_info_row = state.selected_info_row.saturating_sub(10);
    } else {
        state.selected_row = state.selected_row.saturating_sub(10);
        match state.selected_subtab {
            0 => data.likes_state.select(Some(state.selected_row)),
            1 => data.playlists_state.select(Some(state.selected_row)),
            2 => data.albums_state.select(Some(state.selected_row)),
            3 => data.following_state.select(Some(state.selected_row)),
            _ => {}
        }
    }
}

fn handle_normal_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    if state.selected_tab == 2 && state.info_pane_selected && state.selected_info_row > 0 {
        state.selected_info_row -= 1;
    } else if state.selected_row > 0 {
        state.selected_row -= 1;
        if state.selected_subtab == 1 && state.selected_tab == 0 {
            state.selected_playlist_row = state.selected_row;
        }
        if state.selected_subtab == 2 && state.selected_tab == 0 {
            state.selected_album_row = state.selected_row;
        }
        match state.selected_subtab {
            0 => data.likes_state.select(Some(state.selected_row)),
            1 => data.playlists_state.select(Some(state.selected_row)),
            2 => data.albums_state.select(Some(state.selected_row)),
            3 => data.following_state.select(Some(state.selected_row)),
            _ => {}
        }
    }
}

fn handle_search_down(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    match state.selected_searchfilter {
        0 => {
            let max_rows = data.search_tracks.len();
            if key.modifiers.contains(KeyModifiers::ALT) {
                if max_rows > 0 {
                    state.selected_row = (state.selected_row + 10).min(max_rows - 1);
                }
            } else if state.selected_row + 1 < max_rows {
                state.selected_row += 1;
            }
            data.search_tracks_state.select(Some(state.selected_row));
        }
        1 => {
            let tracks_len = data.search_album_tracks.len();
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if state.selected_row + 1 < data.search_albums.len() {
                    state.selected_row += 1;
                    data.search_albums_state.select(Some(state.selected_row));
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                if tracks_len > 0 {
                    state.search_selected_album_track_row =
                        (state.search_selected_album_track_row + 10).min(tracks_len - 1);
                    data.search_album_tracks_state
                        .select(Some(state.search_selected_album_track_row));
                }
            } else if state.search_selected_album_track_row + 1 < tracks_len {
                state.search_selected_album_track_row += 1;
                data.search_album_tracks_state
                    .select(Some(state.search_selected_album_track_row));
            }
        }
        2 => {
            let tracks_len = data.search_playlist_tracks.len();
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if state.selected_row + 1 < data.search_playlists.len() {
                    state.selected_row += 1;
                    data.search_playlists_state.select(Some(state.selected_row));
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                if tracks_len > 0 {
                    state.search_selected_playlist_track_row =
                        (state.search_selected_playlist_track_row + 10).min(tracks_len - 1);
                    data.search_playlist_tracks_state
                        .select(Some(state.search_selected_playlist_track_row));
                }
            } else if state.search_selected_playlist_track_row + 1 < tracks_len {
                state.search_selected_playlist_track_row += 1;
                data.search_playlist_tracks_state
                    .select(Some(state.search_selected_playlist_track_row));
            }
        }
        3 => {
            let tracks_len = data.search_people_tracks.len();
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if state.selected_row + 1 < data.search_people.len() {
                    state.selected_row += 1;
                    data.search_people_state.select(Some(state.selected_row));
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                if tracks_len > 0 {
                    state.search_selected_person_track_row =
                        (state.search_selected_person_track_row + 10).min(tracks_len - 1);
                    state.search_people_tracks_focus = FollowingTracksFocus::Published;
                    data.search_people_tracks_state
                        .select(Some(state.search_selected_person_track_row));
                }
            } else if state.search_selected_person_track_row + 1 < tracks_len {
                state.search_selected_person_track_row += 1;
                state.search_people_tracks_focus = FollowingTracksFocus::Published;
                data.search_people_tracks_state
                    .select(Some(state.search_selected_person_track_row));
            }
        }
        _ => {}
    }
}

fn handle_search_up(key: KeyEvent, state: &mut AppState, data: &mut AppData) {
    match state.selected_searchfilter {
        0 => {
            if key.modifiers.contains(KeyModifiers::ALT) {
                state.selected_row = state.selected_row.saturating_sub(10);
            } else if state.selected_row > 0 {
                state.selected_row -= 1;
            }
            data.search_tracks_state.select(Some(state.selected_row));
        }
        1 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if state.selected_row > 0 {
                    state.selected_row -= 1;
                    data.search_albums_state.select(Some(state.selected_row));
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                state.search_selected_album_track_row =
                    state.search_selected_album_track_row.saturating_sub(10);
                data.search_album_tracks_state
                    .select(Some(state.search_selected_album_track_row));
            } else if state.search_selected_album_track_row > 0 {
                state.search_selected_album_track_row -= 1;
                data.search_album_tracks_state
                    .select(Some(state.search_selected_album_track_row));
            }
        }
        2 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if state.selected_row > 0 {
                    state.selected_row -= 1;
                    data.search_playlists_state.select(Some(state.selected_row));
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                state.search_selected_playlist_track_row =
                    state.search_selected_playlist_track_row.saturating_sub(10);
                data.search_playlist_tracks_state
                    .select(Some(state.search_selected_playlist_track_row));
            } else if state.search_selected_playlist_track_row > 0 {
                state.search_selected_playlist_track_row -= 1;
                data.search_playlist_tracks_state
                    .select(Some(state.search_selected_playlist_track_row));
            }
        }
        3 => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if state.selected_row > 0 {
                    state.selected_row -= 1;
                    data.search_people_state.select(Some(state.selected_row));
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                state.search_selected_person_track_row =
                    state.search_selected_person_track_row.saturating_sub(10);
                state.search_people_tracks_focus = FollowingTracksFocus::Published;
                data.search_people_tracks_state
                    .select(Some(state.search_selected_person_track_row));
            } else if state.search_selected_person_track_row > 0 {
                state.search_selected_person_track_row -= 1;
                state.search_people_tracks_focus = FollowingTracksFocus::Published;
                data.search_people_tracks_state
                    .select(Some(state.search_selected_person_track_row));
            }
        }
        _ => {}
    }
}
