use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::keymap::Action;
use super::helpers::{lookup, toggles_binary_choice};
use crate::tui::logic::state::AppState;

pub(crate) fn handle_quit_confirm(key: KeyEvent, state: &mut AppState) -> InputOutcome {
    let action = lookup(state, &key);
    match key.code {
        // Esc, or the key that asked, cancels.
        _ if key.code == KeyCode::Esc || action == Some(Action::Quit) => {
            state.quit_confirm_visible = false;
            InputOutcome::Continue
        }
        KeyCode::Enter => {
            if state.quit_confirm_selected == 0 {
                InputOutcome::Quit
            } else {
                state.quit_confirm_visible = false;
                InputOutcome::Continue
            }
        }
        _ if toggles_binary_choice(&key, action) => {
            state.quit_confirm_selected = if state.quit_confirm_selected == 0 { 1 } else { 0 };
            InputOutcome::Continue
        }
        _ => InputOutcome::Continue,
    }
}
