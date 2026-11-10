use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use super::helpers::{list_step, lookup, toggles_binary_choice};
use super::theme_picker;
use crate::config::Layout;
use crate::keymap::Action;
use crate::theme::{self, Theme};
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::wizard::{self, Stage};

/// The wizard's modal screens (welcome, layout, theme, done) and the upgrade prompt.
/// They own every key; the practice stage is not routed here at all.
pub(crate) fn handle_wizard_input(key: KeyEvent, state: &mut AppState, data: &AppData) -> InputOutcome {
    if let Some(theme) = step(key, state, data) {
        theme::set(theme);
    }
    InputOutcome::Continue
}

/// The state change for one key, and the theme to show as a result (see
/// `theme_picker::step`).
fn step(key: KeyEvent, state: &mut AppState, data: &AppData) -> Option<Theme> {
    let action = lookup(state, &key);
    if state.tour_prompt {
        match key.code {
            KeyCode::Enter if state.confirm_selected == 0 => {
                state.tour_prompt = false;
                wizard::start_practice(state, data);
            }
            KeyCode::Enter | KeyCode::Esc => {
                let help = state.keymap.hint_label(Action::Help);
                wizard::finish(state, format!("{help} lists the keys · the tour is in the palette: Setup wizard"));
            }
            _ if toggles_binary_choice(&key, action) => state.confirm_selected ^= 1,
            _ => {}
        }
        return None;
    }
    let stage = state.wizard.as_ref()?.stage;
    // The theme stage is the theme picker: it previews, keeps on Enter, reverts on Esc.
    // (Its own key would close it; here that key does nothing.)
    let shown = match stage {
        Stage::Theme => theme_picker::step(key, action.filter(|a| *a != Action::Themes), state),
        _ => None,
    };
    match (key.code, stage) {
        (KeyCode::Esc, _) => wizard::skip(state),
        (KeyCode::Enter, Stage::Welcome) => {
            let at = Layout::ALL.iter().position(|l| *l == state.settings.layout).unwrap_or(0);
            let wizard = state.wizard.as_mut()?;
            (wizard.stage, wizard.selected) = (Stage::Layout, at);
        }
        (KeyCode::Enter, Stage::Layout) => {
            let wizard = state.wizard.as_mut()?;
            (wizard.stage, wizard.revert_layout) = (Stage::Theme, None);
            theme_picker::open_theme_picker(state);
            // The wizard draws the list itself, centred over the preview.
            state.theme_picker_visible = false;
        }
        (_, Stage::Layout) => {
            let wizard = state.wizard.as_mut()?;
            if let Some(next) = list_step(&key, action, wizard.selected, Layout::ALL.len() - 1) {
                wizard.selected = next;
                // The real layout is drawn behind the list: this is the preview.
                state.settings.layout = Layout::ALL[next];
            }
        }
        (KeyCode::Enter, Stage::Theme) => wizard::start_practice(state, data),
        (KeyCode::Enter, Stage::Done) => {
            let help = state.keymap.hint_label(Action::Help);
            wizard::finish(state, format!("Setup done · {help} for keys"));
        }
        _ => {}
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::super::{Routed, commands, movement, navigation, route};
    use super::*;
    use crate::tui::logic::nav;
    use crate::tui::logic::wizard::{LESSONS, Wizard};
    use crate::tui::render::testkit::{fixture, fixture_empty};
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// What `handle_key_event` does with a key, less the player: route it, run the
    /// action if it is one that needs no player, and let the wizard observe.
    fn press(state: &mut AppState, data: &mut AppData, code: KeyCode) {
        let mods = match code {
            KeyCode::Char(c) if c.is_uppercase() => KeyModifiers::SHIFT,
            _ => KeyModifiers::NONE,
        };
        let watched = wizard::watch(state, data);
        let action = match route(KeyEvent::new(code, mods), state, data) {
            Routed::Action(action) => Some(action),
            _ => None,
        };
        match action {
            Some(a) if nav::in_nav(state) && commands::run_nav_command(a, state, data) => {}
            Some(Action::PlaySelected) if nav::focused(state).is_container() => {
                nav::focus_right(state);
            }
            Some(Action::PlaySelected | Action::PlayPause) | None => {}
            Some(Action::PaneLeft) => nav::pane_left(state),
            Some(Action::PaneRight) => {
                nav::focus_right(state);
            }
            Some(Action::SubTabRight) => {
                navigation::sub_tab(state, 1);
            }
            Some(a @ (Action::Up | Action::Down | Action::Top | Action::Bottom)) => {
                movement::handle_move(a, state, data);
            }
            Some(other) => {
                commands::run_command(other, state, data);
            }
        }
        wizard::observe(state, data, action, watched);
    }

    fn type_keys(state: &mut AppState, data: &mut AppData, keys: &str) {
        for c in keys.chars() {
            press(state, data, KeyCode::Char(c));
        }
    }

    fn at(state: &AppState) -> (Stage, usize) {
        let wizard = state.wizard.as_ref().expect("the wizard is running");
        (wizard.stage, wizard.step)
    }

    #[test]
    fn layout_stage_previews_live_and_enter_keeps() {
        let (mut state, mut data, ..) = fixture();
        wizard::on_start(&mut state, true, false);
        // Modal: nothing reaches the app behind the welcome screen.
        type_keys(&mut state, &mut data, "jl:Q?");
        assert_eq!((at(&state), nav::location(&state), nav::cursor(&state, nav::PaneId::Likes)), ((Stage::Welcome, 0), (0, 0), 0));
        assert!(state.palette.is_none() && !state.queue_visible && !state.help_visible);
        press(&mut state, &mut data, KeyCode::Enter);
        // The list starts on the layout in use (Tighten on a fresh install).
        assert_eq!(at(&state).0, Stage::Layout);
        assert_eq!(state.wizard.as_ref().unwrap().selected, 1);
        // Moving, by arrow or by the keymap, switches the real layout as it goes.
        press(&mut state, &mut data, KeyCode::Down);
        assert_eq!(state.settings.layout, Layout::Sidebar);
        type_keys(&mut state, &mut data, "j");
        assert_eq!(state.settings.layout, Layout::Columns);
        type_keys(&mut state, &mut data, "G");
        assert_eq!(state.settings.layout, Layout::PlayerPanel);
        type_keys(&mut state, &mut data, "gg");
        assert_eq!(state.settings.layout, Layout::Classic);
        type_keys(&mut state, &mut data, "jj");
        // Enter keeps it: skipping later no longer puts the old one back.
        press(&mut state, &mut data, KeyCode::Enter);
        assert_eq!((at(&state).0, state.settings.layout), (Stage::Theme, Layout::Sidebar));
        assert!(!state.theme_picker_visible, "the wizard draws the theme list");
        // Esc there reverts the theme, not the layout. Through `step`: the theme is not applied.
        step(key(KeyCode::Esc), &mut state, &data);
        assert!(state.wizard.is_none() && state.settings.onboarded);
        assert_eq!(state.settings.layout, Layout::Sidebar);

        // Esc while only previewing puts the layout back.
        let (mut state, mut data, ..) = fixture();
        (state.settings.onboarded, state.settings.layout) = (true, Layout::Columns);
        wizard::on_start(&mut state, false, true);
        press(&mut state, &mut data, KeyCode::Enter);
        assert_eq!(state.wizard.as_ref().unwrap().selected, 3);
        press(&mut state, &mut data, KeyCode::Up);
        assert_eq!(state.settings.layout, Layout::Sidebar);
        press(&mut state, &mut data, KeyCode::Esc);
        assert_eq!((state.wizard.is_none(), state.settings.layout), (true, Layout::Columns));
    }

    #[test]
    fn theme_stage_reverts_on_esc() {
        let (mut state, data, ..) = fixture();
        state.wizard = Some(Wizard::at(Stage::Theme));
        state.theme_name = Theme::BUILTIN[0].name.to_string();
        // As the layout stage leaves it, but with a known theme to go back to.
        let before = Theme::BUILTIN[3];
        state.theme_picker_previous = Some(before);
        // Moving hands back the theme to preview; the kept name does not change yet.
        assert_eq!(step(key(KeyCode::Down), &mut state, &data), Some(Theme::BUILTIN[1]));
        assert_eq!(step(key(KeyCode::Char('j')), &mut state, &data), Some(Theme::BUILTIN[2]));
        assert_eq!(state.theme_name, Theme::BUILTIN[0].name);
        // The picker's own key does not close the stage.
        assert_eq!(step(key(KeyCode::Char('t')), &mut state, &data), None);
        assert_eq!(at(&state).0, Stage::Theme);
        // Esc: the theme from before comes back, nothing is kept, the wizard is over.
        assert_eq!(step(key(KeyCode::Esc), &mut state, &data), Some(before));
        assert_eq!(state.theme_name, Theme::BUILTIN[0].name);
        assert!(state.wizard.is_none() && state.settings.onboarded && state.theme_picker_previous.is_none());

        // Enter keeps the highlighted theme and starts the practice.
        let (mut state, data, ..) = fixture();
        state.wizard = Some(Wizard::at(Stage::Theme));
        state.theme_picker_previous = Some(before);
        step(key(KeyCode::Down), &mut state, &data);
        assert_eq!(step(key(KeyCode::Enter), &mut state, &data), None);
        assert_eq!(state.theme_name, Theme::BUILTIN[1].name);
        assert_eq!(at(&state), (Stage::Practice, 0));
        assert!(!state.settings.onboarded, "not until the tour ends");
    }

    /// The whole tour, key by key, on a keymap where every lesson's action has moved.
    #[test]
    fn each_lesson_completes_on_its_action_even_when_rebound() {
        let (mut state, mut data, ..) = fixture();
        let warnings = state.keymap.apply_toml(
            "[keys]\ndown = \"n\"\nbottom = \"B\"\ntop = \"T\"\nsub_tab_right = \"w\"\npane_right = \"e\"\npane_left = \"b\"\n\
             play_selected = \"y\"\nplay_pause = \"u\"\nadd_to_queue = \"z\"\nsearch = \"i\"\npalette = \";\"\nhelp = \"K\"\n",
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        wizard::start_practice(&mut state, &data);

        // The old default keys do nothing for the lessons.
        type_keys(&mut state, &mut data, "jjjj");
        assert_eq!(at(&state), (Stage::Practice, 0));
        type_keys(&mut state, &mut data, "nnn");
        assert_eq!(at(&state).1, 1);
        assert!(nav::cursor(&state, nav::PaneId::Likes) >= 3, "the keys really moved the list");
        type_keys(&mut state, &mut data, "BT");
        assert_eq!(at(&state).1, 2);
        type_keys(&mut state, &mut data, "w");
        assert_eq!((at(&state).1, nav::location(&state)), (3, (0, 1)));
        type_keys(&mut state, &mut data, "eb");
        assert_eq!(at(&state).1, 4);
        // Play: on the playlists pane it steps in; on a track it counts.
        type_keys(&mut state, &mut data, "y");
        assert_eq!((at(&state).1, nav::focus(&state)), (4, 1));
        type_keys(&mut state, &mut data, "y");
        assert_eq!(at(&state).1, 5);
        type_keys(&mut state, &mut data, "uu");
        assert_eq!(at(&state).1, 6);
        type_keys(&mut state, &mut data, "z");
        assert_eq!((at(&state).1, state.manual_queue.len()), (7, 3));
        assert!(state.toasts.last().unwrap().msg.starts_with("Added to queue"));
        // Filter: typed text is not commands; Enter keeps it; Esc clears it, and only then
        // does a second Esc mean "skip".
        type_keys(&mut state, &mut data, "iListed");
        press(&mut state, &mut data, KeyCode::Enter);
        assert_eq!((at(&state).1, state.search_query.as_str()), (7, "Listed"));
        press(&mut state, &mut data, KeyCode::Esc);
        assert_eq!(at(&state).1, 8);
        // Palette: Esc out of it is not a command; a command run from it is.
        type_keys(&mut state, &mut data, ";");
        press(&mut state, &mut data, KeyCode::Esc);
        assert_eq!(at(&state).1, 8);
        type_keys(&mut state, &mut data, ";toggle repeat");
        press(&mut state, &mut data, KeyCode::Enter);
        assert_eq!((at(&state).1, state.repeat_enabled), (9, true));
        // Keys popup. `Routed::Help` needs the player, so close it as its handler would.
        type_keys(&mut state, &mut data, "K");
        assert!(state.help_visible && at(&state).1 == 9);
        let watched = wizard::watch(&state, &data);
        state.help_visible = false;
        wizard::observe(&mut state, &data, None, watched);
        assert_eq!(at(&state).0, Stage::Done);
        // Done is modal; Enter ends it and records it.
        type_keys(&mut state, &mut data, "nw");
        assert_eq!(nav::location(&state), (0, 1));
        press(&mut state, &mut data, KeyCode::Enter);
        assert!(state.wizard.is_none() && state.settings.onboarded);
        assert!(state.toasts.last().unwrap().msg.contains("K for keys"));
    }

    #[test]
    fn esc_skips_and_sets_onboarded() {
        for stage in [Stage::Welcome, Stage::Layout, Stage::Practice, Stage::Done] {
            let (mut state, mut data, ..) = fixture();
            state.wizard = Some(Wizard::at(stage));
            press(&mut state, &mut data, KeyCode::Esc);
            assert!(state.wizard.is_none() && state.settings.onboarded, "{stage:?}");
            assert!(!state.quit_confirm_visible, "{stage:?}");
        }
        // During practice Esc still does its own job first: it closes what is open.
        let (mut state, mut data, ..) = fixture();
        wizard::start_practice(&mut state, &data);
        type_keys(&mut state, &mut data, ":");
        press(&mut state, &mut data, KeyCode::Esc);
        type_keys(&mut state, &mut data, "/So");
        press(&mut state, &mut data, KeyCode::Esc);
        state.visualizer_mode = true;
        press(&mut state, &mut data, KeyCode::Esc);
        assert!(state.palette.is_none() && !state.search_typing && !state.visualizer_mode);
        assert!(wizard::practising(&state) && !state.settings.onboarded);
        // Every other key works as usual: the wizard blocks nothing.
        type_keys(&mut state, &mut data, "sQ");
        assert!(state.shuffle_enabled && state.queue_visible);

        // The upgrade prompt: Yes starts at the practice, No and Esc just record it.
        for (keys, practising) in [(&[KeyCode::Enter][..], true), (&[KeyCode::Right, KeyCode::Enter], false), (&[KeyCode::Esc], false)] {
            let (mut state, mut data, ..) = fixture();
            wizard::on_start(&mut state, false, false);
            type_keys(&mut state, &mut data, ":Q");
            assert!(state.tour_prompt && state.palette.is_none() && !state.queue_visible, "modal");
            for code in keys {
                press(&mut state, &mut data, *code);
            }
            assert!(!state.tour_prompt);
            assert_eq!((wizard::practising(&state), state.settings.onboarded), (practising, !practising), "{keys:?}");
            assert_eq!(state.settings.layout, Layout::Classic);
            if !practising {
                assert!(state.toasts.last().unwrap().msg.starts_with("? lists the keys"));
            }
        }
    }

    #[test]
    fn empty_library_skips_track_lessons() {
        let (mut state, mut data, ..) = fixture_empty();
        wizard::start_practice(&mut state, &data);
        // Every lesson's keys are no-ops on empty lists, and still count.
        type_keys(&mut state, &mut data, "jjjGggl][");
        // Play, pause and queue are passed over: straight to the filter.
        assert_eq!(at(&state), (Stage::Practice, 7));
        type_keys(&mut state, &mut data, "/x");
        press(&mut state, &mut data, KeyCode::Esc);
        type_keys(&mut state, &mut data, ":hint");
        press(&mut state, &mut data, KeyCode::Enter);
        assert_eq!(at(&state).1, 9);
        // Nothing to play at the very start either: a wizard begun on lesson 5 moves on.
        let mut wizard = Wizard { step: 4, ..Wizard::at(Stage::Practice) };
        wizard.observe(None, &Default::default(), &Default::default());
        assert_eq!(wizard.step, 7);
    }

    #[test]
    fn lesson_steps_never_enqueue_engagement_or_playlist_edit() {
        let (mut state, mut data, ..) = fixture();
        wizard::on_start(&mut state, true, false);
        // The whole wizard with the default keys: welcome, layout, theme (kept as is), tour.
        press(&mut state, &mut data, KeyCode::Enter);
        type_keys(&mut state, &mut data, "j");
        press(&mut state, &mut data, KeyCode::Enter);
        step(key(KeyCode::Enter), &mut state, &data);
        assert_eq!(at(&state), (Stage::Practice, 0));
        type_keys(&mut state, &mut data, "jjjGggl][");
        press(&mut state, &mut data, KeyCode::Enter);
        press(&mut state, &mut data, KeyCode::Enter);
        type_keys(&mut state, &mut data, "  a/Li");
        press(&mut state, &mut data, KeyCode::Enter);
        press(&mut state, &mut data, KeyCode::Esc);
        type_keys(&mut state, &mut data, ":toggle shuffle");
        press(&mut state, &mut data, KeyCode::Enter);
        type_keys(&mut state, &mut data, "?");
        assert_eq!(at(&state).1, LESSONS.len() - 1);
        assert!(state.engagement_queue.is_empty() && state.playlist_edit_queue.is_empty());
        assert!(state.confirm.is_none() && !state.playlist_picker_visible);
    }
}
