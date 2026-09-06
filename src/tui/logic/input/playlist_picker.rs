use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use crate::tui::logic::state::{AppData, AppState, PlaylistEdit};
use crate::tui::logic::utils::{bump_track_count, soundcloud_playlist_id_from_tracks_uri};

/// Indices into `data.playlists` of the playlists the user owns, in display order.
pub(crate) fn owned_playlists(data: &AppData) -> Vec<usize> {
    data.playlists
        .iter()
        .enumerate()
        .filter(|(_, p)| p.is_owned)
        .map(|(i, _)| i)
        .collect()
}

pub(crate) fn open_picker(state: &mut AppState, track: crate::api::Track) {
    state.playlist_picker_track = Some(track);
    state.playlist_picker_selected = 0;
    state.playlist_picker_title = None;
    state.playlist_picker_visible = true;
}

fn close_picker(state: &mut AppState) {
    state.playlist_picker_visible = false;
    state.playlist_picker_title = None;
    state.playlist_picker_track = None;
}

/// Keys the "add to playlist" popup owns. `None` lets the key fall through.
pub(crate) fn handle_picker_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
) -> Option<InputOutcome> {
    // Typing a name for a new playlist: the popup owns every key.
    if let Some(title) = state.playlist_picker_title.as_mut() {
        match key.code {
            KeyCode::Esc => state.playlist_picker_title = None,
            KeyCode::Backspace => {
                title.pop();
            }
            KeyCode::Char(c) => title.push(c),
            KeyCode::Enter => {
                let title = title.trim().to_string();
                if title.is_empty() {
                    return Some(InputOutcome::Continue);
                }
                if let Some(track) = state.playlist_picker_track.take() {
                    state.playlist_edit_queue.push_back(PlaylistEdit::Create {
                        title,
                        track_urn: track.track_urn,
                    });
                }
                close_picker(state);
            }
            _ => {}
        }
        return Some(InputOutcome::Continue);
    }

    let owned = owned_playlists(data);
    let last_row = owned.len(); // row 0 is "+ New playlist…"
    match key.code {
        KeyCode::Esc => close_picker(state),
        KeyCode::Up => state.playlist_picker_selected = state.playlist_picker_selected.saturating_sub(1),
        KeyCode::Down => state.playlist_picker_selected = (state.playlist_picker_selected + 1).min(last_row),
        KeyCode::Enter => {
            if state.playlist_picker_selected == 0 {
                state.playlist_picker_title = Some(String::new());
            } else if let Some(&playlist_idx) = owned.get(state.playlist_picker_selected - 1) {
                add_to_playlist(playlist_idx, state, data);
                close_picker(state);
            }
        }
        _ => return None,
    }
    Some(InputOutcome::Continue)
}

/// Optimistically append the picked track to `data.playlists[playlist_idx]`
/// and queue the write.
fn add_to_playlist(playlist_idx: usize, state: &mut AppState, data: &mut AppData) {
    let Some(track) = state.playlist_picker_track.clone() else { return };
    let Some(playlist) = data.playlists.get_mut(playlist_idx) else { return };
    let Some(playlist_id) = soundcloud_playlist_id_from_tracks_uri(&playlist.tracks_uri) else {
        return;
    };

    if data.playlist_tracks_uri.as_deref() == Some(playlist.tracks_uri.as_str())
        && !data.playlist_tracks.iter().any(|t| t.track_urn == track.track_urn)
    {
        data.playlist_tracks.push(track.clone());
    }
    bump_track_count(playlist, 1);
    state.playlist_edit_queue.push_back(PlaylistEdit::Add {
        playlist_id,
        tracks_uri: playlist.tracks_uri.clone(),
        track_urn: track.track_urn,
    });
}
