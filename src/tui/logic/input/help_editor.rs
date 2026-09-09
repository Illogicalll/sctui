use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::config::{SettingRow, Settings};
use crate::keymap::{Action, Chord};
use crate::player::Player;
use crate::tui::logic::filtering::apply_unplayable_filter;
use crate::tui::logic::state::{AppData, AppState, visible_tabs};

/// The `?` popup: a modal key editor, with a settings page behind Tab. Owns every
/// key while open.
pub(crate) fn handle_help_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    let last = Action::ALL.len() - 1;
    let selected = Action::ALL[state.help_selected.min(last)];

    // Waiting for the key to bind. Tab is a bindable key, so this comes first.
    if let Some(action) = state.help_capture.take() {
        if key.code == KeyCode::Esc {
            state.help_message = Some("Cancelled".to_string());
            return InputOutcome::Continue;
        }
        let chord = Chord::from_key(&key);
        state.help_message = Some(match state.keymap.add_chord(action, chord) {
            Ok(()) => saved(state, format!("{} → {}", chord.label(), action.description())),
            Err(other) => format!("{} is already bound to {}", chord.label(), other.description()),
        });
        return InputOutcome::Continue;
    }

    if key.code == KeyCode::Tab {
        state.help_settings = !state.help_settings;
        state.help_message = None;
        return InputOutcome::Continue;
    }
    if state.help_settings {
        return handle_settings_input(key, state, data, player);
    }

    state.help_message = None;
    match (key.code, state.keymap.action(&key)) {
        (KeyCode::Esc, _) | (_, Some(Action::Help)) => state.help_visible = false,
        (KeyCode::Enter, _) => {
            state.help_capture = Some(selected);
            state.help_message = Some(format!("Press a key for \"{}\"  (Esc cancels)", selected.description()));
        }
        (KeyCode::Backspace | KeyCode::Delete, _) => {
            state.keymap.clear(selected);
            state.help_message = Some(saved(state, format!("Unbound {}", selected.description())));
        }
        (_, Some(Action::Up)) | (KeyCode::Up, _) => state.help_selected = state.help_selected.saturating_sub(1),
        (_, Some(Action::Down)) | (KeyCode::Down, _) => state.help_selected = (state.help_selected + 1).min(last),
        (_, Some(Action::PageUp)) | (KeyCode::PageUp, _) => state.help_selected = state.help_selected.saturating_sub(10),
        (_, Some(Action::PageDown)) | (KeyCode::PageDown, _) => state.help_selected = (state.help_selected + 10).min(last),
        (KeyCode::Home, _) => state.help_selected = 0,
        (KeyCode::End, _) => state.help_selected = last,
        (KeyCode::Char('r'), _) => {
            let notes = state.keymap.reset(selected);
            let mut msg = format!("Default restored for {}", selected.description());
            if !notes.is_empty() {
                msg.push_str(&format!(" ({})", notes.join(", ")));
            }
            state.help_message = Some(saved(state, msg));
        }
        _ => {}
    }
    InputOutcome::Continue
}

/// The settings page: Enter or Space flips the highlighted toggle, left/right
/// nudges the highlighted duration.
fn handle_settings_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    let last = Settings::ROWS.len() - 1;
    let row = Settings::ROWS[state.help_settings_selected.min(last)];
    state.help_message = None;
    match (key.code, state.keymap.action(&key)) {
        (KeyCode::Esc, _) | (_, Some(Action::Help)) => state.help_visible = false,
        (KeyCode::Enter | KeyCode::Char(' '), _) => {
            if let SettingRow::Toggle(label, field) | SettingRow::CrossfadeToggle(label, field) = row {
                let value = {
                    let flag = field(&mut state.settings);
                    *flag = !*flag;
                    *flag
                };
                apply_settings(state, data, player);
                let on = if value { "on" } else { "off" };
                state.help_message = Some(saved(state, format!("{label}: {on}")));
            }
        }
        (KeyCode::Left, _) | (_, Some(Action::SubTabLeft)) => adjust(row, state, data, player, -1),
        (KeyCode::Right, _) | (_, Some(Action::SubTabRight)) => adjust(row, state, data, player, 1),
        (_, Some(Action::Up)) | (KeyCode::Up, _) => {
            state.help_settings_selected = state.help_settings_selected.saturating_sub(1)
        }
        (_, Some(Action::Down)) | (KeyCode::Down, _) => {
            state.help_settings_selected = (state.help_settings_selected + 1).min(last)
        }
        (KeyCode::Home, _) => state.help_settings_selected = 0,
        (KeyCode::End, _) => state.help_settings_selected = last,
        _ => {}
    }
    InputOutcome::Continue
}

/// Move a duration row by `delta` seconds, within its allowed range.
fn adjust(row: SettingRow, state: &mut AppState, data: &mut AppData, player: &Player, delta: i16) {
    let SettingRow::Secs(label, field) = row else { return };
    let secs = {
        let value = field(&mut state.settings);
        *value = (i16::from(*value) + delta).clamp(
            i16::from(Settings::CROSSFADE_SECS_MIN),
            i16::from(Settings::CROSSFADE_SECS_MAX),
        ) as u8;
        *value
    };
    apply_settings(state, data, player);
    state.help_message = Some(saved(state, format!("{label}: {secs}s")));
}

/// Make a just-changed setting take effect on what is already loaded.
fn apply_settings(state: &mut AppState, data: &mut AppData, player: &Player) {
    apply_unplayable_filter(state, data);
    if state.selected_tab >= visible_tabs(state).len() {
        state.selected_tab = 0;
        state.selected_row = 0;
    }
    player.set_crossfade(state.settings.crossfade_ms(), state.settings.crossfade_user_skips);
}

/// Persist and decorate the message with the outcome.
fn saved(state: &AppState, msg: String) -> String {
    match crate::config::save(
        &state.keymap,
        &state.theme_name,
        &state.theme_overrides,
        &state.settings,
    ) {
        Ok(()) => format!("{msg}  · saved"),
        Err(e) => format!("{msg}  · NOT saved: {e}"),
    }
}
