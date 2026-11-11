//! The command palette: every action, layout and theme in one list, narrowed by a
//! fuzzy query and run with Enter.

use std::cmp::Reverse;

use crate::config::Layout;
use crate::keymap::Action;
use crate::theme::Theme;

use super::state::AppState;

/// What a palette row does when it is run.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    Action(Action),
    Layout(Layout),
    /// A built-in theme, by name.
    Theme(&'static str),
    ToggleHints,
    ToggleCredit,
    ToggleHideUnplayable,
    SetupWizard,
}

/// The open palette: what has been typed and the highlighted row of the matches.
#[derive(Default, Clone, PartialEq, Eq, Debug)]
pub struct PaletteState {
    pub query: String,
    pub selected: usize,
}

/// Movement means nothing run from a list, and the palette is already open.
pub(crate) fn listed(action: Action) -> bool {
    !matches!(
        action,
        Action::Up
            | Action::Down
            | Action::PageUp
            | Action::PageDown
            | Action::HalfPageUp
            | Action::HalfPageDown
            | Action::Top
            | Action::Bottom
            | Action::PaneLeft
            | Action::PaneRight
            | Action::Palette
    )
}

/// Every command as (label, keys or current state, what it runs): the actions with
/// their live bindings, one row per layout and per theme, the hint line, the wizard.
pub fn items(state: &AppState) -> Vec<(String, String, Command)> {
    let current = |is: bool| if is { "current".to_string() } else { String::new() };
    let mut out: Vec<(String, String, Command)> = Action::ALL
        .into_iter()
        .filter(|a| listed(*a))
        .map(|a| {
            (a.description().to_string(), state.keymap.hint_labels(a), Command::Action(a))
        })
        .collect();
    out.extend(Layout::ALL.map(|l| {
        (format!("Layout: {}", l.label()), current(state.settings.layout == l), Command::Layout(l))
    }));
    out.extend(
        Theme::BUILTIN.map(|t| (format!("Theme: {}", t.name), current(state.theme_name == t.name), Command::Theme(t.name))),
    );
    let hints = if state.settings.hint_line { "on" } else { "off" };
    out.push(("Hint line: toggle".to_string(), hints.to_string(), Command::ToggleHints));
    let credit = state.settings.soundcloud_credit.label();
    out.push(("SoundCloud credit: text / glyph".to_string(), credit.to_string(), Command::ToggleCredit));
    let hiding = if state.settings.hide_unplayable { "on" } else { "off" };
    out.push(("Hide unplayable tracks: toggle".to_string(), hiding.to_string(), Command::ToggleHideUnplayable));
    out.push(("Setup wizard".to_string(), String::new(), Command::SetupWizard));
    out
}

/// Score `label` against `query`: `None` unless the query's characters appear in the
/// label in order (case ignored). Higher is better: runs of adjacent characters and
/// characters that start a word count for more, and a shorter label wins a tie.
pub fn fuzzy(query: &str, label: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let label: Vec<char> = label.to_lowercase().chars().collect();
    let (mut from, mut last, mut score) = (0, None, 0);
    for c in query.to_lowercase().chars() {
        let at = from + label[from..].iter().position(|l| *l == c)?;
        let adjacent = last.is_some_and(|l| l + 1 == at);
        let word_start = at == 0 || matches!(label[at - 1], ' ' | ':');
        score += if adjacent { 3 } else { 1 } + if word_start { 2 } else { 0 };
        last = Some(at);
        from = at + 1;
    }
    Some(score * 100 - label.len() as i32)
}

/// [`items`] that match `query`, best first; all of them, in order, for an empty query.
pub fn matches(state: &AppState, query: &str) -> Vec<(String, String, Command)> {
    let mut scored: Vec<(i32, (String, String, Command))> =
        items(state).into_iter().filter_map(|item| Some((fuzzy(query, &item.0)?, item))).collect();
    scored.sort_by_key(|(score, _)| Reverse(*score));
    scored.into_iter().map(|(_, item)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_subsequence_and_ranking() {
        let state = AppState::default();
        let found = matches(&state, "lay sb");
        assert_eq!(found[0].0, "Layout: sidebar", "{:?}", found.iter().map(|i| &i.0).collect::<Vec<_>>());
        assert_eq!(found[0].2, Command::Layout(Layout::Sidebar));
        assert!(matches(&state, "zzz").is_empty());
        assert_eq!(fuzzy("zzz", "Layout: sidebar"), None);
        // In order only, case ignored.
        assert!(fuzzy("LS", "Layout: sidebar").is_some());
        assert_eq!(fuzzy("sl", "Layout: s"), None);
        // Adjacent beats scattered, a word start beats mid-word, shorter beats longer.
        assert!(fuzzy("lay", "Layout") > fuzzy("lay", "l-a-y-out"));
        assert!(fuzzy("s", "Layout: sidebar") > fuzzy("s", "Layout: classic"));
        assert!(fuzzy("quit", "Quit") > fuzzy("quit", "Quit now"));
        assert_eq!(matches(&state, "quit")[0].2, Command::Action(Action::Quit));
        assert_eq!(matches(&state, "theme nord")[0].2, Command::Theme("nord"));
        // Non-ASCII text is matched by character, never sliced mid-char.
        assert!(fuzzy("語", "日本語").is_some());
    }

    #[test]
    fn empty_query_lists_everything() {
        let state = AppState::default();
        let all = items(&state);
        assert_eq!(matches(&state, ""), all, "everything, in its own order");
        for layout in Layout::ALL {
            assert!(all.iter().any(|i| i.2 == Command::Layout(layout)));
        }
        for theme in Theme::BUILTIN {
            assert!(all.iter().any(|i| i.2 == Command::Theme(theme.name)));
        }
        assert!(all.iter().any(|i| i.2 == Command::ToggleHints));
        assert!(all.iter().any(|i| i.2 == Command::SetupWizard));
        // The right-hand column says what is current and what the keys are.
        let right = |label: &str| all.iter().find(|i| i.0 == label).unwrap().1.clone();
        assert_eq!((right("Layout: classic"), right("Layout: sidebar")), ("current".to_string(), String::new()));
        assert_eq!(right("Hint line: toggle"), "on");
        assert_eq!(right("SoundCloud credit: text / glyph"), "text");
        assert_eq!((right("Open the selection on SoundCloud"), right("Open the currently playing track on SoundCloud")), ("b".to_string(), "B".to_string()));
        assert_eq!(right("Quit"), "q");
        // The hint line's notation, not `Shift+A, Ctrl+P`.
        assert_eq!(right("Play the selected track next"), "A");
        assert_eq!(right("Start a station from the selected track (or artist)"), "S, shift+enter");
    }

    #[test]
    fn every_action_is_listed_except_movement_and_close() {
        let mut state = AppState::default();
        state.keymap.clear(Action::ToggleRepeat);
        let all = items(&state);
        let hidden = [
            Action::Up, Action::Down, Action::PageUp, Action::PageDown, Action::HalfPageUp,
            Action::HalfPageDown, Action::Top, Action::Bottom, Action::PaneLeft, Action::PaneRight,
            Action::Palette,
        ];
        for action in Action::ALL {
            let row = all.iter().find(|i| i.2 == Command::Action(action));
            assert_eq!(row.is_some(), !hidden.contains(&action), "{}", action.name());
            if let Some((label, keys, _)) = row {
                assert_eq!(label, action.description());
                let want = if action == Action::ToggleRepeat { String::new() } else { state.keymap.hint_labels(action) };
                assert_eq!(*keys, want, "{}: unbound shows no keys", action.name());
            }
        }
    }
}
