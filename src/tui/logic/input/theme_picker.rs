use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use super::helpers::{list_step, lookup};
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
    let action = lookup(state, &key);
    if let Some(theme) = step(key, action, state) {
        theme::set(theme);
    }
    InputOutcome::Continue
}

/// The picker's state change for one key, and the theme to show as a result (the
/// preview, or the one to go back to). Applying it is the caller's job: the theme is
/// process-wide, so tests drive this and never set it. The wizard's theme stage is
/// this too.
pub(crate) fn step(key: KeyEvent, action: Option<Action>, state: &mut AppState) -> Option<Theme> {
    let last = Theme::BUILTIN.len() - 1;
    match (key.code, action) {
        (KeyCode::Esc, _) | (_, Some(Action::Themes)) => {
            state.theme_picker_visible = false;
            return state.theme_picker_previous.take();
        }
        (KeyCode::Enter, _) => {
            let chosen = Theme::BUILTIN[state.theme_picker_selected.min(last)];
            state.theme_name = chosen.name.to_string();
            state.theme_picker_previous = None;
            state.theme_picker_visible = false;
            match crate::config::save(
                state.config_load_failed,
                &state.keymap,
                &state.theme_name,
                &state.theme_overrides,
                &state.settings,
                &state.eq,
            ) {
                Ok(()) => state.toast(format!("Theme: {}", chosen.name)),
                Err(e) => state.toast_err(format!("Theme not saved: {e}")),
            }
        }
        _ => {
            if let Some(next) = list_step(&key, action, state.theme_picker_selected, last) {
                state.theme_picker_selected = next;
                return Some(Theme::BUILTIN[next].with_overrides(&state.theme_overrides).0);
            }
        }
    }
    None
}
