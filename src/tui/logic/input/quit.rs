use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::tui::logic::state::AppState;

pub(crate) fn handle_quit_confirm(key: KeyEvent, state: &mut AppState) -> InputOutcome {
    match key.code {
        KeyCode::Esc => {
            state.quit_confirm_visible = false;
            InputOutcome::Continue
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
            state.quit_confirm_selected = if state.quit_confirm_selected == 0 { 1 } else { 0 };
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
        _ => InputOutcome::Continue,
    }
}
