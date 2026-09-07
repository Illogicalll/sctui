use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::keymap::Action;
use crate::theme::{self, Theme};
use crate::tui::logic::state::AppState;

pub(crate) fn open_theme_picker(state: &mut AppState) {
    state.theme_picker_previous = Some(theme::current());
    state.theme_picker_selected = Theme::BUILTIN
        .iter()
        .position(|t| t.name == state.theme_name)
        .unwrap_or(0);
    state.theme_picker_visible = true;
}

/// Modal while open. Moving previews the theme; Enter keeps and saves it,
/// Esc restores what was active before.
pub(crate) fn handle_theme_picker_input(key: KeyEvent, state: &mut AppState) -> InputOutcome {
    let last = Theme::BUILTIN.len() - 1;
    match (key.code, state.keymap.action(&key)) {
        (KeyCode::Esc, _) => {
            if let Some(previous) = state.theme_picker_previous.take() {
                theme::set(previous);
            }
            state.theme_picker_visible = false;
        }
        (KeyCode::Enter, _) => {
            let chosen = Theme::BUILTIN[state.theme_picker_selected.min(last)];
            state.theme_name = chosen.name.to_string();
            state.theme_picker_previous = None;
            state.theme_picker_visible = false;
            state.help_message = crate::config::save(&state.keymap, &state.theme_name, &state.theme_overrides)
                .err()
                .map(|e| format!("theme not saved: {e}"));
        }
        (_, Some(Action::Up)) | (KeyCode::Up, _) => {
            state.theme_picker_selected = state.theme_picker_selected.saturating_sub(1);
            preview(state);
        }
        (_, Some(Action::Down)) | (KeyCode::Down, _) => {
            state.theme_picker_selected = (state.theme_picker_selected + 1).min(last);
            preview(state);
        }
        (KeyCode::Home, _) => {
            state.theme_picker_selected = 0;
            preview(state);
        }
        (KeyCode::End, _) => {
            state.theme_picker_selected = last;
            preview(state);
        }
        _ => {}
    }
    InputOutcome::Continue
}

fn preview(state: &AppState) {
    let base = Theme::BUILTIN[state.theme_picker_selected.min(Theme::BUILTIN.len() - 1)];
    let (resolved, _) = base.with_overrides(&state.theme_overrides);
    theme::set(resolved);
}
