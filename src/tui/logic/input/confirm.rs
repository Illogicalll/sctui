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
