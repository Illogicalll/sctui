use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use std::time::Instant;

use crate::keymap::Action;
use crate::player::Player;

use crate::tui::logic::state::{AppData, AppState, Lane};

pub(crate) mod helpers;
mod quit;
mod search;
mod navigation;
mod movement;
mod playback;
mod history;
mod help_editor;
mod theme_picker;
mod confirm;
mod playlist_picker;
mod queue;
mod commands;

pub enum InputOutcome {
    Continue,
    Quit,
}

/// Entry points for OS media commands (media keys, headphone buttons, Control
/// Centre), which arrive outside the key-event path.
pub(crate) fn toggle_play_pause(player: &Player) {
    if player.is_playing() {
        player.pause();
    } else {
        player.resume();
    }
}

pub(crate) fn next_track(state: &mut AppState, data: &mut AppData, player: &Player) {
    navigation::handle_next_track(state, data, player);
}

pub(crate) fn prev_track(state: &mut AppState, data: &mut AppData, player: &Player) {
    navigation::handle_prev_track(state, data, player);
}

/// Popups and text fields take keys first; everything else goes through the
/// keymap to an [`Action`].
pub fn handle_key_event(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if key.kind != KeyEventKind::Press {
        // Windows emits both Press and Release events per keystroke; Unix
        // ttys only ever emit Press, so this is a no-op there.
        return InputOutcome::Continue;
    }
    if state.quit_confirm_visible {
        return quit::handle_quit_confirm(key, state);
    }
    if state.help_visible {
        return help_editor::handle_help_input(key, state);
    }
    if state.theme_picker_visible {
        return theme_picker::handle_theme_picker_input(key, state);
    }
    if state.confirm.is_some() {
        return confirm::handle_confirm_input(key, state, data);
    }
    if state.playlist_picker_visible
        && let Some(outcome) = playlist_picker::handle_picker_input(key, state, data)
    {
        return outcome;
    }
    if state.history_visible
        && let Some(outcome) = history::handle_history_input(key, state, data, player)
    {
        return outcome;
    }
    if state.search_popup_visible
        && let Some(outcome) = search::handle_search_input(key, state, data)
    {
        return outcome;
    }

    // Search tab: while typing, plain printable keys go to the query. A chord with a
    // modifier that is bound to an action runs the action instead (so Shift+A queues
    // the highlighted result mid-search; Caps Lock letters still type on terminals
    // that report them without Shift). Enter/Esc leave typing; any other key leaves
    // and is then handled normally.
    if state.selected_tab == 1 && state.search_typing {
        let modified = key.modifiers.intersects(KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                state.search_typing = false;
                return InputOutcome::Continue;
            }
            KeyCode::Backspace => return commands::handle_backspace(state),
            KeyCode::Char(_) if modified && state.keymap.action(&key).is_some() => {}
            KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                return commands::handle_search_char(c, state);
            }
            KeyCode::Char(_) => {}
            _ => state.search_typing = false,
        }
    }

    // Esc closes whatever is open before it means "quit".
    if key.code == KeyCode::Esc && state.queue_visible {
        state.queue_visible = false;
        return InputOutcome::Continue;
    }

    if state.visualizer_mode && key.code == KeyCode::Tab {
        state.visualizer_view = state.visualizer_view.next();
        return InputOutcome::Continue;
    }

    match state.keymap.action(&key) {
        Some(action) => run_action(action, state, data, player),
        None => InputOutcome::Continue,
    }
}

/// Views with two selectable panes: Library playlists/albums/following, Search
/// albums/playlists/people, and the feed (activities + their tracks).
fn has_second_pane(state: &AppState) -> bool {
    match state.selected_tab {
        0 => state.selected_subtab != 0,
        1 => state.selected_searchfilter != 0,
        2 => true,
        _ => false,
    }
}

/// Dispatch one action. Plain Up/Down go through the held-key ramp; the page and
/// secondary variants still call the movement handlers with a `KeyEvent` whose
/// modifiers select the variant (Alt = page, Shift = second pane).
pub(crate) fn run_action(
    action: Action,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    let key = |code: KeyCode, mods: KeyModifiers| KeyEvent::new(code, mods);
    let seek_ok = |state: &AppState| {
        // Only seeks wait for an in-flight seek; every other key stays live.
        (player.is_playing() || state.current_playing_index.is_some()) && !player.is_seeking()
    };
    match action {
        Action::Quit => {
            state.quit_confirm_visible = true;
            state.quit_confirm_selected = 1;
            InputOutcome::Continue
        }
        Action::NextTab => navigation::handle_tab_switch(state),
        Action::PrevTab => navigation::handle_tab_switch_back(state),
        Action::SubTabLeft => navigation::sub_tab_left(state, data),
        Action::SubTabRight => navigation::sub_tab_right(state, data),
        Action::Up => movement::handle_step_key(Lane::Primary, -1, KeyModifiers::NONE, state, data),
        Action::Down => movement::handle_step_key(Lane::Primary, 1, KeyModifiers::NONE, state, data),
        Action::PageUp => movement::handle_up_key(key(KeyCode::Up, KeyModifiers::ALT), state, data),
        Action::PageDown => movement::handle_down_key(key(KeyCode::Down, KeyModifiers::ALT), state, data),
        // Second pane where there is one, and then it ramps like Up/Down; with no second
        // pane it is a ten-row page jump, which is already a big step and is left alone.
        Action::SecondaryUp if has_second_pane(state) => {
            movement::handle_step_key(Lane::Secondary, -1, KeyModifiers::SHIFT, state, data)
        }
        Action::SecondaryDown if has_second_pane(state) => {
            movement::handle_step_key(Lane::Secondary, 1, KeyModifiers::SHIFT, state, data)
        }
        Action::SecondaryUp => movement::handle_up_key(key(KeyCode::Up, KeyModifiers::ALT), state, data),
        Action::SecondaryDown => {
            movement::handle_down_key(key(KeyCode::Down, KeyModifiers::ALT), state, data)
        }
        // The third pane's cursor lives in `run_command`, so the ramp repeats the whole
        // action rather than a movement handler.
        Action::TertiaryUp | Action::TertiaryDown => {
            let dir = if action == Action::TertiaryUp { -1 } else { 1 };
            let mut outcome = InputOutcome::Continue;
            for _ in 0..state.move_accel.step_rows(Lane::Tertiary, dir, Instant::now()) {
                outcome = commands::run_command(action, state, data, player);
                if !matches!(outcome, InputOutcome::Continue) {
                    break;
                }
            }
            outcome
        }
        Action::PlayPause => {
            toggle_play_pause(player);
            InputOutcome::Continue
        }
        Action::PlaySelected => playback::handle_enter(state, data, player),
        Action::StartStation => playback::handle_station(state, data, player),
        Action::NextTrack => navigation::handle_next_track(state, data, player),
        Action::PrevTrack => navigation::handle_prev_track(state, data, player),
        Action::SeekForward => {
            if seek_ok(state) {
                player.fast_forward();
            }
            InputOutcome::Continue
        }
        Action::SeekBackward => {
            if seek_ok(state) {
                player.rewind();
            }
            InputOutcome::Continue
        }
        other => commands::run_command(other, state, data, player),
    }
}
