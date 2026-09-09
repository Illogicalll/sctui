use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::config::Settings;
use crate::keymap::{Action, Chord};
use crate::player::eq;
use crate::tui::logic::filtering::apply_unplayable_filter;
use crate::tui::logic::state::{AppData, AppState, HelpPage, visible_tabs};

/// The `?` popup: a modal key editor, with a settings page and an equaliser page
/// behind Tab. Owns every key while open.
pub(crate) fn handle_help_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
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
        state.help_page = state.help_page.next();
        state.help_message = None;
        return InputOutcome::Continue;
    }
    match state.help_page {
        HelpPage::Settings => return handle_settings_input(key, state, data),
        HelpPage::Equalizer => return handle_eq_input(key, state),
        HelpPage::Keys => {}
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

/// The settings page: Enter or Space flips the highlighted toggle.
fn handle_settings_input(key: KeyEvent, state: &mut AppState, data: &mut AppData) -> InputOutcome {
    let last = Settings::ROWS.len() - 1;
    state.help_message = None;
    match (key.code, state.keymap.action(&key)) {
        (KeyCode::Esc, _) | (_, Some(Action::Help)) => state.help_visible = false,
        (KeyCode::Enter | KeyCode::Char(' '), _) => {
            let (label, field) = Settings::ROWS[state.help_settings_selected.min(last)];
            let value = {
                let flag = field(&mut state.settings);
                *flag = !*flag;
                *flag
            };
            apply_settings(state, data);
            let on = if value { "on" } else { "off" };
            state.help_message = Some(saved(state, format!("{label}: {on}")));
        }
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

/// The equaliser page: ←→ (or h/l) picks a band, ↑↓ (or k/j) moves its fader,
/// r flattens it. The axes follow the sliders, which are vertical. Each change
/// goes straight to the audio thread, so it is audible on the track already
/// playing, and is saved like every other page of the popup.
fn handle_eq_input(key: KeyEvent, state: &mut AppState) -> InputOutcome {
    let last = eq::BANDS.len() - 1;
    let band = state.help_eq_selected.min(last);
    state.help_message = None;
    let delta = match (key.code, state.keymap.action(&key)) {
        (KeyCode::Esc, _) | (_, Some(Action::Help)) => {
            state.help_visible = false;
            return InputOutcome::Continue;
        }
        (KeyCode::Up, _) | (_, Some(Action::Up)) => 1,
        (KeyCode::Down, _) | (_, Some(Action::Down)) => -1,
        (KeyCode::Char('r'), _) => -state.eq.gains[band],
        (_, Some(Action::SubTabLeft)) | (KeyCode::Left, _) => {
            state.help_eq_selected = band.saturating_sub(1);
            return InputOutcome::Continue;
        }
        (_, Some(Action::SubTabRight)) | (KeyCode::Right, _) => {
            state.help_eq_selected = (band + 1).min(last);
            return InputOutcome::Continue;
        }
        (KeyCode::Home, _) => {
            state.help_eq_selected = 0;
            return InputOutcome::Continue;
        }
        (KeyCode::End, _) => {
            state.help_eq_selected = last;
            return InputOutcome::Continue;
        }
        _ => return InputOutcome::Continue,
    };
    let Some(value) = nudge(&mut state.eq.gains, band, delta) else {
        return InputOutcome::Continue;
    };
    eq::set(state.eq.gains);
    let label = eq::band_label(eq::BANDS[band]);
    state.help_message = Some(saved(state, format!("{label}: {}", eq::gain_label(value))));
    InputOutcome::Continue
}

/// Move one band, clamped to the equaliser's range. `None` when it was already
/// at the rail, so holding the key does not keep rewriting the config file.
fn nudge(gains: &mut [i8; eq::BANDS.len()], band: usize, delta: i8) -> Option<i8> {
    let current = gains[band];
    let next = current.saturating_add(delta).clamp(-eq::MAX_GAIN_DB, eq::MAX_GAIN_DB);
    (next != current).then(|| {
        gains[band] = next;
        next
    })
}

/// Make a just-flipped setting take effect on what is already loaded.
fn apply_settings(state: &mut AppState, data: &mut AppData) {
    apply_unplayable_filter(state, data);
    if state.selected_tab >= visible_tabs(state).len() {
        state.selected_tab = 0;
        state.selected_row = 0;
    }
}

/// Persist and decorate the message with the outcome.
fn saved(state: &AppState, msg: String) -> String {
    match crate::config::save(
        &state.keymap,
        &state.theme_name,
        &state.theme_overrides,
        &state.settings,
        &state.eq,
    ) {
        Ok(()) => format!("{msg}  · saved"),
        Err(e) => format!("{msg}  · NOT saved: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_the_three_pages() {
        let page = HelpPage::default();
        assert_eq!(page, HelpPage::Keys);
        assert_eq!(page.next(), HelpPage::Settings);
        assert_eq!(page.next().next(), HelpPage::Equalizer);
        assert_eq!(page.next().next().next(), HelpPage::Keys);
    }

    /// Both axes. Every band is pinned to a rail so no keypress here reaches
    /// `config::save` or `eq::set`, while the gain keys still prove they are not
    /// wired to the selection.
    #[test]
    fn the_axes_follow_the_vertical_faders() {
        let press = |code, state: &mut AppState| {
            handle_eq_input(KeyEvent::from(code), state);
        };
        let mut state = AppState {
            help_page: HelpPage::Equalizer,
            help_eq_selected: 2,
            ..Default::default()
        };

        state.eq.gains = [eq::MAX_GAIN_DB; eq::BANDS.len()];
        press(KeyCode::Right, &mut state);
        assert_eq!(state.help_eq_selected, 3, "→ moves to the next band");
        press(KeyCode::Left, &mut state);
        assert_eq!(state.help_eq_selected, 2, "← moves to the previous band");
        press(KeyCode::Up, &mut state);
        assert_eq!(state.help_eq_selected, 2, "↑ adjusts the gain, it does not move");
        assert_eq!(state.eq.gains[2], eq::MAX_GAIN_DB, "and stops at the top rail");

        state.eq.gains = [-eq::MAX_GAIN_DB; eq::BANDS.len()];
        press(KeyCode::Down, &mut state);
        assert_eq!(state.help_eq_selected, 2, "↓ adjusts the gain, it does not move");
        assert_eq!(state.eq.gains[2], -eq::MAX_GAIN_DB, "and stops at the bottom rail");
    }

    #[test]
    fn a_band_stops_at_the_rails() {
        let mut gains = [0i8; eq::BANDS.len()];
        assert_eq!(nudge(&mut gains, 2, 1), Some(1));
        assert_eq!(gains, [0, 0, 1, 0, 0, 0], "only the selected band moves");

        gains[2] = eq::MAX_GAIN_DB;
        assert_eq!(nudge(&mut gains, 2, 1), None, "already at the top rail");
        assert_eq!(gains[2], eq::MAX_GAIN_DB);

        gains[2] = -eq::MAX_GAIN_DB;
        assert_eq!(nudge(&mut gains, 2, -1), None, "already at the bottom rail");

        // What `r` sends: whatever it takes to get back to flat.
        gains[2] = -7;
        assert_eq!(nudge(&mut gains, 2, 7), Some(0));
        assert_eq!(nudge(&mut gains, 2, 0), None, "flattening a flat band is a no-op");
    }
}
