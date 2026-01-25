use ratatui::{
    Frame,
    layout::{Alignment, Constraint},
    widgets::{Block, Borders, Clear, Row, Table},
};

use crate::tui::render::utils::styled_header;

use super::utils::centered_rect;

pub fn render_help(frame: &mut Frame) {
    let popup_area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, popup_area);

    let mut rows: Vec<Row> = vec![
        Row::new(vec!["Esc", "Quit"]),
        Row::new(vec!["Tab", "Cycle main tabs"]),
        Row::new(vec!["Left/Right", "Change sub-tab"]),
        Row::new(vec!["Up/Down", "Move selector"]),
        Row::new(vec!["Space", "Play/Pause"]),
        Row::new(vec!["Enter", "Play selected track"]),
        Row::new(vec!["Shift + Right", "Skip song"]),
        Row::new(vec!["Shift + Left", "Go back a song"]),
        Row::new(vec!["Option + Right", "Fast forward 10s"]),
        Row::new(vec!["Option + Left", "Rewind 10s"]),
        Row::new(vec!["Option + Up/Down", "Move selector by 10"]),
        Row::new(vec!["Shift + Up/Down", "Move secondary selector"]),
        Row::new(vec!["Shift + J/K", "Move tertiary selector"]),
        Row::new(vec!["Shift + U", "Volume up"]),
        Row::new(vec!["Shift + D", "Volume down"]),
        Row::new(vec!["Shift + S", "Toggle shuffle queue"]),
        Row::new(vec!["Shift + R", "Toggle repeat same song"]),
        Row::new(vec!["Shift + A", "Add selected song to queue"]),
        Row::new(vec!["Shift + N", "Play next (add to front of queue)"]),
        Row::new(vec!["Shift + F", "Search current view (only works in library)"]),
        Row::new(vec!["Shift + Q", "Toggle queue popup"]),
        Row::new(vec!["Shift + H", "Toggle help popup"]),
    ];

    let max_rows = popup_area.height.saturating_sub(3) as usize;
    if rows.len() > max_rows {
        rows.truncate(max_rows);
    }

    let table = Table::new(
        rows,
        vec![Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .header(styled_header(&["Action", "Description"]))
    .block(
        Block::default()
            .title("Help")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded),
    )
    .column_spacing(1);

    frame.render_widget(table, popup_area);
}
