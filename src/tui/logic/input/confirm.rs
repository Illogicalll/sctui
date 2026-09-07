use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::keymap::{Action, Keymap};
use crate::tui::logic::state::{AppData, AppState, ConfirmAction, PlaylistEdit, clamp_row};
use crate::tui::logic::utils::{bump_track_count, soundcloud_playlist_id_from_tracks_uri};

/// Arrow keys always toggle Yes/No, plus whatever the user has bound to left/right
/// (default h/l) so a Vim-bound user doesn't have to reach for arrows here.
fn toggles_confirm_selection(key: &KeyEvent, keymap: &Keymap) -> bool {
    matches!(key.code, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down)
        || matches!(keymap.action(key), Some(Action::SubTabLeft | Action::SubTabRight))
}

/// Yes / No popup for destructive playlist actions. Owns every key while open.
pub(crate) fn handle_confirm_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
) -> InputOutcome {
    match key.code {
        KeyCode::Esc => state.confirm = None,
        KeyCode::Enter => {
            let action = state.confirm.take();
            if state.confirm_selected == 0
                && let Some(action) = action
            {
                perform(action, state, data);
            }
        }
        _ if toggles_confirm_selection(&key, &state.keymap) => {
            state.confirm_selected = if state.confirm_selected == 0 { 1 } else { 0 };
        }
        _ => {}
    }
    InputOutcome::Continue
}

/// Apply the action to the UI at once and queue the write.
fn perform(action: ConfirmAction, state: &mut AppState, data: &mut AppData) {
    match action {
        ConfirmAction::RemoveTrack { playlist_idx, track } => {
            let Some(playlist) = data.playlists.get_mut(playlist_idx) else { return };
            let Some(playlist_id) = soundcloud_playlist_id_from_tracks_uri(&playlist.tracks_uri)
            else {
                return;
            };
            if data.playlist_tracks_uri.as_deref() == Some(playlist.tracks_uri.as_str()) {
                data.playlist_tracks.retain(|t| t.track_urn != track.track_urn);
                clamp_row(
                    &mut state.selected_playlist_track_row,
                    &mut data.playlist_tracks_state,
                    data.playlist_tracks.len(),
                );
            }
            bump_track_count(playlist, -1);
            state.playlist_edit_queue.push_back(PlaylistEdit::Remove {
                playlist_id,
                tracks_uri: playlist.tracks_uri.clone(),
                track_urn: track.track_urn,
            });
        }
        ConfirmAction::DeletePlaylist { playlist_idx } => {
            if playlist_idx >= data.playlists.len() {
                return;
            }
            let playlist = data.playlists.remove(playlist_idx);
            if data.playlist_tracks_uri.as_deref() == Some(playlist.tracks_uri.as_str()) {
                data.playlist_tracks.clear();
                data.playlist_tracks_uri = None;
            }
            clamp_row(&mut state.selected_row, &mut data.playlists_state, data.playlists.len());
            if let Some(playlist_id) = soundcloud_playlist_id_from_tracks_uri(&playlist.tracks_uri) {
                state.playlist_edit_queue.push_back(PlaylistEdit::Delete { playlist_id });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn arrows_and_keymap_bound_left_right_toggle_but_other_keys_dont() {
        let km = Keymap::default();
        assert!(toggles_confirm_selection(&key(KeyCode::Left), &km));
        assert!(toggles_confirm_selection(&key(KeyCode::Right), &km));
        assert!(toggles_confirm_selection(&key(KeyCode::Up), &km));
        assert!(toggles_confirm_selection(&key(KeyCode::Down), &km));
        // Default keymap: SubTabLeft/SubTabRight are h/l.
        assert!(toggles_confirm_selection(&key(KeyCode::Char('h')), &km));
        assert!(toggles_confirm_selection(&key(KeyCode::Char('l')), &km));
        assert!(!toggles_confirm_selection(&key(KeyCode::Char('j')), &km));
        assert!(!toggles_confirm_selection(&key(KeyCode::Enter), &km));
    }

    #[test]
    fn rebound_left_right_keys_are_honoured() {
        let mut km = Keymap::default();
        km.clear(Action::SubTabLeft);
        km.clear(Action::SubTabRight);
        assert!(km.add_chord(Action::SubTabLeft, crate::keymap::Chord::parse("a").unwrap()).is_ok());
        assert!(km.add_chord(Action::SubTabRight, crate::keymap::Chord::parse("d").unwrap()).is_ok());
        assert!(toggles_confirm_selection(&key(KeyCode::Char('a')), &km));
        assert!(toggles_confirm_selection(&key(KeyCode::Char('d')), &km));
        // The old default no longer does anything for this action.
        assert!(!toggles_confirm_selection(&key(KeyCode::Char('h')), &km));
    }
}
