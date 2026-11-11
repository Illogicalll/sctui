use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::keymap::{Action, SEQ_TIMEOUT};
use crate::player::Player;

use crate::tui::logic::filtering::clear_filter;
use crate::tui::logic::nav;
use crate::tui::logic::palette::Command;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::wizard as tour;

pub(crate) mod helpers;
mod quit;
mod search;
mod navigation;
mod movement;
pub(crate) mod playback;
mod history;
mod help_editor;
mod theme_picker;
mod confirm;
mod playlist_picker;
mod palette;
mod queue;
mod wizard;
pub(crate) mod commands;

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

pub fn handle_key_event(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    let before = mode(state);
    // The wizard's lessons watch what each key did; they never take a key.
    let watched = tour::watch(state, data);
    let routed = route(key, state, data);
    let action = if let Routed::Action(action) = routed { Some(action) } else { None };
    let outcome = match routed {
        Routed::Done(outcome) => outcome,
        Routed::Help => help_editor::handle_help_input(key, state, data, player),
        Routed::History => history::handle_history_input(key, state, data, player),
        Routed::Action(action) => run_action(action, state, data, player),
    };
    tour::observe(state, data, action, watched);
    // A half-typed sequence never survives an overlay opening or closing, or a
    // switch into or out of insert mode.
    if mode(state) != before {
        helpers::clear_pending(state);
    }
    outcome
}

/// Which mode owns the keyboard: each overlay, and each place that takes typed text.
/// A new overlay adds its flag here.
fn mode(state: &AppState) -> [bool; 12] {
    [
        tour::modal(state),
        state.palette.is_some(),
        state.quit_confirm_visible,
        state.help_visible,
        state.help_capture.is_some(),
        state.theme_picker_visible,
        state.confirm.is_some(),
        state.playlist_picker_visible,
        state.playlist_picker_title.is_some(),
        state.history_visible,
        state.queue_visible,
        state.search_typing,
    ]
}

/// Once per loop: a half-typed sequence lapses after a second. In the key editor that
/// pause is what ends a capture, so there the keys are bound rather than dropped.
pub(crate) fn expire_pending(state: &mut AppState) {
    if state.pending_since.is_some_and(|since| since.elapsed() >= SEQ_TIMEOUT) {
        if state.help_capture.is_some() {
            help_editor::commit_capture(state);
        } else {
            helpers::clear_pending(state);
        }
    }
}

/// Where a key press goes. The two overlays that drive the player are named so that
/// everything else can be decided, and tested, without one.
enum Routed {
    Done(InputOutcome),
    Help,
    History,
    Action(Action),
}

/// The four modes, in order: an open overlay owns every key (nothing reaches the view
/// behind it); insert mode types every key; the visualiser drops actions that need the
/// lists it hides; otherwise the keymap decides. Esc only ever closes or cancels.
fn route(key: KeyEvent, state: &mut AppState, data: &mut AppData) -> Routed {
    let done = Routed::Done(InputOutcome::Continue);
    if key.kind != KeyEventKind::Press {
        // Windows emits both Press and Release events per keystroke; Unix
        // ttys only ever emit Press, so this is a no-op there.
        return done;
    }
    if tour::modal(state) {
        return Routed::Done(wizard::handle_wizard_input(key, state, data));
    }
    if state.palette.is_some() {
        // Insert mode: the keymap is never consulted, so nothing can be left pending.
        helpers::clear_pending(state);
        let command = palette::handle_palette_input(key, state);
        if command.is_some()
            && let Some(wizard) = &mut state.wizard
        {
            wizard.ran_command();
        }
        return match command {
            Some(Command::Action(action)) if state.visualizer_mode && needs_lists(action) => done,
            Some(Command::Action(action)) => Routed::Action(action),
            Some(command) => {
                palette::run(command, state);
                done
            }
            None => done,
        };
    }
    if state.quit_confirm_visible {
        return Routed::Done(quit::handle_quit_confirm(key, state));
    }
    if state.help_visible {
        return Routed::Help;
    }
    if state.theme_picker_visible {
        return Routed::Done(theme_picker::handle_theme_picker_input(key, state));
    }
    if state.confirm.is_some() {
        return Routed::Done(confirm::handle_confirm_input(key, state, data));
    }
    if state.playlist_picker_visible {
        return Routed::Done(playlist_picker::handle_picker_input(key, state, data));
    }
    if state.history_visible {
        return Routed::History;
    }
    if state.queue_visible {
        return Routed::Done(queue::handle_queue_input(key, state));
    }
    if state.search_typing {
        helpers::clear_pending(state);
        return Routed::Done(search::handle_insert(key, state, data));
    }

    if key.code == KeyCode::Esc {
        helpers::clear_pending(state);
        if state.visualizer_mode {
            state.visualizer_mode = false;
        } else if state.search_popup_visible {
            clear_filter(state);
        } else if tour::practising(state) {
            // Nothing else for Esc to close: it skips the rest of the tour.
            tour::skip(state);
        }
        return done;
    }
    if state.visualizer_mode && key.code == KeyCode::Tab {
        state.visualizer_view = state.visualizer_view.next();
        return done;
    }
    match helpers::lookup(state, &key) {
        Some(action) if state.visualizer_mode && needs_lists(action) => done,
        Some(action) => Routed::Action(action),
        None => done,
    }
}

/// Actions on the selection (and the filter prompt), which mean nothing while the
/// visualiser covers the lists.
fn needs_lists(action: Action) -> bool {
    matches!(
        action,
        Action::PlaySelected
            | Action::StartStation
            | Action::ToggleLike
            | Action::AddToQueue
            | Action::PlayNext
            | Action::AddToPlaylist
            | Action::RemoveFromPlaylist
            | Action::DeletePlaylist
            | Action::Search
    )
}

/// Dispatch one action in normal mode.
pub(crate) fn run_action(
    action: Action,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    let seek_ok = |state: &AppState| {
        // Only seeks wait for an in-flight seek; every other key stays live.
        (player.is_playing() || state.current_playing_index.is_some()) && !player.is_seeking()
    };
    if nav::in_nav(state) && commands::run_nav_command(action, state, data) {
        return InputOutcome::Continue;
    }
    match action {
        Action::Quit => {
            state.quit_confirm_visible = true;
            state.quit_confirm_selected = 1;
        }
        Action::NextTab => return navigation::handle_tab_switch(state),
        Action::PrevTab => return navigation::handle_tab_switch_back(state),
        Action::TabLibrary => return navigation::go_to_tab(state, 0),
        Action::TabSearch => return navigation::go_to_tab(state, 1),
        Action::TabFeed => return navigation::go_to_tab(state, 2),
        Action::SubTabLeft => return navigation::sub_tab(state, 3),
        Action::SubTabRight => return navigation::sub_tab(state, 1),
        Action::PaneLeft => nav::pane_left(state),
        Action::PaneRight => {
            nav::focus_right(state);
        }
        Action::Up
        | Action::Down
        | Action::PageUp
        | Action::PageDown
        | Action::HalfPageUp
        | Action::HalfPageDown
        | Action::Top
        | Action::Bottom => return movement::handle_move(action, state, data),
        Action::PlayPause => toggle_play_pause(player),
        // Enter steps into a container; only a tracks pane plays.
        Action::PlaySelected if nav::focused(state).is_container() => {
            nav::focus_right(state);
        }
        Action::PlaySelected => return playback::handle_enter(state, data, player),
        Action::StartStation => return playback::handle_station(state, data, player),
        Action::NextTrack => return navigation::handle_next_track(state, data, player),
        Action::PrevTrack => return navigation::handle_prev_track(state, data, player),
        Action::SeekForward => {
            if seek_ok(state) {
                player.fast_forward();
            }
        }
        Action::SeekBackward => {
            if seek_ok(state) {
                player.rewind();
            }
        }
        Action::VolumeUp => player.volume_up(),
        Action::VolumeDown => player.volume_down(),
        other => return commands::run_command(other, state, data),
    }
    InputOutcome::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Track;
    use ratatui::crossterm::event::KeyModifiers;

    fn press(state: &mut AppState, data: &mut AppData, code: KeyCode, mods: KeyModifiers) -> Routed {
        route(KeyEvent::new(code, mods), state, data)
    }

    fn ch(state: &mut AppState, data: &mut AppData, c: char) -> Routed {
        let mods = if c.is_uppercase() { KeyModifiers::SHIFT } else { KeyModifiers::NONE };
        press(state, data, KeyCode::Char(c), mods)
    }

    fn track() -> Track {
        Track {
            title: "t".to_string(),
            artists: String::new(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: "soundcloud:tracks:1".to_string(),
            permalink_url: String::new(),
        }
    }

    #[test]
    fn overlay_swallows_unbound_and_bound_keys() {
        let opens: [(&str, crate::tui::render::testkit::Setup); 3] = [
            ("queue", |s| s.queue_visible = true),
            ("history", |s| s.history_visible = true),
            ("picker", |s| playlist_picker::open_picker(s, track())),
        ];
        for (name, open) in opens {
            let mut state = AppState::default();
            let mut data = AppData { likes: vec![track(), track(), track()], ..Default::default() };
            open(&mut state);
            // l = next sub-tab, j = down, tab = next tab, enter = play, z and F5 = unbound.
            for c in ['l', 'j', 'z', 'S'] {
                let routed = ch(&mut state, &mut data, c);
                assert!(!matches!(routed, Routed::Action(_)), "{name}: {c} reached the view");
            }
            for code in [KeyCode::Tab, KeyCode::Enter, KeyCode::Right, KeyCode::F(5)] {
                let routed = press(&mut state, &mut data, code, KeyModifiers::NONE);
                assert!(!matches!(routed, Routed::Action(_)), "{name}: {code:?} reached the view");
            }
            assert_eq!((state.selected_tab, state.selected_subtab), (0, 0), "{name}");
            assert_eq!(nav::cursor(&state, nav::PaneId::Likes), 0, "{name}");
            assert!(!state.shuffle_enabled, "{name}");
        }

        // The queue popup scrolls with the movement keys and closes on Esc or its own key.
        let mut state = AppState::default();
        let mut data = AppData::default();
        state.queue_visible = true;
        state.auto_queue = (0..5).collect();
        ch(&mut state, &mut data, 'j');
        ch(&mut state, &mut data, 'j');
        assert_eq!(state.queue_scroll, 2);
        press(&mut state, &mut data, KeyCode::End, KeyModifiers::NONE);
        assert_eq!(state.queue_scroll, 4);
        ch(&mut state, &mut data, 'q');
        assert!(!state.queue_visible);
        state.queue_visible = true;
        press(&mut state, &mut data, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!state.queue_visible && !state.quit_confirm_visible);
    }

    #[test]
    fn search_requests_are_debounced_and_never_repeated() {
        use crate::tui::logic::state::SEARCH_DEBOUNCE;
        use std::time::Duration;
        let mut state = AppState::default();
        let mut data = AppData::default();
        nav::set_location(&mut state, 1, None);
        commands::run_command(Action::Search, &mut state, &mut data);
        // What the main loop asks on every pass, `ms` after the last edit of the query.
        let due = |state: &mut AppState, ms: u64| {
            let edited = state.search_edited_at.unwrap_or_else(std::time::Instant::now);
            state.search_due(edited + Duration::from_millis(ms))
        };
        let quiet = SEARCH_DEBOUNCE.as_millis() as u64;

        // Several edits inside the window: nothing goes out until the query has sat
        // unedited for the whole of it, and then one request does, with no further key.
        for c in ['l', 'o', 'f', 'i'] {
            ch(&mut state, &mut data, c);
            assert!(!due(&mut state, quiet - 1), "sent while still typing {c:?}");
        }
        assert!(due(&mut state, quiet));
        assert_eq!(state.search_last_sent, Some((0, "lofi".to_string())));
        assert!(!due(&mut state, quiet + 1000), "one request per pause");

        // Enter sends what is pending without waiting.
        ch(&mut state, &mut data, 'x');
        press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE);
        assert!(due(&mut state, 0));
        // ...and sends nothing when the query is the one already sent.
        commands::run_command(Action::Search, &mut state, &mut data);
        press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE);
        assert!(!due(&mut state, 0));

        // Edited and put back, or only padded: still the search that was sent.
        commands::run_command(Action::Search, &mut state, &mut data);
        ch(&mut state, &mut data, 'y');
        press(&mut state, &mut data, KeyCode::Backspace, KeyModifiers::NONE);
        ch(&mut state, &mut data, ' ');
        assert!(!due(&mut state, quiet));

        // A change of search type sends at once, replacing an edit still waiting: one request.
        ch(&mut state, &mut data, 'z');
        press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE);
        state.search_edited_at = Some(std::time::Instant::now());
        nav::set_location(&mut state, 1, Some(1));
        assert!(due(&mut state, 0));
        assert_eq!(state.search_last_sent, Some((1, "lofix z".to_string())));
        assert!(!due(&mut state, quiet), "the debounced edit must not send a second one");
        // There and back before the loop runs: the same type and query as last time.
        nav::set_location(&mut state, 1, Some(0));
        nav::set_location(&mut state, 1, Some(1));
        assert!(!due(&mut state, 0));
        // Back on the first type its results are gone, so that one is sent.
        nav::set_location(&mut state, 1, Some(0));
        assert!(due(&mut state, 0));

        // An emptied query is passed to the loop once (it clears the lists and sends
        // nothing, see the dispatch site); spaces are the same empty query.
        commands::run_command(Action::Search, &mut state, &mut data);
        press(&mut state, &mut data, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert!(due(&mut state, quiet));
        ch(&mut state, &mut data, ' ');
        assert!(!due(&mut state, quiet));
    }

    #[test]
    fn insert_mode_types_capitals() {
        // Search query.
        let mut state = AppState::default();
        let mut data = AppData::default();
        nav::set_location(&mut state, 1, None);
        assert!(!state.search_typing, "switching tab never starts typing");
        commands::run_command(Action::Search, &mut state, &mut data);
        for c in ['A', 'S', 'Q', ' ', 'q', '?', '/'] {
            assert!(matches!(ch(&mut state, &mut data, c), Routed::Done(_)));
        }
        assert_eq!(state.query, "ASQ q?/");
        assert!(!state.queue_visible && !state.shuffle_enabled && !state.help_visible);
        assert!(state.manual_queue.is_empty() && !state.quit_confirm_visible);
        press(&mut state, &mut data, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(state.query, "ASQ ");
        press(&mut state, &mut data, KeyCode::Backspace, KeyModifiers::NONE);
        press(&mut state, &mut data, KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(state.query, "AS");
        press(&mut state, &mut data, KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(state.selected_tab, 1, "Tab is not a command while typing");
        press(&mut state, &mut data, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(state.query, "");
        ch(&mut state, &mut data, 'x');
        press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE);
        assert!(!state.search_typing && state.query == "x");
        assert!(matches!(ch(&mut state, &mut data, 'q'), Routed::Action(Action::ToggleQueue)));

        // Library filter.
        let mut state = AppState::default();
        let mut data = AppData { likes: vec![track(), Track { title: "ASQ".to_string(), ..track() }], ..Default::default() };
        commands::run_command(Action::Search, &mut state, &mut data);
        for c in ['A', 'S', 'Q'] {
            assert!(matches!(ch(&mut state, &mut data, c), Routed::Done(_)));
        }
        assert_eq!(state.search_query, "ASQ");
        assert_eq!(state.search_matches, [1]);
        assert!(!state.queue_visible && !state.shuffle_enabled && state.manual_queue.is_empty());
        // Enter keeps the filter and goes back to commands.
        press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE);
        assert!(!state.search_typing && state.search_popup_visible);
        assert_eq!(nav::selected_index(&state, nav::PaneId::Likes), Some(1));
        assert!(matches!(ch(&mut state, &mut data, 'q'), Routed::Action(Action::ToggleQueue)));
        // Esc in normal mode clears it, leaving the cursor on the same track.
        press(&mut state, &mut data, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!state.search_popup_visible && state.search_query.is_empty());
        assert_eq!(nav::cursor(&state, nav::PaneId::Likes), 1);
        // Esc while typing cancels.
        commands::run_command(Action::Search, &mut state, &mut data);
        ch(&mut state, &mut data, 'A');
        press(&mut state, &mut data, KeyCode::Esc, KeyModifiers::NONE);
        assert!(!state.search_typing && !state.search_popup_visible);

        // New playlist name.
        let mut state = AppState::default();
        commands::run_command(Action::NewPlaylist, &mut state, &mut data);
        for c in ['A', 'S', 'Q'] {
            ch(&mut state, &mut data, c);
        }
        assert_eq!(state.playlist_picker_title.as_deref(), Some("ASQ"));
        press(&mut state, &mut data, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(state.playlist_picker_title.as_deref(), Some(""));
    }

    #[test]
    fn esc_never_opens_quit() {
        let esc = |state: &mut AppState, data: &mut AppData| {
            let routed = press(state, data, KeyCode::Esc, KeyModifiers::NONE);
            assert!(matches!(routed, Routed::Done(InputOutcome::Continue)));
            assert!(!state.quit_confirm_visible);
        };
        let mut data = AppData::default();
        for tab in 0..3 {
            let mut state = AppState::default();
            nav::set_location(&mut state, tab, None);
            esc(&mut state, &mut data);
        }
        // Not even when an old config still binds it.
        let mut state = AppState::default();
        state.keymap.apply_toml("[keys]\nquit = \"esc\"\n");
        esc(&mut state, &mut data);
        // It leaves the visualiser instead.
        state.visualizer_mode = true;
        esc(&mut state, &mut data);
        assert!(!state.visualizer_mode);

        let mut state = AppState::default();
        assert!(matches!(ch(&mut state, &mut data, 'Q'), Routed::Action(Action::Quit)));
    }

    #[test]
    fn sequences_work_in_normal_mode_and_in_overlays() {
        let mut state = AppState::default();
        let mut data = AppData::default();
        assert!(matches!(ch(&mut state, &mut data, 'g'), Routed::Done(_)));
        assert_eq!(state.pending_keys.len(), 1);
        assert!(state.pending_since.is_some());
        assert!(matches!(ch(&mut state, &mut data, 'g'), Routed::Action(Action::Top)));
        assert!(state.pending_keys.is_empty() && state.pending_since.is_none());
        // The key that breaks a sequence still counts on its own.
        ch(&mut state, &mut data, 'g');
        assert!(matches!(ch(&mut state, &mut data, 'j'), Routed::Action(Action::Down)));
        // Esc drops what was waiting; so does the pause, once the loop notices.
        ch(&mut state, &mut data, 'g');
        press(&mut state, &mut data, KeyCode::Esc, KeyModifiers::NONE);
        assert!(state.pending_keys.is_empty());
        ch(&mut state, &mut data, 'g');
        expire_pending(&mut state);
        assert_eq!(state.pending_keys.len(), 1, "not yet");
        state.pending_since = std::time::Instant::now().checked_sub(SEQ_TIMEOUT);
        expire_pending(&mut state);
        assert!(state.pending_keys.is_empty() && state.pending_since.is_none());
        // Insert mode never sees, or leaves behind, a pending key.
        ch(&mut state, &mut data, 'g');
        state.search_typing = true;
        ch(&mut state, &mut data, 'g');
        assert!(state.pending_keys.is_empty());
        assert_eq!(state.search_query, "g");
        state.search_typing = false;

        // History and queue popups: g g and G reach the ends, j/k step.
        let mut state = AppState {
            queue_visible: true,
            auto_queue: (0..30).collect(),
            ..Default::default()
        };
        ch(&mut state, &mut data, 'G');
        assert_eq!(state.queue_scroll, 29);
        ch(&mut state, &mut data, 'k');
        assert_eq!(state.queue_scroll, 28);
        ch(&mut state, &mut data, 'g');
        assert_eq!((state.queue_scroll, state.pending_keys.len()), (28, 1));
        ch(&mut state, &mut data, 'g');
        assert_eq!(state.queue_scroll, 0);
        press(&mut state, &mut data, KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(state.queue_scroll, 5);
        // So does the playlist picker.
        let mut state = AppState::default();
        playlist_picker::open_picker(&mut state, track());
        ch(&mut state, &mut data, 'j');
        assert_eq!(state.playlist_picker_selected, 0, "only the New playlist row");
        ch(&mut state, &mut data, 'G');
        assert_eq!(state.playlist_picker_selected, 0);
    }

    fn type_into_palette(state: &mut AppState, data: &mut AppData, text: &str) {
        for c in text.chars() {
            assert!(matches!(ch(state, data, c), Routed::Done(_)), "{c} ran an action");
        }
    }

    #[test]
    fn o_cycles_pages() {
        use crate::tui::logic::state::PanelPage;
        let mut state = AppState::default();
        let mut data = AppData::default();
        assert_eq!(state.panel_page, PanelPage::UpNext);
        // Whatever the layout: the page is there when the user switches to the panel.
        for (layout, want) in [
            (crate::config::Layout::PlayerPanel, PanelPage::Lyrics),
            (crate::config::Layout::PlayerPanel, PanelPage::Spectrum),
            (crate::config::Layout::Classic, PanelPage::History),
            (crate::config::Layout::Columns, PanelPage::UpNext),
        ] {
            state.settings.layout = layout;
            let Routed::Action(action) = ch(&mut state, &mut data, 'o') else { panic!("o is not bound") };
            assert_eq!(action, Action::PanelPage);
            commands::run_command(action, &mut state, &mut data);
            assert_eq!(state.panel_page, want);
            assert!(state.toasts.is_empty());
        }
        // The navigation column does not swallow it.
        state.nav_focused = true;
        assert!(!commands::run_nav_command(Action::PanelPage, &mut state, &mut data));
    }

    #[test]
    fn q_and_h_still_open_popups_in_panel_layout() {
        use crate::tui::logic::state::PanelPage;
        let mut state = AppState::default();
        let mut data = AppData::default();
        state.settings.layout = crate::config::Layout::PlayerPanel;
        for (key, want) in [('q', Action::ToggleQueue), ('H', Action::ToggleHistory)] {
            let Routed::Action(action) = ch(&mut state, &mut data, key) else { panic!("{key} is not bound") };
            assert_eq!(action, want);
            commands::run_command(action, &mut state, &mut data);
            // Popups, not pages: the panel stays where `o` left it.
            assert!(state.queue_visible != state.history_visible, "{key} opened its popup");
            assert_eq!(state.panel_page, PanelPage::UpNext);
            (state.queue_visible, state.history_visible) = (false, key == 'H');
        }
        // The popups are modal as in any layout: `o` does not reach the panel behind.
        assert!(!matches!(ch(&mut state, &mut data, 'o'), Routed::Action(_)));
        assert_eq!(state.panel_page, PanelPage::UpNext);
    }

    #[test]
    fn palette_runs_selected_command_and_closes() {
        let mut state = AppState::default();
        let mut data = AppData::default();
        // Both default keys open it.
        assert!(matches!(press(&mut state, &mut data, KeyCode::Char('p'), KeyModifiers::CONTROL), Routed::Action(Action::Palette)));
        assert!(matches!(ch(&mut state, &mut data, ':'), Routed::Action(Action::Palette)));
        commands::run_command(Action::Palette, &mut state, &mut data);
        assert_eq!(state.palette, Some(Default::default()));

        // A layout command is applied, toasted, and the palette is gone.
        type_into_palette(&mut state, &mut data, "lay sb");
        assert!(matches!(press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE), Routed::Done(_)));
        assert!(state.palette.is_none());
        assert_eq!(state.settings.layout, crate::config::Layout::Sidebar);
        assert_eq!(state.toasts.last().unwrap().msg, "Layout: sidebar");

        // An action comes back to be dispatched like its key press.
        commands::run_command(Action::Palette, &mut state, &mut data);
        type_into_palette(&mut state, &mut data, "toggle shuffle");
        assert!(matches!(press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE), Routed::Action(Action::ToggleShuffle)));
        assert!(state.palette.is_none());

        // Down / Up pick another row and stop at the ends; typing goes back to the first.
        commands::run_command(Action::Palette, &mut state, &mut data);
        type_into_palette(&mut state, &mut data, "layout:");
        let rows = crate::tui::logic::palette::matches(&state, "layout:").len();
        assert_eq!(rows, 5);
        for (code, mods, want) in [
            (KeyCode::Down, KeyModifiers::NONE, 1),
            (KeyCode::Tab, KeyModifiers::NONE, 2),
            (KeyCode::Char('n'), KeyModifiers::CONTROL, 3),
            (KeyCode::Char('j'), KeyModifiers::CONTROL, 4),
            (KeyCode::Down, KeyModifiers::NONE, 4),
            (KeyCode::Up, KeyModifiers::NONE, 3),
            (KeyCode::BackTab, KeyModifiers::SHIFT, 2),
            (KeyCode::Char('p'), KeyModifiers::CONTROL, 1),
            (KeyCode::Char('k'), KeyModifiers::CONTROL, 0),
            (KeyCode::Up, KeyModifiers::NONE, 0),
            (KeyCode::Down, KeyModifiers::NONE, 1),
        ] {
            press(&mut state, &mut data, code, mods);
            assert_eq!(state.palette.as_ref().unwrap().selected, want, "{code:?}");
        }
        assert_eq!(state.palette.as_ref().unwrap().query, "layout:", "moving types nothing");
        press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(state.settings.layout, crate::config::Layout::Tighten, "the second layout row");
        commands::run_command(Action::Palette, &mut state, &mut data);
        press(&mut state, &mut data, KeyCode::Down, KeyModifiers::NONE);
        ch(&mut state, &mut data, 'x');
        assert_eq!(state.palette.as_ref().unwrap().selected, 0);

        // Nothing matches: Enter just closes. Esc closes without running anything.
        type_into_palette(&mut state, &mut data, "zzzz");
        assert!(matches!(press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE), Routed::Done(_)));
        assert!(state.palette.is_none());
        commands::run_command(Action::Palette, &mut state, &mut data);
        type_into_palette(&mut state, &mut data, "quit");
        assert!(matches!(press(&mut state, &mut data, KeyCode::Esc, KeyModifiers::NONE), Routed::Done(_)));
        assert!(state.palette.is_none() && !state.quit_confirm_visible);

        // The other commands: the hint line flips, the wizard is asked for.
        commands::run_command(Action::Palette, &mut state, &mut data);
        type_into_palette(&mut state, &mut data, "hint line");
        press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE);
        assert!(!state.settings.hint_line);
        commands::run_command(Action::Palette, &mut state, &mut data);
        type_into_palette(&mut state, &mut data, "setup wiz");
        press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE);
        assert!(state.wizard.take().is_some() && state.palette.is_none());

        // Behind the visualiser a selection action is dropped, as its key would be.
        state.visualizer_mode = true;
        commands::run_command(Action::Palette, &mut state, &mut data);
        type_into_palette(&mut state, &mut data, "add the selected track to the queue");
        assert!(matches!(press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE), Routed::Done(_)));
        assert!(state.palette.is_none());
    }

    #[test]
    fn palette_is_insert_mode() {
        let mut state = AppState::default();
        let mut data = AppData { likes: vec![track(), track(), track()], ..Default::default() };
        // A pending key does not survive into the query.
        ch(&mut state, &mut data, 'g');
        commands::run_command(Action::Palette, &mut state, &mut data);
        type_into_palette(&mut state, &mut data, "Q");
        assert_eq!(state.palette.as_ref().unwrap().query, "Q");
        assert!(!state.queue_visible && state.pending_keys.is_empty());
        type_into_palette(&mut state, &mut data, "gg q?/:jl S");
        assert_eq!(state.palette.as_ref().unwrap().query, "Qgg q?/:jl S");
        assert!(!state.help_visible && !state.quit_confirm_visible && !state.shuffle_enabled);
        assert_eq!((nav::location(&state), nav::cursor(&state, nav::PaneId::Likes)), ((0, 0), 0));
        assert!(state.pending_keys.is_empty());
        // The editing keys every text field has.
        press(&mut state, &mut data, KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(state.palette.as_ref().unwrap().query, "Qgg q?/:jl ");
        press(&mut state, &mut data, KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(state.palette.as_ref().unwrap().query, "Qgg q?/:jl");
        press(&mut state, &mut data, KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert_eq!(state.palette.as_ref().unwrap().query, "");
        // Unbound and function keys are dropped; the palette stays open.
        for code in [KeyCode::F(5), KeyCode::Left, KeyCode::Home] {
            assert!(matches!(press(&mut state, &mut data, code, KeyModifiers::NONE), Routed::Done(_)));
        }
        assert!(state.palette.is_some());
    }

    #[test]
    fn visualiser_drops_selection_actions_but_keeps_playback() {
        let mut state = AppState::default();
        let mut data = AppData::default();
        state.visualizer_mode = true;
        assert!(matches!(press(&mut state, &mut data, KeyCode::Enter, KeyModifiers::NONE), Routed::Done(_)));
        assert!(matches!(ch(&mut state, &mut data, '/'), Routed::Done(_)));
        assert!(matches!(ch(&mut state, &mut data, ' '), Routed::Action(Action::PlayPause)));
        let before = state.visualizer_view;
        press(&mut state, &mut data, KeyCode::Tab, KeyModifiers::NONE);
        assert!(state.visualizer_view != before && state.selected_tab == 0);
    }
}
