use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::tui::render::utils::{
    calculate_column_widths, calculate_min_widths, styled_header, truncate_with_ellipsis,
};

const NUM_FEED_ACTIVITY_COLS: usize = 4;
const NUM_FEED_INFO_COLS: usize = 3;

pub fn render_feed(
    frame: &mut Frame,
    area: Rect,
    width: usize,
    selected_row: usize,
    selected_info_row: usize,
    info_pane_selected: bool,
) {
    let subchunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
        .split(area);

    let activity_header = styled_header(&["User", "Action", "Media Type", "Age"]);

    let activity_col_widths = calculate_column_widths(NUM_FEED_ACTIVITY_COLS);
    let activity_col_min_widths = calculate_min_widths(&activity_col_widths, width / 2);

    let activity_rows = vec![
        vec![
            truncate_with_ellipsis("User 1", activity_col_min_widths[0]),
            truncate_with_ellipsis("Post", activity_col_min_widths[1]),
            truncate_with_ellipsis("Track", activity_col_min_widths[2]),
            truncate_with_ellipsis("2d", activity_col_min_widths[3]),
        ],
        vec![
            truncate_with_ellipsis("User 2", activity_col_min_widths[0]),
            truncate_with_ellipsis("Repost", activity_col_min_widths[1]),
            truncate_with_ellipsis("Album", activity_col_min_widths[2]),
            truncate_with_ellipsis("5d", activity_col_min_widths[3]),
        ],
    ]
    .into_iter()
    .enumerate()
    .map(|(i, cols)| {
        let row = Row::new(cols);
        if i == selected_row && info_pane_selected {
            row.style(Style::default().bg(Color::Gray).fg(Color::Black))
        } else if i == selected_row && !info_pane_selected {
            row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
        } else {
            row
        }
    })
    .collect::<Vec<_>>();

    let table = Table::new(activity_rows, activity_col_widths)
        .header(activity_header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("activity")
                .title_alignment(ratatui::layout::Alignment::Center)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(if info_pane_selected {
                    Style::default()
                } else {
                    Style::default().fg(Color::Cyan)
                }),
        )
        .column_spacing(1);
    frame.render_widget(table, subchunks[0]);

    let info_header = styled_header(&["Title", "Artist", "Dur."]);

    let info_col_widths = calculate_column_widths(NUM_FEED_INFO_COLS);
    let info_col_min_widths = calculate_min_widths(&info_col_widths, width / 2);

    let info_rows = vec![
        vec![
            truncate_with_ellipsis("Track 1", info_col_min_widths[0]),
            truncate_with_ellipsis("Artist 1", info_col_min_widths[1]),
            truncate_with_ellipsis("1:30", info_col_min_widths[2]),
        ],
        vec![
            truncate_with_ellipsis("Track 2", info_col_min_widths[0]),
            truncate_with_ellipsis("Artist 1", info_col_min_widths[1]),
            truncate_with_ellipsis("2:10", info_col_min_widths[2]),
        ],
    ]
    .into_iter()
    .enumerate()
    .map(|(i, cols)| {
        let row = Row::new(cols);
        if i == selected_info_row && info_pane_selected {
            row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
        } else if i == selected_info_row && !info_pane_selected {
            row.style(Style::default().bg(Color::Gray).fg(Color::Black))
        } else {
            row
        }
    })
    .collect::<Vec<_>>();

    let table = Table::new(info_rows, info_col_widths)
        .header(info_header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("info")
                .title_alignment(ratatui::layout::Alignment::Center)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(if info_pane_selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }),
        )
        .column_spacing(1);

    frame.render_widget(table, subchunks[1]);
}
