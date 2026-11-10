use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::Track;
use crate::keymap::{Action, Keymap};
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

/// Arrow keys always flip a two-option Yes/No choice, plus whatever the user has bound to
/// left/right/up/down (default h/j/k/l) so a Vim-bound user doesn't have to reach for arrows.
pub(crate) fn toggles_binary_choice(key: &KeyEvent, keymap: &Keymap) -> bool {
    matches!(key.code, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down)
        || matches!(
            keymap.action(key),
            Some(Action::SubTabLeft | Action::SubTabRight | Action::Up | Action::Down)
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn arrows_and_keymap_bound_hjkl_toggle_but_other_keys_dont() {
        let km = Keymap::default();
        assert!(toggles_binary_choice(&key(KeyCode::Left), &km));
        assert!(toggles_binary_choice(&key(KeyCode::Right), &km));
        assert!(toggles_binary_choice(&key(KeyCode::Up), &km));
        assert!(toggles_binary_choice(&key(KeyCode::Down), &km));
        // Default keymap: SubTabLeft/SubTabRight/Up/Down are h/l/k/j.
        assert!(toggles_binary_choice(&key(KeyCode::Char('h')), &km));
        assert!(toggles_binary_choice(&key(KeyCode::Char('j')), &km));
        assert!(toggles_binary_choice(&key(KeyCode::Char('k')), &km));
        assert!(toggles_binary_choice(&key(KeyCode::Char('l')), &km));
        assert!(!toggles_binary_choice(&key(KeyCode::Char('x')), &km));
        assert!(!toggles_binary_choice(&key(KeyCode::Enter), &km));
    }

    #[test]
    fn rebound_direction_keys_are_honoured() {
        let mut km = Keymap::default();
        km.clear(Action::SubTabLeft);
        km.clear(Action::SubTabRight);
        assert!(km.add_chord(Action::SubTabLeft, crate::keymap::Chord::parse("a").unwrap()).is_ok());
        assert!(km.add_chord(Action::SubTabRight, crate::keymap::Chord::parse("d").unwrap()).is_ok());
        assert!(toggles_binary_choice(&key(KeyCode::Char('a')), &km));
        assert!(toggles_binary_choice(&key(KeyCode::Char('d')), &km));
        // The old default no longer does anything for this action.
        assert!(!toggles_binary_choice(&key(KeyCode::Char('h')), &km));
    }
}
