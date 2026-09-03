mod help;
mod queue;
mod quit;
mod utils;

use ratatui::Frame;

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

    if state.help_visible {
        help::render_help(frame);
    }

    if state.quit_confirm_visible {
        quit::render_quit_confirm(frame, state.quit_confirm_selected);
    }
}
