use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::InputOutcome;
use crate::player::Player;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::utils::{play_queued_track, queued_from_current};

/// Keys the history popup owns while it is open. `None` lets the key fall
/// through to the normal handlers (play/pause, volume, the Shift+P toggle...).
pub(crate) fn handle_history_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> Option<InputOutcome> {
    let last = state.playback_history.len().saturating_sub(1);
    let step = if key.modifiers.contains(KeyModifiers::ALT) { 10 } else { 1 };

    match key.code {
        KeyCode::Esc => state.history_visible = false,
        KeyCode::Up => state.history_selected = state.history_selected.saturating_sub(step),
        KeyCode::Down => state.history_selected = (state.history_selected + step).min(last),
        KeyCode::Home => state.history_selected = 0,
        KeyCode::End => state.history_selected = last,
        KeyCode::Enter => {
            // The popup lists newest first; map the row back to the Vec index.
            let idx = last.saturating_sub(state.history_selected);
            if let Some(entry) = state.playback_history.get(idx).cloned() {
                // Same as Shift+Left: what is playing now goes into history, the
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
