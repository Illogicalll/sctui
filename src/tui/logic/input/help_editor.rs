use ratatui::crossterm::event::{KeyCode, KeyEvent};

use std::time::Instant;

use super::InputOutcome;
use super::helpers::{clear_pending, list_step, lookup};
use crate::config::{SettingRow, Settings};
use crate::keymap::{Action, Chord, SEQ_TIMEOUT, Seq};
use crate::player::{Player, eq};
use crate::tui::logic::filtering::apply_unplayable_filter;
use crate::tui::logic::nav;
use crate::tui::logic::state::{AppData, AppState, HelpPage, visible_tabs};

/// The `?` popup: a modal key editor, with a settings page and an equaliser page
/// behind Tab. Owns every key while open.
pub(crate) fn handle_help_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    // Waiting for the key to bind. Tab is a bindable key, so this comes first.
    if capture_key(key, state) {
        return InputOutcome::Continue;
    }
    if key.code == KeyCode::Tab {
        state.help_page = state.help_page.next();
        state.help_message = None;
        return InputOutcome::Continue;
    }
    match state.help_page {
        HelpPage::Settings => handle_settings_input(key, state, data, player),
        HelpPage::Equalizer => handle_eq_input(key, state),
        HelpPage::Keys => handle_keys_input(key, state),
    }
}

/// While the editor is capturing a binding, take this key for it; returns whether it
/// did. The keys taken so far wait in `pending_keys`: each further key within a second
/// makes the binding a longer sequence, and the pause (or the third key) binds it.
fn capture_key(key: KeyEvent, state: &mut AppState) -> bool {
    if state.help_capture.is_none() {
        return false;
    }
    if key.code == KeyCode::Esc {
        state.help_capture = None;
        clear_pending(state);
        state.help_message = Some("Cancelled".to_string());
        return true;
    }
    if state.pending_since.is_some_and(|since| since.elapsed() >= SEQ_TIMEOUT) {
        // The loop has not got to it yet: bind what was typed; this key is an
        // ordinary one.
        commit_capture(state);
        return false;
    }
    state.pending_keys.push(Chord::from_key(&key));
    state.pending_since = Some(Instant::now());
    if state.pending_keys.len() >= Seq::MAX {
        commit_capture(state);
    } else {
        state.help_message = Some(format!(
            "{} …  (another key within a second makes it a sequence)",
            Seq(state.pending_keys.clone()).label()
        ));
    }
    true
}

/// The keys page: Enter starts a capture, Backspace removes the oldest key, r restores
/// the defaults, and the keymap's own movement keys move the highlight.
fn handle_keys_input(key: KeyEvent, state: &mut AppState) -> InputOutcome {
    let last = Action::ALL.len() - 1;
    let selected = Action::ALL[state.help_selected.min(last)];
    state.help_message = None;
    let action = lookup(state, &key);
    let step = list_step(&key, action, state.help_selected, last);
    match (key.code, action) {
        (KeyCode::Esc, _) | (_, Some(Action::Help)) => state.help_visible = false,
        (KeyCode::Enter, _) => {
            clear_pending(state);
            state.help_capture = Some(selected);
            state.help_message = Some(format!("Press a key for \"{}\"  (Esc cancels)", selected.description()));
        }
        (KeyCode::Backspace | KeyCode::Delete, _) => {
            state.help_message = Some(match state.keymap.unbind_one(selected) {
                Ok(seq) => saved(state, format!("Removed {} from {}", seq.label(), selected.description())),
                Err(refusal) => refusal,
            });
        }
        _ if step.is_some() => state.help_selected = step.unwrap_or(state.help_selected),
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

/// Bind the keys the editor has captured to the action it was capturing for.
pub(crate) fn commit_capture(state: &mut AppState) {
    let Some(action) = state.help_capture.take() else { return };
    let seq = Seq(std::mem::take(&mut state.pending_keys));
    state.pending_since = None;
    if seq.0.is_empty() {
        return;
    }
    let label = seq.label();
    state.help_message = Some(match state.keymap.add(action, seq) {
        Ok(()) => saved(state, format!("{label} → {}", action.description())),
        Err(other) => format!("{label} clashes with a key of {}", other.description()),
    });
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
    let action = lookup(state, &key);
    match (key.code, action) {
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
            } else if let SettingRow::Choice(..) = row {
                adjust(row, state, data, player, 1);
            }
        }
        (KeyCode::Left, _) | (_, Some(Action::SubTabLeft)) => adjust(row, state, data, player, -1),
        (KeyCode::Right, _) | (_, Some(Action::SubTabRight)) => adjust(row, state, data, player, 1),
        _ => {
            state.help_settings_selected =
                list_step(&key, action, state.help_settings_selected, last).unwrap_or(state.help_settings_selected);
        }
    }
    InputOutcome::Continue
}

/// Move a duration row by `steps` half-seconds, within its allowed range. The current
/// value is snapped onto the grid first, so a duration hand-edited to something off it
/// lands on a round figure rather than carrying the offset for ever.
fn adjust(row: SettingRow, state: &mut AppState, data: &mut AppData, player: &Player, steps: i8) {
    if let Some(msg) = cycle(row, &mut state.settings, steps) {
        state.help_message = Some(saved(state, msg));
        return;
    }
    let SettingRow::Secs(label, field) = row else { return };
    let snapped = state.settings.crossfade_secs_snapped();
    let secs = {
        let value = field(&mut state.settings);
        *value = (snapped + f32::from(steps) * Settings::CROSSFADE_SECS_STEP)
            .clamp(Settings::CROSSFADE_SECS_MIN, Settings::CROSSFADE_SECS_MAX);
        *value
    };
    apply_settings(state, data, player);
    state.help_message = Some(saved(state, format!("{label}: {secs:.1}s")));
}

/// Move a choice row one option back (`steps` < 0) or forward, wrapping; the message
/// names what it is now. `None` for any other kind of row.
fn cycle(row: SettingRow, settings: &mut Settings, steps: i8) -> Option<String> {
    let SettingRow::Choice(label, field) = row else { return None };
    let layout = field(settings);
    *layout = if steps < 0 { layout.prev() } else { layout.next() };
    Some(format!("{label}: {}", layout.label()))
}

/// The equaliser page: ←→ (or h/l) picks a band, ↑↓ (or k/j) moves its fader,
/// r flattens it. The axes follow the sliders, which are vertical. Each change
/// goes straight to the audio thread, so it is audible on the track already
/// playing, and is saved like every other page of the popup.
fn handle_eq_input(key: KeyEvent, state: &mut AppState) -> InputOutcome {
    let last = eq::BANDS.len() - 1;
    let band = state.help_eq_selected.min(last);
    state.help_message = None;
    let delta = match (key.code, lookup(state, &key)) {
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
        (_, Some(Action::Top)) => {
            state.help_eq_selected = 0;
            return InputOutcome::Continue;
        }
        (_, Some(Action::Bottom)) => {
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

/// Make a just-changed setting take effect on what is already loaded.
fn apply_settings(state: &mut AppState, data: &mut AppData, player: &Player) {    apply_unplayable_filter(state, data);
    if state.selected_tab >= visible_tabs(state).len() {
        nav::set_location(state, 0, None);
    }
    player.set_crossfade(state.settings.crossfade_ms(), state.settings.crossfade_user_skips);
}

/// Persist and decorate the message with the outcome.
fn saved(state: &AppState, msg: String) -> String {
    match crate::config::save(
        state.config_load_failed,
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

    /// A state whose saves are refused, so no test here writes the user's config.
    fn editor(action: Action) -> AppState {
        let mut state = AppState { help_visible: true, config_load_failed: true, ..Default::default() };
        state.help_selected = Action::ALL.iter().position(|a| *a == action).unwrap();
        state
    }

    fn press(state: &mut AppState, code: KeyCode) {
        let key = KeyEvent::from(code);
        if !capture_key(key, state) {
            handle_keys_input(key, state);
        }
    }

    #[test]
    fn capture_takes_one_key_or_a_sequence_typed_within_a_second() {
        // One key, then the pause: the loop binds it.
        let mut state = editor(Action::PlayPause);
        press(&mut state, KeyCode::Enter);
        assert_eq!(state.help_capture, Some(Action::PlayPause));
        press(&mut state, KeyCode::Char('z'));
        assert_eq!(state.keymap.seqs(Action::PlayPause).len(), 1, "not bound until the pause");
        assert!(state.help_message.as_deref().unwrap().starts_with("z …"));
        super::super::expire_pending(&mut state);
        assert!(state.help_capture.is_some(), "the second has not passed");
        state.pending_since = Instant::now().checked_sub(SEQ_TIMEOUT);
        super::super::expire_pending(&mut state);
        assert_eq!(state.keymap.labels(Action::PlayPause), "Space, z");
        assert!(state.help_capture.is_none() && state.pending_keys.is_empty() && state.pending_since.is_none());
        assert!(state.help_message.as_deref().unwrap().starts_with("z → Play / pause"));

        // Two keys in time make a sequence; j and Tab are captured, not obeyed.
        press(&mut state, KeyCode::Enter);
        press(&mut state, KeyCode::Char('z'));
        press(&mut state, KeyCode::Char('j'));
        assert_eq!((state.help_selected, state.help_page), (editor(Action::PlayPause).help_selected, HelpPage::Keys));
        state.pending_since = Instant::now().checked_sub(SEQ_TIMEOUT);
        // The loop has not run: the next key binds what was typed and then acts normally.
        press(&mut state, KeyCode::Char('j'));
        assert!(state.help_capture.is_none());
        assert_eq!(state.keymap.labels(Action::PlayPause), "Space, z", "z j starts with z, which is taken");
        assert_eq!(state.help_selected, editor(Action::PlayPause).help_selected + 1);

        // The third key ends the capture at once.
        let mut state = editor(Action::PlayPause);
        press(&mut state, KeyCode::Enter);
        for c in ['z', 'z', 't'] {
            press(&mut state, KeyCode::Char(c));
        }
        assert!(state.help_capture.is_none() && state.pending_keys.is_empty());
        assert_eq!(state.keymap.labels(Action::PlayPause), "Space, z z t");
        // Sharing a start with another sequence is fine; being the start of one is not.
        press(&mut state, KeyCode::Enter);
        press(&mut state, KeyCode::Char('g'));
        state.pending_since = Instant::now().checked_sub(SEQ_TIMEOUT);
        super::super::expire_pending(&mut state);
        assert!(state.help_message.as_deref().unwrap().contains("clashes with a key of Go to the first row"));
        press(&mut state, KeyCode::Enter);
        press(&mut state, KeyCode::Char('g'));
        press(&mut state, KeyCode::Char('p'));
        state.pending_since = Instant::now().checked_sub(SEQ_TIMEOUT);
        super::super::expire_pending(&mut state);
        assert_eq!(state.keymap.labels(Action::PlayPause), "Space, z z t, g p");

        // Esc cancels, keeping nothing.
        press(&mut state, KeyCode::Enter);
        press(&mut state, KeyCode::Char('y'));
        press(&mut state, KeyCode::Esc);
        assert!(state.help_capture.is_none() && state.pending_keys.is_empty() && state.help_visible);
        assert_eq!(state.keymap.seqs(Action::PlayPause).len(), 3);
    }

    #[test]
    fn the_last_key_of_help_or_quit_cannot_be_removed() {
        for action in [Action::Help, Action::Quit] {
            let mut state = editor(action);
            press(&mut state, KeyCode::Backspace);
            assert_eq!(state.keymap.seqs(action).len(), 1);
            assert!(state.help_message.as_deref().unwrap().contains("must keep a key"));
        }
        // Any other action loses its oldest key, and says which.
        let mut state = editor(Action::Top);
        press(&mut state, KeyCode::Backspace);
        assert_eq!(state.keymap.labels(Action::Top), "Home");
        assert!(state.help_message.as_deref().unwrap().starts_with("Removed g g from"));
        // The list moves with the keymap, sequences included.
        press(&mut state, KeyCode::Char('j'));
        press(&mut state, KeyCode::End);
        assert_eq!(state.help_selected, Action::ALL.len() - 1);
        press(&mut state, KeyCode::Home);
        assert_eq!(state.help_selected, 0);
    }

    #[test]
    fn settings_row_cycles_layout() {
        use crate::config::Layout;
        let row = *Settings::ROWS.iter().find(|r| r.label() == "Layout").expect("a Layout row");
        let mut settings = Settings::default();
        assert_eq!(row.display(settings), ("‹ classic ›".to_string(), true));
        // Right (and Enter / Space) goes forward through all four and wraps; Left goes back.
        let seen: Vec<Layout> = (0..4).map(|_| { cycle(row, &mut settings, 1); settings.layout }).collect();
        assert_eq!(seen, [Layout::Tighten, Layout::Columns, Layout::PlayerPanel, Layout::Classic]);
        assert_eq!(cycle(row, &mut settings, -1).as_deref(), Some("Layout: player-panel"));
        assert_eq!(settings.layout, Layout::PlayerPanel);
        assert_eq!(row.display(settings).0, "‹ player-panel ›");
        // Only a choice row cycles; the others keep their own keys.
        for other in Settings::ROWS.iter().filter(|r| !matches!(r, SettingRow::Choice(..))) {
            assert_eq!(cycle(*other, &mut settings, 1), None);
        }
        assert!(!Settings::ROWS.iter().any(|r| r.label().to_lowercase().contains("onboard")));
    }

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
