use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Row, Table},
};

use crate::tui::logic::state::{AppData, AppState};
use crate::tui::render::tabs::library::{TRACK_HEADER, TRACK_WIDTHS, track_cells, track_table};
use crate::tui::render::utils::{calculate_min_widths, styled_header, truncate_with_ellipsis};

const ACTIVITY_WIDTHS: [Constraint; 4] = [
    Constraint::Percentage(40),
    Constraint::Percentage(20),
    Constraint::Percentage(25),
    Constraint::Percentage(15),
];

/// Left: posts and reposts from followed users. Right: the tracks of the highlighted item.
pub fn render_feed(frame: &mut Frame, area: Rect, state: &AppState, data: &mut AppData) {
    let panes = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
        .split(area);

    let col_widths = ACTIVITY_WIDTHS;
    let min_widths = calculate_min_widths(&col_widths, panes[0].width as usize);
    let rows = data.feed.iter().enumerate().map(|(i, activity)| {
        let row = Row::new(vec![
            truncate_with_ellipsis(&activity.user, min_widths[0]),
            activity.action.to_string(),
            activity.media.to_string(),
            activity.age(),
        ]);
        if i != state.selected_row {
            row
        } else if state.info_pane_selected {
            row.style(Style::default().bg(Color::Gray).fg(Color::Black))
        } else {
            row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
        }
    });
    let table = Table::new(rows, col_widths)
        .header(styled_header(&["User", "Action", "Media Type", "Age"]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("activity")
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Rounded),
        )
        .column_spacing(1);
    frame.render_stateful_widget(table, panes[0], &mut data.feed_state);

    track_table(
        frame,
        panes[1],
        &mut data.feed_tracks_state,
        Some("info"),
        TRACK_HEADER,
        &TRACK_WIDTHS,
        &data.feed_tracks,
        track_cells,
        state.selected_info_row,
        state.info_pane_selected,
    );
}
