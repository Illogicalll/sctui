use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
};

use crate::tui::render::utils::{
    calculate_column_widths, calculate_min_widths, styled_header, truncate_with_ellipsis,
};

const NUM_SEARCHFILTERS: usize = 4;

pub fn render_search(
    frame: &mut Frame,
    area: Rect,
    width: usize,
    query: &str,
    searchfilters: &[&str],
    selected_searchfilter: usize,
    selected_row: usize,
) {
    let subchunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(area);

    let input = Paragraph::new(query.to_string())
        .block(
            Block::default()
                .title("search")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded),
        )
        .alignment(Alignment::Center);
    frame.render_widget(input, subchunks[0]);

    let (header, num_columns) = match selected_searchfilter {
        0 => (
            styled_header(&["Title", "Artist(s)", "Album", "Duration"]),
            4,
        ),
        1 => (
            styled_header(&["Title", "Artist(s)", "Year", "Duration"]),
            4,
        ),
        2 => (styled_header(&["Name", "No. Songs", "Duration"]), 3),
        3 => (styled_header(&["Name"]), 1),
        _ => (Row::new(vec![] as Vec<Cell>), 0),
    };

    let col_widths = calculate_column_widths(num_columns);
    let col_min_widths = calculate_min_widths(&col_widths, width);

    let rows = match selected_searchfilter {
        0 => vec![
            Row::new(vec![
                truncate_with_ellipsis("Short song name", col_min_widths[0]),
                truncate_with_ellipsis("Short artist name", col_min_widths[1]),
                truncate_with_ellipsis("Short album name", col_min_widths[2]),
                truncate_with_ellipsis("0:57", col_min_widths[3]),
            ]),
            Row::new(vec![
                truncate_with_ellipsis("Medium length song name", col_min_widths[0]),
                truncate_with_ellipsis("Medium length artist name", col_min_widths[1]),
                truncate_with_ellipsis("Medium length album name", col_min_widths[2]),
                truncate_with_ellipsis("12:54", col_min_widths[3]),
            ]),
            Row::new(vec![
                truncate_with_ellipsis(
                    "Really really really long song name",
                    col_min_widths[0],
                ),
                truncate_with_ellipsis(
                    "Really really really long artist name",
                    col_min_widths[1],
                ),
                truncate_with_ellipsis(
                    "Really really really long album name",
                    col_min_widths[2],
                ),
                truncate_with_ellipsis("12:59:30", col_min_widths[3]),
            ]),
        ],
        1 => vec![
            Row::new(vec![
                truncate_with_ellipsis("Album One", col_min_widths[0]),
                truncate_with_ellipsis("Artist X", col_min_widths[1]),
                truncate_with_ellipsis("1997", col_min_widths[2]),
                truncate_with_ellipsis("45:02", col_min_widths[3]),
            ]),
            Row::new(vec![
                truncate_with_ellipsis("Album Two", col_min_widths[0]),
                truncate_with_ellipsis("Artist Y", col_min_widths[1]),
                truncate_with_ellipsis("2009", col_min_widths[2]),
                truncate_with_ellipsis("16:03", col_min_widths[3]),
            ]),
        ],
        2 => vec![
            Row::new(vec![
                truncate_with_ellipsis("Playlist 1", col_min_widths[0]),
                truncate_with_ellipsis("15", col_min_widths[1]),
                truncate_with_ellipsis("30:00", col_min_widths[2]),
            ]),
            Row::new(vec![
                truncate_with_ellipsis("Playlist 2", col_min_widths[0]),
                truncate_with_ellipsis("1", col_min_widths[1]),
                truncate_with_ellipsis("2:30", col_min_widths[2]),
            ]),
        ],
        3 => vec![
            Row::new(vec![truncate_with_ellipsis(
                "Following Artist A",
                col_min_widths[0],
            )]),
            Row::new(vec![truncate_with_ellipsis(
                "Following Artist B",
                col_min_widths[0],
            )]),
        ],
        _ => vec![],
    };

    let rows: Vec<_> = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            if i == selected_row {
                row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
            } else {
                row
            }
        })
        .collect();

    let table = Table::new(rows, col_widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded),
        )
        .column_spacing(1);
    frame.render_widget(table, subchunks[1]);

    let tab_width = width / NUM_SEARCHFILTERS;
    fn center_text_in_width(text: &str, width: usize) -> String {
        let total_padding = width - text.chars().count();
        let padding = (total_padding / 2) - 1;
        format!("{}{}{}", " ".repeat(padding), text, " ".repeat(padding))
    }

    let searchfilter: Vec<Span<'static>> = searchfilters
        .iter()
        .map(|filter| Span::raw(center_text_in_width(filter, tab_width)))
        .collect();
    let searchfilter_widget = Tabs::new(searchfilter)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("filter")
                .title_alignment(Alignment::Center)
                .border_type(ratatui::widgets::BorderType::Rounded),
        )
        .select(selected_searchfilter)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(searchfilter_widget, subchunks[2]);
}
