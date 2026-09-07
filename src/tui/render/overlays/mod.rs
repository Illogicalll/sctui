mod confirm;
mod help;
mod history;
mod playlist_picker;
mod theme_picker;
mod queue;
mod quit;
mod utils;

use ratatui::Frame;

use crate::api::Playlist;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::utils::active_tracks;

pub fn render_overlays(frame: &mut Frame, state: &AppState, data: &AppData) {
    if state.queue_visible {
        let queue_tracks = active_tracks(state, data);
        let previous_playing_track = state
            .playback_history
            .last()
            .map(|queued| queued.track.clone());
        let current_playing_track = state
            .override_playing
            .as_ref()
            .map(|queued| queued.track.clone())
            .or_else(|| {
                state
                    .current_playing_index
                    .and_then(|idx| queue_tracks.get(idx).cloned())
            });
        queue::render_queue(
            frame,
            queue_tracks,
            &state.manual_queue,
            &state.auto_queue,
            current_playing_track,
            previous_playing_track,
        );
    }

    if state.history_visible {
        history::render_history(frame, &state.playback_history, state.history_selected);
    }

    if state.playlist_picker_visible {
        let owned: Vec<&Playlist> = data.playlists.iter().filter(|p| p.is_owned).collect();
        playlist_picker::render_playlist_picker(
            frame,
            state.playlist_picker_track.as_ref(),
            &owned,
            state.playlist_picker_selected,
            state.playlist_picker_title.as_deref(),
            state.playlist_picker_public,
        );
    }

    if let Some(action) = &state.confirm {
        confirm::render_confirm(frame, &action.message(data), state.confirm_selected);
    }

    if state.theme_picker_visible {
        theme_picker::render_theme_picker(frame, state.theme_picker_selected, &state.theme_name);
    }

    if state.help_visible {
        help::render_help(frame, state);
    }

    if state.quit_confirm_visible {
        quit::render_quit_confirm(frame, state.quit_confirm_selected);
    }
}
