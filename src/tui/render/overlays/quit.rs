use ratatui::Frame;

use super::confirm::render_confirm;

pub fn render_quit_confirm(frame: &mut Frame, quit_confirm_selected: usize) {
    render_confirm(frame, "Are you Sure you Want to Quit?", quit_confirm_selected);
}
