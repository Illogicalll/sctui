use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use super::helpers::toggles_binary_choice;
use crate::tui::logic::state::{AppData, AppState, ConfirmAction, PlaylistEdit, clamp_row};
use crate::tui::logic::utils::{bump_track_count, soundcloud_playlist_id_from_tracks_uri};

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
        _ if toggles_binary_choice(&key, &state.keymap) => {
            state.confirm_selected = if state.confirm_selected == 0 { 1 } else { 0 };
        }
        _ => {}
    }
    InputOutcome::Continue
}

/// Apply the action to the UI at once and queue the write. The playlist is looked
/// up now, by identity; if it has gone in the meantime nothing happens.
fn perform(action: ConfirmAction, state: &mut AppState, data: &mut AppData) {
    let (ConfirmAction::RemoveTrack { tracks_uri, .. } | ConfirmAction::DeletePlaylist { tracks_uri }) =
        &action;
    let Some(playlist_idx) = data.playlists.iter().position(|p| p.tracks_uri == *tracks_uri) else {
        return;
    };
    match action {
        ConfirmAction::RemoveTrack { track, .. } => {
            let playlist = &mut data.playlists[playlist_idx];
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
        ConfirmAction::DeletePlaylist { .. } => {
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
    use crate::api::Playlist;

    fn playlist(id: u64) -> Playlist {
        Playlist {
            title: format!("playlist {id}"),
            track_count: "0".to_string(),
            duration: String::new(),
            created_at: Default::default(),
            tracks_uri: format!("https://api.soundcloud.com/playlists/{id}/tracks"),
            is_owned: true,
        }
    }

    #[test]
    fn confirm_targets_playlist_by_identity_after_insert() {
        let mut state = AppState::default();
        let mut data = AppData::default();
        data.playlists = vec![playlist(1), playlist(2)];
        let b = data.playlists[1].tracks_uri.clone();
        state.confirm = Some(ConfirmAction::DeletePlaylist { tracks_uri: b.clone() });
        state.confirm_selected = 0;

        // What Msg::PlaylistCreated does while the popup is open.
        data.playlists.insert(0, playlist(3));
        let confirm = state.confirm.clone().unwrap();
        assert!(confirm.message(&data).contains("playlist 2"), "the question still names B");

        handle_confirm_input(KeyEvent::from(KeyCode::Enter), &mut state, &mut data);
        let left: Vec<&str> = data.playlists.iter().map(|p| p.title.as_str()).collect();
        assert_eq!(left, ["playlist 3", "playlist 1"]);
        assert!(matches!(
            state.playlist_edit_queue.pop_front(),
            Some(PlaylistEdit::Delete { playlist_id: 2 })
        ));

        // Gone by the time the user answers: nothing happens.
        perform(ConfirmAction::DeletePlaylist { tracks_uri: b }, &mut state, &mut data);
        assert_eq!(data.playlists.len(), 2);
        assert!(state.playlist_edit_queue.is_empty());
    }
}
