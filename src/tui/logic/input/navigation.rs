use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

use super::InputOutcome;
use crate::api::Track;
use crate::player::Player;
use crate::tui::logic::state::{AppData, AppState, PlaybackSource};
use crate::tui::logic::utils::{build_queue, build_search_matches};

pub(crate) fn handle_tab_switch(state: &mut AppState) -> InputOutcome {
    state.selected_tab = (state.selected_tab + 1) % 3;
    state.selected_row = 0;
    InputOutcome::Continue
}

pub(crate) fn handle_right_key(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if key.modifiers.contains(KeyModifiers::ALT) {
        if player.is_playing() || state.current_playing_index.is_some() {
            player.fast_forward();
        }
        return InputOutcome::Continue;
    }

    if key.modifiers.contains(KeyModifiers::SHIFT) {
        return handle_next_track(state, data, player);
    }

    if state.selected_tab == 0 {
        if state.selected_subtab == 1 {
            state.selected_playlist_row = state.selected_row;
        }
        if state.selected_subtab == 2 {
            state.selected_album_row = state.selected_row;
        }
        state.selected_subtab = (state.selected_subtab + 1) % 4;
        if state.selected_subtab == 1 {
            state.selected_row = state.selected_playlist_row;
            data.playlists_state.select(Some(state.selected_row));
        } else if state.selected_subtab == 2 {
            state.selected_row = state.selected_album_row;
            data.albums_state.select(Some(state.selected_row));
        } else {
            state.selected_row = 0;
        }

        if state.search_popup_visible && !state.search_query.trim().is_empty() {
            state.search_matches = build_search_matches(
                state.selected_subtab,
                &state.search_query,
                &data.likes,
                &data.playlists,
                &data.albums,
                &data.following,
            );
            state.selected_row = 0;
            match state.selected_subtab {
                0 => data.likes_state.select(Some(0)),
                1 => {
                    state.selected_playlist_row = 0;
                    data.playlists_state.select(Some(0));
                }
                2 => {
                    state.selected_album_row = 0;
                    data.albums_state.select(Some(0));
                }
                3 => data.following_state.select(Some(0)),
                _ => {}
            }
        }
    } else if state.selected_tab == 1 {
        state.selected_searchfilter = (state.selected_searchfilter + 1) % 4;
        state.selected_row = 0;
    } else if state.selected_tab == 2 {
        state.info_pane_selected = !state.info_pane_selected;
    }

    InputOutcome::Continue
}

pub(crate) fn handle_left_key(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if key.modifiers.contains(KeyModifiers::ALT) {
        if player.is_playing() || state.current_playing_index.is_some() {
            player.rewind();
        }
        return InputOutcome::Continue;
    }

    if key.modifiers.contains(KeyModifiers::SHIFT) {
        return handle_prev_track(state, data, player);
    }

    if state.selected_tab == 0 {
        if state.selected_subtab == 1 {
            state.selected_playlist_row = state.selected_row;
        }
        if state.selected_subtab == 2 {
            state.selected_album_row = state.selected_row;
        }
        state.selected_subtab = if state.selected_subtab == 0 {
            3
        } else {
            state.selected_subtab - 1
        };
        if state.selected_subtab == 1 {
            state.selected_row = state.selected_playlist_row;
            data.playlists_state.select(Some(state.selected_row));
        } else if state.selected_subtab == 2 {
            state.selected_row = state.selected_album_row;
            data.albums_state.select(Some(state.selected_row));
        } else {
            state.selected_row = 0;
        }

        if state.search_popup_visible && !state.search_query.trim().is_empty() {
            state.search_matches = build_search_matches(
                state.selected_subtab,
                &state.search_query,
                &data.likes,
                &data.playlists,
                &data.albums,
                &data.following,
            );
            state.selected_row = 0;
            match state.selected_subtab {
                0 => data.likes_state.select(Some(0)),
                1 => {
                    state.selected_playlist_row = 0;
                    data.playlists_state.select(Some(0));
                }
                2 => {
                    state.selected_album_row = 0;
                    data.albums_state.select(Some(0));
                }
                3 => data.following_state.select(Some(0)),
                _ => {}
            }
        }
    } else if state.selected_tab == 1 {
        state.selected_searchfilter = if state.selected_searchfilter == 0 {
            3
        } else {
            state.selected_searchfilter - 1
        };
        state.selected_row = 0;
    } else if state.selected_tab == 2 {
        state.info_pane_selected = !state.info_pane_selected;
    }

    InputOutcome::Continue
}

fn handle_next_track(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if let Some(current_idx) = state.current_playing_index {
        let active_tracks = match state.playback_source {
            PlaybackSource::Likes => &data.likes,
            PlaybackSource::Playlist
            | PlaybackSource::Album
            | PlaybackSource::FollowingPublished
            | PlaybackSource::FollowingLikes => &data.playback_tracks,
        };
        if state.manual_queue.is_empty() && state.auto_queue.is_empty() {
            state.auto_queue =
                build_queue(current_idx, active_tracks, state.shuffle_enabled);
        }
        if let Some(queued) = state.manual_queue.pop_front() {
            if let Some(current) = crate::tui::logic::utils::queued_from_current(state, data) {
                state.playback_history.push(current);
            }
            crate::tui::logic::utils::play_queued_track(queued, state, data, player, true);
        } else if let Some(next_idx) = state.auto_queue.pop_front() {
            if let Some(track) = active_tracks.get(next_idx) {
                if let Some(current) = crate::tui::logic::utils::queued_from_current(state, data) {
                    state.playback_history.push(current);
                }
                player.play(track.clone());
                state.override_playing = None;
                state.current_playing_index = Some(next_idx);
            }
        }
    }
    InputOutcome::Continue
}

fn handle_prev_track(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if state.current_playing_index.is_some() {
        if let Some(prev) = state.playback_history.pop() {
            if let Some(current) = crate::tui::logic::utils::queued_from_current(state, data) {
                let mut current = current;
                current.user_added = false;
                state.manual_queue.push_front(current);
            }
            crate::tui::logic::utils::play_queued_track(prev, state, data, player, true);
        }
    }
    InputOutcome::Continue
}
