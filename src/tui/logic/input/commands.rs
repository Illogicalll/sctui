use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

use super::InputOutcome;
use crate::tui::logic::state::{AppData, AppState, FollowingTracksFocus, PlaybackSource};
use crate::player::Player;
use crate::tui::logic::utils::build_queue;
use crate::tui::logic::utils::build_search_matches;

use super::queue::{handle_add_to_queue, handle_add_next_to_queue};

pub(crate) fn handle_char(
    key: KeyEvent,
    c: char,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        handle_shift_char(c, state, data, player)
    } else if state.selected_tab == 0 {
        handle_space(c, player)
    } else if state.selected_tab == 1 {
        handle_search_char(c, state)
    } else {
        InputOutcome::Continue
    }
}

pub(crate) fn handle_backspace(state: &mut AppState) -> InputOutcome {
    if state.selected_tab == 1 {
        state.query.pop();
    }
    InputOutcome::Continue
}

fn handle_shift_char(
    c: char,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    match c {
        'u' | 'U' => {
            player.volume_up();
        }
        'd' | 'D' => {
            player.volume_down();
        }
        's' | 'S' => {
            state.shuffle_enabled = !state.shuffle_enabled;
            if let Some(current_idx) = state.current_playing_index {
                let active_tracks = match state.playback_source {
                    PlaybackSource::Likes => &data.likes,
                    PlaybackSource::Playlist
                    | PlaybackSource::Album
                    | PlaybackSource::FollowingPublished
                    | PlaybackSource::FollowingLikes => &data.playback_tracks,
                };
                state.auto_queue =
                    build_queue(current_idx, active_tracks, state.shuffle_enabled);
            }
        }
        'r' | 'R' => {
            state.repeat_enabled = !state.repeat_enabled;
        }
        'a' | 'A' => {
            handle_add_to_queue(state, data);
        }
        'n' | 'N' => {
            handle_add_next_to_queue(state, data);
        }
        'f' | 'F' => {
            if state.selected_tab == 0 {
                state.search_popup_visible = true;
                state.search_query.clear();
                state.search_matches = build_search_matches(
                    state.selected_subtab,
                    &state.search_query,
                    &data.likes,
                    &data.playlists,
                    &data.albums,
                    &data.following,
                );
            }
        }
        'h' | 'H' => {
            state.help_visible = !state.help_visible;
        }
        'v' | 'V' => {
            state.visualizer_mode = !state.visualizer_mode;
        }
        'q' | 'Q' => {
            state.queue_visible = !state.queue_visible;
            if state.queue_visible {
                if let Some(current_idx) = state.current_playing_index {
                    if state.auto_queue.is_empty() {
                        let active_tracks = match state.playback_source {
                            PlaybackSource::Likes => &data.likes,
                            PlaybackSource::Playlist
                            | PlaybackSource::Album
                            | PlaybackSource::FollowingPublished
                            | PlaybackSource::FollowingLikes => &data.playback_tracks,
                        };
                        state.auto_queue = build_queue(
                            current_idx,
                            active_tracks,
                            state.shuffle_enabled,
                        );
                    }
                }
            }
        }
        'j' | 'J' => {
            if state.selected_tab == 0 && state.selected_subtab == 3 {
                if state.selected_following_like_row + 1 < data.following_likes_tracks.len() {
                    state.selected_following_like_row += 1;
                    state.following_tracks_focus = FollowingTracksFocus::Likes;
                    data.following_likes_state
                        .select(Some(state.selected_following_like_row));
                }
            }
        }
        'k' | 'K' => {
            if state.selected_tab == 0 && state.selected_subtab == 3 {
                if state.selected_following_like_row > 0 {
                    state.selected_following_like_row -= 1;
                    state.following_tracks_focus = FollowingTracksFocus::Likes;
                    data.following_likes_state
                        .select(Some(state.selected_following_like_row));
                }
            }
        }
        _ => {}
    }
    InputOutcome::Continue
}

fn handle_space(c: char, player: &Player) -> InputOutcome {
    if c == ' ' {
        if player.is_playing() {
            player.pause();
        } else {
            player.resume();
        }
    }
    InputOutcome::Continue
}

fn handle_search_char(c: char, state: &mut AppState) -> InputOutcome {
    state.query.push(c);
    InputOutcome::Continue
}
