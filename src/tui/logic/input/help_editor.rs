use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::keymap::{Action, Chord};
use crate::tui::logic::state::AppState;

/// The `?` popup: a modal key editor. Owns every key while open.
pub(crate) fn handle_help_input(key: KeyEvent, state: &mut AppState) -> InputOutcome {
    let last = Action::ALL.len() - 1;
    let selected = Action::ALL[state.help_selected.min(last)];

    // Waiting for the key to bind.
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

/// Persist and decorate the message with the outcome.
fn saved(state: &AppState, msg: String) -> String {
    match state.keymap.save() {
        Ok(()) => format!("{msg}  · saved"),
        Err(e) => format!("{msg}  · NOT saved: {e}"),
    }
}
