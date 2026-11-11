use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use super::helpers::{list_step, lookup};
use crate::keymap::Action;
use crate::player::{Player, TrackChange};
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::utils::{play_queued_track, queued_from_current};

/// The history popup is modal: it owns every key while it is open. Movement follows
/// the user's keymap; Esc or its own key closes it.
pub(crate) fn handle_history_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    let last = state.playback_history.len().saturating_sub(1);
    let action = lookup(state, &key);
    match (key.code, action) {
        (KeyCode::Esc, _) | (_, Some(Action::ToggleHistory)) => state.history_visible = false,
        (KeyCode::Enter, _) if state.stream_held() => {}
        (KeyCode::Enter, _) => {
            // The popup lists newest first; map the row back to the Vec index.
            let idx = last.saturating_sub(state.history_selected);
            if let Some(entry) = state.playback_history.get(idx).cloned() {
                // Same as "previous track": what is playing now goes into history, the
                // chosen track plays as an override and the queue carries on after it.
                if let Some(current) = queued_from_current(state, data) {
                    state.push_history(current);
                }
                state.cancel_artist_station();
                play_queued_track(entry, state, data, player, true, TrackChange::UserSkip);
                player.resume();
                state.history_visible = false;
            }
        }
        _ => state.history_selected = list_step(&key, action, state.history_selected, last).unwrap_or(state.history_selected),
    }
    InputOutcome::Continue
}
