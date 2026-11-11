use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::helpers::edit_text;
use crate::theme::{self, Theme};
use crate::tui::logic::nav;
use crate::tui::logic::palette::{Command, matches};
use crate::tui::logic::state::AppState;

/// The palette is insert mode: every printable key types into the query and no bound
/// action fires. Up/Down (also Tab/Shift+Tab, Ctrl+N/P, Ctrl+J/K) move, Esc closes, and
/// Enter closes and hands back the highlighted command for the caller to run.
pub(crate) fn handle_palette_input(key: KeyEvent, state: &mut AppState) -> Option<Command> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let step = match key.code {
        KeyCode::Esc => {
            state.palette = None;
            return None;
        }
        KeyCode::Enter => {
            let palette = state.palette.take()?;
            return matches(state, &palette.query).into_iter().nth(palette.selected).map(|(_, _, command)| command);
        }
        KeyCode::Down | KeyCode::Tab => 1,
        KeyCode::Up | KeyCode::BackTab => -1,
        KeyCode::Char('n' | 'j') if ctrl => 1,
        KeyCode::Char('p' | 'k') if ctrl => -1,
        _ => {
            let palette = state.palette.as_mut()?;
            if edit_text(&key, &mut palette.query) {
                palette.selected = 0;
            }
            return None;
        }
    };
    let last = matches(state, &state.palette.as_ref()?.query).len().saturating_sub(1);
    let palette = state.palette.as_mut()?;
    palette.selected = palette.selected.saturating_add_signed(step).min(last);
    None
}

/// Run a palette command that is not a keymap action (the caller dispatches those):
/// apply it, save the config, and say what changed.
pub(crate) fn run(command: Command, state: &mut AppState) {
    let said = match command {
        Command::Action(_) => return,
        Command::Layout(layout) => {
            state.settings.layout = layout;
            state.nav_focused &= nav::has_nav(layout);
            format!("Layout: {}", layout.label())
        }
        Command::Theme(name) => {
            let Some(base) = Theme::by_name(name) else { return };
            theme::set(base.with_overrides(&state.theme_overrides).0);
            state.theme_name = name.to_string();
            format!("Theme: {name}")
        }
        Command::ToggleHints => {
            state.settings.hint_line = !state.settings.hint_line;
            format!("Hint line {}", if state.settings.hint_line { "on" } else { "off" })
        }
        Command::ToggleHideUnplayable => {
            // The loop notices the change and filters the lists.
            state.settings.hide_unplayable = !state.settings.hide_unplayable;
            format!("Hide unplayable tracks {}", if state.settings.hide_unplayable { "on" } else { "off" })
        }
        Command::SetupWizard => return crate::tui::logic::wizard::start(state),
        Command::Help(page) => {
            state.help_visible = true;
            state.help_page = page;
            state.help_selected = 0;
            state.help_settings_selected = 0;
            state.help_eq_selected = 0;
            state.help_capture = None;
            state.help_message = None;
            return;
        }
    };
    match crate::config::save(
        state.config_load_failed,
        &state.keymap,
        &state.theme_name,
        &state.theme_overrides,
        &state.settings,
        &state.eq,
    ) {
        Ok(()) => state.toast(said),
        Err(e) => state.toast_err(format!("{said} · not saved: {e}")),
    }
}
