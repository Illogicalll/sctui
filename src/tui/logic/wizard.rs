//! The first-run setup wizard: welcome, choose a layout, choose a theme, then a guided
//! practice of the important keys in the real app. Lessons complete when the *action*
//! happens (never a key code), so they still work after the user rebinds keys.

use crate::config::Layout;
use crate::keymap::Action;

use super::filtering::{build_filtered_views, is_filter_active};
use super::nav;
use super::state::{AppData, AppState};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    Welcome,
    Layout,
    Theme,
    /// The real app with a lesson card. The only stage that is not modal.
    Practice,
    Done,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Wizard {
    pub stage: Stage,
    /// Index into [`LESSONS`] during practice.
    pub step: usize,
    /// Highlighted row of the layout list.
    pub selected: usize,
    /// Parts of the current lesson done so far, one bit each.
    pub sub: u8,
    /// The layout to go back to if the wizard is skipped before one is chosen.
    pub revert_layout: Option<Layout>,
    /// The lesson finished last, which the card shows ticked.
    pub ticked: Option<usize>,
}

impl Wizard {
    pub fn at(stage: Stage) -> Wizard {
        Wizard { stage, step: 0, selected: 0, sub: 0, revert_layout: None, ticked: None }
    }
}

/// A key on the lesson card: a bound action, shown as the live keymap has it, or a
/// key that is the same in every keymap (Esc, Enter in a popup).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Bound(Action),
    Fixed(&'static str),
}

pub struct Lesson {
    pub goal: &'static str,
    pub why: &'static str,
    /// The keys to press, in order, each with what it does.
    pub keys: &'static [(Key, &'static str)],
    /// How many times the (single) key is wanted.
    pub times: u8,
    /// Skipped when there is no track to practise on.
    pub needs_track: bool,
}

const fn lesson(goal: &'static str, why: &'static str, keys: &'static [(Key, &'static str)]) -> Lesson {
    Lesson { goal, why, keys, times: 1, needs_track: false }
}

use Key::{Bound, Fixed};

/// The practice steps, in order. Only actions that leave the SoundCloud account alone.
pub const LESSONS: [Lesson; 10] = [
    Lesson { times: 3, ..lesson("Move down a few rows", "Movement always acts on the focused pane.", &[(Bound(Action::Down), "down, three times")]) },
    lesson("Jump to the bottom, then the top", "The quick way round a long list.", &[(Bound(Action::Bottom), "bottom"), (Bound(Action::Top), "top")]),
    lesson("Go to the next sub-tab", "Likes, Playlists, Albums, Following.", &[(Bound(Action::SubTabRight), "next sub-tab")]),
    lesson("Focus the next pane, and come back", "Keys act on the pane with the border.", &[(Bound(Action::PaneRight), "in"), (Bound(Action::PaneLeft), "back")]),
    Lesson { needs_track: true, ..lesson("Play a track", "Plays the selected track, or opens a row.", &[(Bound(Action::PlaySelected), "play")]) },
    Lesson { times: 2, needs_track: true, ..lesson("Pause, then resume", "Works wherever you are in the lists.", &[(Bound(Action::PlayPause), "pause, then again")]) },
    Lesson { needs_track: true, ..lesson("Add a track to the queue", "Queued tracks play next, in order.", &[(Bound(Action::AddToQueue), "queue")]) },
    lesson("Filter the list, then clear it", "Type to narrow, enter keeps, esc clears.", &[(Bound(Action::Search), "filter"), (Fixed("esc"), "clear")]),
    lesson("Open the palette and run a command", "Every action, layout and theme by name.", &[(Bound(Action::Palette), "palette"), (Fixed("enter"), "run")]),
    lesson("Open the keys popup and close it", "Every key, rebindable, plus settings.", &[(Bound(Action::Help), "keys"), (Fixed("esc"), "close")]),
];

/// What a lesson needs to know about the app around one key press.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Observed {
    /// A tracks pane has focus and there is a track under its cursor.
    pub track_selected: bool,
    /// There is a track somewhere to practise on.
    pub has_tracks: bool,
    /// `[` / `]` have somewhere to go.
    pub multi_pane: bool,
    pub filter: bool,
    pub palette: bool,
    pub help: bool,
}

impl Observed {
    pub fn of(state: &AppState, data: &AppData) -> Observed {
        let views = build_filtered_views(state, data);
        let track_selected = !nav::in_nav(state) && nav::selected_track(state, data, &views).is_some();
        Observed {
            track_selected,
            has_tracks: track_selected || !data.likes.is_empty(),
            multi_pane: nav::panes(state).len() > 1 || nav::has_nav(state.settings.layout),
            filter: is_filter_active(state),
            palette: state.palette.is_some(),
            help: state.help_visible,
        }
    }
}

impl Wizard {
    /// Step past lessons that need a track when there is none; after the last lesson,
    /// the Done screen.
    fn settle(&mut self, has_tracks: bool) {
        while LESSONS.get(self.step).is_some_and(|l| l.needs_track && !has_tracks) {
            self.step += 1;
        }
        if self.step >= LESSONS.len() {
            self.stage = Stage::Done;
        }
    }

    fn advance(&mut self, has_tracks: bool) {
        self.ticked = Some(self.step);
        self.step += 1;
        self.sub = 0;
        self.settle(has_tracks);
    }

    /// One key press during practice: the action it resolved to (if any) and the app
    /// just before and just after it ran. Returns whether that finished a lesson.
    pub fn observe(&mut self, action: Option<Action>, before: &Observed, after: &Observed) -> bool {
        if self.stage != Stage::Practice {
            return false;
        }
        self.settle(after.has_tracks);
        let sub = self.sub;
        let done = match (self.step, action) {
            (0, Some(Action::Down)) => {
                self.sub = sub << 1 | 1;
                self.sub == 0b111
            }
            (1, Some(Action::Bottom)) => {
                self.sub = 1;
                false
            }
            (1, Some(Action::Top)) => sub == 1,
            (2, Some(Action::SubTabRight)) => true,
            // Either order: in Columns the first pane's way "in" is from the column.
            (3, Some(Action::PaneRight)) if before.multi_pane => {
                self.sub |= 1;
                self.sub == 3
            }
            (3, Some(Action::PaneLeft)) if before.multi_pane => {
                self.sub |= 2;
                self.sub == 3
            }
            (4, Some(Action::PlaySelected)) => before.track_selected,
            (5, Some(Action::PlayPause)) => {
                self.sub = sub << 1 | 1;
                self.sub == 0b11
            }
            (6, Some(Action::AddToQueue)) => before.track_selected,
            (7, _) => {
                self.sub |= after.filter as u8;
                sub == 1 && !after.filter
            }
            // Finished by `ran_command`.
            (8, _) => {
                self.sub |= after.palette as u8;
                false
            }
            (9, _) => {
                self.sub |= after.help as u8;
                sub == 1 && !after.help
            }
            _ => false,
        };
        if done {
            self.advance(after.has_tracks);
        }
        done
    }

    /// A command was run from the palette (lesson 9's second half).
    pub fn ran_command(&mut self) {
        if self.stage == Stage::Practice && self.step == 8 {
            // No later lesson needs a track.
            self.advance(true);
        }
    }
}

/// The practice stage is running (the app is live, with a lesson card).
pub fn practising(state: &AppState) -> bool {
    state.wizard.as_ref().is_some_and(|w| w.stage == Stage::Practice)
}

/// One of the wizard's modal screens, or the upgrade prompt, owns the keyboard.
pub fn modal(state: &AppState) -> bool {
    state.tour_prompt || state.wizard.as_ref().is_some_and(|w| w.stage != Stage::Practice)
}

/// Before a key is handled: what lessons will compare against. `None` unless practising.
pub fn watch(state: &AppState, data: &AppData) -> Option<Observed> {
    practising(state).then(|| Observed::of(state, data))
}

/// After a key was handled: let the current lesson see what happened.
pub fn observe(state: &mut AppState, data: &AppData, action: Option<Action>, before: Option<Observed>) {
    let Some(before) = before else { return };
    let after = Observed::of(state, data);
    if let Some(wizard) = &mut state.wizard {
        wizard.observe(action, &before, &after);
    }
}

/// What happens when the TUI starts. `fresh`: there was no config file at all.
/// `setup`: the `--setup` flag. A fresh install gets the full wizard, and Tighten if it
/// skips it; an existing user who has not been through it keeps their layout and is
/// asked once whether to take the tour.
pub fn on_start(state: &mut AppState, fresh: bool, setup: bool) {
    if fresh {
        state.settings.layout = Layout::Tighten;
    }
    if fresh || setup {
        start(state);
    } else if !state.settings.onboarded {
        state.tour_prompt = true;
        state.confirm_selected = 0;
    }
}

/// Run the whole wizard from the welcome screen.
pub fn start(state: &mut AppState) {
    state.tour_prompt = false;
    state.wizard = Some(Wizard { revert_layout: Some(state.settings.layout), ..Wizard::at(Stage::Welcome) });
}

/// Go straight to the practice stage (after the theme stage, or from the upgrade prompt).
pub fn start_practice(state: &mut AppState, data: &AppData) {
    let mut wizard = Wizard::at(Stage::Practice);
    wizard.settle(Observed::of(state, data).has_tracks);
    state.wizard = Some(wizard);
}

/// Esc: leave the wizard wherever it is. A layout that was only being previewed goes back.
pub fn skip(state: &mut AppState) {
    if let Some(layout) = state.wizard.as_ref().and_then(|w| w.revert_layout) {
        state.settings.layout = layout;
    }
    finish(state, "Setup skipped · rerun it from the palette: Setup wizard");
}

/// Close the wizard or the upgrade prompt for good: record `onboarded` and save.
pub fn finish(state: &mut AppState, said: impl Into<String>) {
    state.wizard = None;
    state.tour_prompt = false;
    state.settings.onboarded = true;
    match crate::config::save(
        state.config_load_failed,
        &state.keymap,
        &state.theme_name,
        &state.theme_overrides,
        &state.settings,
        &state.eq,
    ) {
        Ok(()) => state.toast(said),
        Err(e) => state.toast_err(format!("Setup not saved: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_install_starts_wizard() {
        let mut state = AppState::default();
        on_start(&mut state, true, false);
        let wizard = state.wizard.as_ref().expect("no config file: the wizard runs");
        assert_eq!(wizard.stage, Stage::Welcome);
        assert!(!state.tour_prompt && !state.settings.onboarded);
        assert!(modal(&state) && !practising(&state));

        // `--setup` reruns it for someone who has been through it, and leaves their layout.
        let mut state = AppState::default();
        (state.settings.onboarded, state.settings.layout) = (true, Layout::Columns);
        on_start(&mut state, false, true);
        assert_eq!(state.wizard.as_ref().map(|w| w.stage), Some(Stage::Welcome));
        assert_eq!(state.settings.layout, Layout::Columns);
        skip(&mut state);
        assert_eq!(state.settings.layout, Layout::Columns);

        // Onboarded and no flag: nothing.
        let mut state = AppState::default();
        state.settings.onboarded = true;
        on_start(&mut state, false, false);
        assert!(state.wizard.is_none() && !state.tour_prompt && !modal(&state));
    }

    #[test]
    fn existing_config_without_onboarded_gets_tour_prompt_not_wizard_and_keeps_classic() {
        let mut state = AppState::default();
        on_start(&mut state, false, false);
        assert!(state.wizard.is_none() && state.tour_prompt && modal(&state));
        assert_eq!(state.settings.layout, Layout::Classic);
        assert!(!state.settings.onboarded, "not until it is answered");
        // Whatever layout the file names stays.
        let mut state = AppState::default();
        state.settings.layout = Layout::Columns;
        on_start(&mut state, false, false);
        assert_eq!(state.settings.layout, Layout::Columns);
    }

    #[test]
    fn skipped_wizard_defaults_to_tighten() {
        let mut state = AppState::default();
        on_start(&mut state, true, false);
        skip(&mut state);
        assert_eq!(state.settings.layout, Layout::Tighten);
        assert!(state.settings.onboarded && state.wizard.is_none());
        assert!(state.toasts.last().unwrap().msg.contains("Setup wizard"));
        // Also when it is skipped while another layout is only being previewed.
        let mut state = AppState::default();
        on_start(&mut state, true, false);
        state.settings.layout = Layout::Columns;
        skip(&mut state);
        assert_eq!(state.settings.layout, Layout::Tighten);
    }

    fn practice() -> (Wizard, Observed) {
        (Wizard::at(Stage::Practice), Observed { track_selected: true, has_tracks: true, multi_pane: true, ..Default::default() })
    }

    #[test]
    fn two_part_lessons_need_both_parts() {
        let (mut w, o) = practice();
        // Three downs, not two.
        for done in [false, false, true] {
            assert_eq!(w.observe(Some(Action::Down), &o, &o), done);
        }
        // Bottom then top: top alone, or top first, is not it.
        assert_eq!(w.step, 1);
        assert!(!w.observe(Some(Action::Top), &o, &o));
        assert!(!w.observe(Some(Action::Bottom), &o, &o));
        assert_eq!(w.sub, 1);
        assert!(w.observe(Some(Action::Top), &o, &o));
        assert!(w.observe(Some(Action::SubTabRight), &o, &o));
        // Focus in and back: one direction twice is not both; a one-pane location does not count.
        assert_eq!(w.step, 3);
        let one_pane = Observed { multi_pane: false, ..o };
        assert!(!w.observe(Some(Action::PaneRight), &one_pane, &one_pane));
        assert_eq!(w.sub, 0);
        assert!(!w.observe(Some(Action::PaneRight), &o, &o));
        assert!(!w.observe(Some(Action::PaneRight), &o, &o));
        assert!(w.observe(Some(Action::PaneLeft), &o, &o));
        // Play needs a track under the cursor (Enter on a playlist only steps in).
        let container = Observed { track_selected: false, ..o };
        assert!(!w.observe(Some(Action::PlaySelected), &container, &o));
        assert!(w.observe(Some(Action::PlaySelected), &o, &o));
        // Pause and resume.
        assert!(!w.observe(Some(Action::PlayPause), &o, &o));
        assert!(w.observe(Some(Action::PlayPause), &o, &o));
        assert!(!w.observe(Some(Action::AddToQueue), &container, &container));
        assert!(w.observe(Some(Action::AddToQueue), &o, &o));
        // Filter then clear: no filter yet is not "cleared".
        assert_eq!(w.step, 7);
        let filtered = Observed { filter: true, ..o };
        assert!(!w.observe(Some(Action::Search), &o, &o));
        assert!(!w.observe(None, &o, &filtered));
        assert!(!w.observe(None, &filtered, &filtered));
        assert!(w.observe(None, &filtered, &o));
        // Palette: opening it is half; running a command is the rest.
        let open = Observed { palette: true, ..o };
        assert!(!w.observe(Some(Action::Palette), &o, &open));
        assert_eq!((w.step, w.sub), (8, 1));
        assert!(!w.observe(None, &open, &o), "closed with esc: nothing ran");
        w.ran_command();
        // Keys popup: open, then closed.
        assert_eq!((w.step, w.sub), (9, 0));
        let help = Observed { help: true, ..o };
        assert!(!w.observe(None, &o, &o));
        assert!(!w.observe(Some(Action::Help), &o, &help));
        assert!(w.observe(None, &help, &o));
        assert_eq!(w.stage, Stage::Done);
        // Nothing more to observe.
        assert!(!w.observe(Some(Action::Down), &o, &o));
    }

    #[test]
    fn lessons_fit_their_card_and_only_use_harmless_actions() {
        let mutating = [
            Action::ToggleLike, Action::ToggleLikePlaying, Action::AddToPlaylist, Action::AddToPlaylistPlaying,
            Action::NewPlaylist, Action::RemoveFromPlaylist, Action::DeletePlaylist, Action::Quit,
        ];
        for lesson in &LESSONS {
            assert!(lesson.goal.chars().count() <= 42 && lesson.why.chars().count() <= 42, "{}", lesson.goal);
            assert!(lesson.times == 1 || lesson.keys.len() == 1, "{}", lesson.goal);
            for (key, _) in lesson.keys {
                assert!(!matches!(key, Bound(a) if mutating.contains(a)), "{}", lesson.goal);
            }
        }
    }
}
