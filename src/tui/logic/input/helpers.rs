use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::Track;
use crate::keymap::{Action, KeyResult, SEQ_TIMEOUT};
use crate::tui::logic::state::{AppState, PlaybackSource, QueuedTrack};

impl QueuedTrack {
    pub(crate) fn new(
        source: PlaybackSource,
        index: usize,
        track: &Track,
        tracks_snapshot: Option<&Vec<Track>>,
        playlist_uri: Option<String>,
        album_uri: Option<String>,
        following_user_urn: Option<String>,
        user_added: bool,
    ) -> Self {
        Self {
            source,
            index,
            track: track.clone(),
            tracks_snapshot: tracks_snapshot.cloned(),
            playlist_uri,
            album_uri,
            following_user_urn,
            user_added,
        }
    }
}

pub(crate) fn insert_manual_queue(state: &mut AppState, queued: QueuedTrack) {
    let mut items: Vec<QueuedTrack> = state.manual_queue.drain(..).collect();
    let insert_idx = match items.iter().rposition(|item| item.user_added) {
        Some(idx) => idx + 1,
        None => 0,
    };
    items.insert(insert_idx, queued);
    state.manual_queue = items.into_iter().collect();
}

/// Insert-mode editing shared by every text field: printable keys type (capitals
/// included), Backspace deletes, Ctrl+W deletes a word, Ctrl+U clears. Returns whether
/// the key was an edit.
pub(crate) fn edit_text(key: &KeyEvent, text: &mut String) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Backspace => {
            text.pop();
        }
        KeyCode::Char('w') if ctrl => {
            let keep = text
                .trim_end()
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_whitespace())
                .map_or(0, |(i, c)| i + c.len_utf8());
            text.truncate(keep);
        }
        KeyCode::Char('u') if ctrl => text.clear(),
        KeyCode::Char(c) if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
            text.push(c)
        }
        _ => return false,
    }
    true
}

/// What this key means under the user's keymap, sequences included. Every handler that
/// consults the keymap, in normal mode or in an overlay, asks here and nowhere else, so
/// `g g` works in a popup exactly as it does in the lists. `None` while a sequence is
/// still being typed.
pub(crate) fn lookup(state: &mut AppState, key: &KeyEvent) -> Option<Action> {
    if state.pending_since.is_some_and(|since| since.elapsed() >= SEQ_TIMEOUT) {
        state.pending_keys.clear();
    }
    let result = state.keymap.feed(&mut state.pending_keys, key);
    state.pending_since = (!state.pending_keys.is_empty()).then(Instant::now);
    match result {
        KeyResult::Action(action) => Some(action),
        KeyResult::Pending | KeyResult::None => None,
    }
}

/// Forget a half-typed sequence.
pub(crate) fn clear_pending(state: &mut AppState) {
    state.pending_keys.clear();
    state.pending_since = None;
}

/// Where a popup's list cursor goes for this key: the keymap's movement actions, with
/// the Up and Down arrows working even when unbound. `None` when the key does not move.
pub(crate) fn list_step(key: &KeyEvent, action: Option<Action>, at: usize, last: usize) -> Option<usize> {
    let action = match (action, key.code) {
        (Some(action), _) => action,
        (None, KeyCode::Up) => Action::Up,
        (None, KeyCode::Down) => Action::Down,
        _ => return None,
    };
    Some(match action {
        Action::Up => at.saturating_sub(1),
        Action::Down => (at + 1).min(last),
        Action::HalfPageUp => at.saturating_sub(5),
        Action::HalfPageDown => (at + 5).min(last),
        Action::PageUp => at.saturating_sub(10),
        Action::PageDown => (at + 10).min(last),
        Action::Top => 0,
        Action::Bottom => last,
        _ => return None,
    })
}

/// Arrow keys always flip a two-option Yes/No choice, plus whatever the user has bound to
/// left/right/up/down (default h/j/k/l) so a Vim-bound user doesn't have to reach for arrows.
/// `action` is what `lookup` made of the key.
pub(crate) fn toggles_binary_choice(key: &KeyEvent, action: Option<Action>) -> bool {
    matches!(key.code, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down)
        || matches!(
            action,
            Some(Action::SubTabLeft | Action::SubTabRight | Action::Up | Action::Down)
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Keymap, Seq};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn toggles(key: &KeyEvent, km: &Keymap) -> bool {
        toggles_binary_choice(key, km.action(key))
    }

    #[test]
    fn arrows_and_keymap_bound_hjkl_toggle_but_other_keys_dont() {
        let km = Keymap::default();
        assert!(toggles(&key(KeyCode::Left), &km));
        assert!(toggles(&key(KeyCode::Right), &km));
        assert!(toggles(&key(KeyCode::Up), &km));
        assert!(toggles(&key(KeyCode::Down), &km));
        // Default keymap: SubTabLeft/SubTabRight/Up/Down are h/l/k/j.
        assert!(toggles(&key(KeyCode::Char('h')), &km));
        assert!(toggles(&key(KeyCode::Char('j')), &km));
        assert!(toggles(&key(KeyCode::Char('k')), &km));
        assert!(toggles(&key(KeyCode::Char('l')), &km));
        assert!(!toggles(&key(KeyCode::Char('x')), &km));
        assert!(!toggles(&key(KeyCode::Enter), &km));
    }

    #[test]
    fn rebound_direction_keys_are_honoured() {
        let mut km = Keymap::default();
        km.clear(Action::SubTabLeft);
        km.clear(Action::SubTabRight);
        km.clear(Action::AddToQueue);
        km.clear(Action::RemoveFromPlaylist);
        assert!(km.add(Action::SubTabLeft, Seq::parse("a").unwrap()).is_ok());
        assert!(km.add(Action::SubTabRight, Seq::parse("d").unwrap()).is_ok());
        assert!(toggles(&key(KeyCode::Char('a')), &km));
        assert!(toggles(&key(KeyCode::Char('d')), &km));
        // The old default no longer does anything for this action.
        assert!(!toggles(&key(KeyCode::Char('h')), &km));
    }

    #[test]
    fn popup_lists_move_with_the_keymap_in_every_overlay() {
        let mut state = AppState::default();
        let mut at = 20;
        let mut press = |code, mods| {
            let key = KeyEvent::new(code, mods);
            let action = lookup(&mut state, &key);
            if let Some(next) = list_step(&key, action, at, 40) {
                at = next;
            }
            (at, state.pending_keys.len())
        };
        let ch = |c| KeyCode::Char(c);
        assert_eq!(press(ch('j'), KeyModifiers::NONE), (21, 0));
        assert_eq!(press(ch('k'), KeyModifiers::NONE), (20, 0));
        assert_eq!(press(ch('d'), KeyModifiers::CONTROL), (25, 0));
        assert_eq!(press(ch('u'), KeyModifiers::CONTROL), (20, 0));
        assert_eq!(press(ch('f'), KeyModifiers::CONTROL), (30, 0));
        assert_eq!(press(ch('b'), KeyModifiers::CONTROL), (20, 0));
        assert_eq!(press(ch('G'), KeyModifiers::SHIFT), (40, 0));
        assert_eq!(press(ch('j'), KeyModifiers::NONE), (40, 0), "stops at the end");
        assert_eq!(press(ch('g'), KeyModifiers::NONE), (40, 1), "first g waits");
        assert_eq!(press(ch('g'), KeyModifiers::NONE), (0, 0));
        assert_eq!(press(KeyCode::End, KeyModifiers::NONE), (40, 0));
        assert_eq!(press(KeyCode::Home, KeyModifiers::NONE), (0, 0));
        assert_eq!(press(ch('x'), KeyModifiers::NONE), (0, 0), "not a movement");

        // The arrows move even with nothing bound to them.
        let mut state = AppState::default();
        state.keymap.clear(Action::Down);
        let down = key(KeyCode::Down);
        let action = lookup(&mut state, &down);
        assert_eq!((action, list_step(&down, action, 3, 9)), (None, Some(4)));
    }

    #[test]
    fn a_half_typed_sequence_lapses_after_a_second() {
        let mut state = AppState::default();
        let g = key(KeyCode::Char('g'));
        assert_eq!(lookup(&mut state, &g), None);
        assert!(state.pending_since.is_some() && state.pending_keys.len() == 1);
        // Typed in time it completes; typed too late it starts again.
        state.pending_since = Instant::now().checked_sub(SEQ_TIMEOUT);
        assert_eq!(lookup(&mut state, &g), None, "the first g had lapsed");
        assert_eq!(state.pending_keys.len(), 1);
        assert_eq!(lookup(&mut state, &g), Some(Action::Top));
        assert!(state.pending_keys.is_empty() && state.pending_since.is_none());
        lookup(&mut state, &g);
        clear_pending(&mut state);
        assert!(state.pending_keys.is_empty() && state.pending_since.is_none());
    }
}
