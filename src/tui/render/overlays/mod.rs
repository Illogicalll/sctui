mod help;
mod queue;
mod quit;
mod utils;

use std::collections::VecDeque;

use ratatui::Frame;

use crate::api::Track;
use crate::tui::logic::state::QueuedTrack;

pub fn render_overlays(
    frame: &mut Frame,
    queue_tracks: &Vec<Track>,
    manual_queue: &VecDeque<QueuedTrack>,
    auto_queue: &VecDeque<usize>,
    current_playing_track: Option<Track>,
    previous_playing_track: Option<Track>,
    queue_visible: bool,
    help_visible: bool,
    quit_confirm_visible: bool,
    quit_confirm_selected: usize,
) {
    if queue_visible {
        queue::render_queue(
            frame,
            queue_tracks,
            manual_queue,
            auto_queue,
            current_playing_track,
            previous_playing_track,
        );
    }

    if help_visible {
        help::render_help(frame);
    }

    if quit_confirm_visible {
        quit::render_quit_confirm(frame, quit_confirm_selected);
    }
}
