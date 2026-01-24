mod help;
mod queue;
mod quit;

use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
};

use crate::api::Track;

pub fn render_overlays(
    frame: &mut Frame,
    likes_all: &Vec<Track>,
    manual_queue: &VecDeque<usize>,
    auto_queue: &VecDeque<usize>,
    current_playing_index: Option<usize>,
    previous_playing_index: Option<usize>,
    queue_visible: bool,
    help_visible: bool,
    quit_confirm_visible: bool,
    quit_confirm_selected: usize,
) {
    if queue_visible {
        queue::render_queue(
            frame,
            likes_all,
            manual_queue,
            auto_queue,
            current_playing_index,
            previous_playing_index,
        );
    }

    if help_visible {
        help::render_help(frame);
    }

    if quit_confirm_visible {
        quit::render_quit_confirm(frame, quit_confirm_selected);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
