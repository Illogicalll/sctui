use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::api::Track;
use crate::keymap::{Action, Keymap};
use crate::tui::logic::state::{AppState, FollowingTracksFocus, PlaybackSource, QueuedTrack};

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

/// Maps a visible Library row to its index in the unfiltered list while the search filter is active.
pub(crate) fn filtered_row(state: &AppState, row: usize) -> Option<usize> {
    if state.search_popup_visible && !state.search_query.trim().is_empty() {
        state.search_matches.get(row).copied()
    } else {
        Some(row)
    }
}

pub(crate) fn reset_search_rows(state: &mut AppState) {
    state.selected_row = 0;
    state.search_selected_playlist_track_row = 0;
    state.search_selected_album_track_row = 0;
    state.search_selected_person_track_row = 0;
    state.search_selected_person_like_row = 0;
    state.search_people_tracks_focus = FollowingTracksFocus::Published;
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
    use ratatui::crossterm::event::KeyModifiers;

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
