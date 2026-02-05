use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

use super::InputOutcome;
use crate::tui::logic::state::{AppData, AppState, info_table_rows_count, table_rows_count, FollowingTracksFocus};

pub(crate) fn handle_down_key(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
) -> InputOutcome {
    if state.selected_tab == 0 && state.selected_subtab == 1 {
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
    if state.selected_tab == 0 && state.selected_subtab == 1 {
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
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if state.selected_row + 1 < data.playlists.len() {
            state.selected_row += 1;
            state.selected_playlist_row = state.selected_row;
            data.playlists_state.select(Some(state.selected_row));
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        if !data.playlist_tracks.is_empty() {
            state.selected_playlist_track_row = (state.selected_playlist_track_row + 10)
                .min(data.playlist_tracks.len() - 1);
            data.playlist_tracks_state.select(Some(state.selected_playlist_track_row));
        }
    } else if state.selected_playlist_track_row + 1 < data.playlist_tracks.len() {
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
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if state.selected_row > 0 {
            state.selected_row -= 1;
            state.selected_playlist_row = state.selected_row;
            data.playlists_state.select(Some(state.selected_row));
        }
    } else if key.modifiers.contains(KeyModifiers::ALT) {
        state.selected_playlist_track_row = state.selected_playlist_track_row.saturating_sub(10);
        data.playlist_tracks_state.select(Some(state.selected_playlist_track_row));
    } else if state.selected_playlist_track_row > 0 {
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
