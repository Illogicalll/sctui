use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::keymap::Action;
use crate::player::Player;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::utils::{play_queued_track, queued_from_current};

/// Keys the history popup owns while it is open. Movement follows the user's
/// keymap; Esc/Enter/Home/End are fixed. `None` lets the key fall through.
pub(crate) fn handle_history_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> Option<InputOutcome> {
    let last = state.playback_history.len().saturating_sub(1);
    let sel = &mut state.history_selected;
    match (key.code, state.keymap.action(&key)) {
        (KeyCode::Esc, _) => state.history_visible = false,
        (_, Some(Action::Up)) => *sel = sel.saturating_sub(1),
        (_, Some(Action::Down)) => *sel = (*sel + 1).min(last),
        (_, Some(Action::PageUp)) => *sel = sel.saturating_sub(10),
        (_, Some(Action::PageDown)) => *sel = (*sel + 10).min(last),
        (KeyCode::Home, _) => *sel = 0,
        (KeyCode::End, _) => *sel = last,
        (KeyCode::Enter, _) => {
            // The popup lists newest first; map the row back to the Vec index.
            let idx = last.saturating_sub(state.history_selected);
            if let Some(entry) = state.playback_history.get(idx).cloned() {
                // Same as "previous track": what is playing now goes into history, the
                // chosen track plays as an override and the queue carries on after it.
                if let Some(current) = queued_from_current(state, data) {
                    state.playback_history.push(current);
                }
                play_queued_track(entry, state, data, player, true);
                state.history_visible = false;
            }
        }
        _ => return None,
    }
    Some(InputOutcome::Continue)
}
