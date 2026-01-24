use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
};

use crate::api::{Album, Artist, Playlist, Track};

use crate::tui::render::utils::{calculate_min_widths, styled_header, truncate_with_ellipsis};

pub fn render_library(
    frame: &mut Frame,
    area: Rect,
    width: usize,
    likes_view: &Vec<Track>,
    likes_state: &mut TableState,
    playlists: &Vec<Playlist>,
    playlists_state: &mut TableState,
    albums: &Vec<Album>,
    albums_state: &mut TableState,
    following: &Vec<Artist>,
    following_state: &mut TableState,
    selected_subtab: usize,
    subtab_titles: &[&str],
    selected_row: usize,
    search_popup_visible: bool,
    search_query: &str,
) {
    let subchunks = if search_popup_visible {
        Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(area)
    };

    let subtabs: Vec<_> = subtab_titles.iter().map(|t| Span::raw(*t)).collect();
    let subtabs_widget = Tabs::new(subtabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(selected_subtab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(subtabs_widget, subchunks[0]);

    if search_popup_visible {
        let input = Paragraph::new(search_query.to_string())
            .block(
                Block::default()
                    .title("search")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Center);
        frame.render_widget(input, subchunks[1]);
    }

    let (header, col_widths) = match selected_subtab {
        0 => (
            styled_header(&["Title", "Artist(s)", "Duration", "Streams"]),
            vec![
                Constraint::Percentage(55),
                Constraint::Percentage(25),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
            ],
        ),
        1 => (
            styled_header(&["Name", "No. Songs", "Duration"]),
            vec![
                Constraint::Percentage(80),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
            ],
        ),
        2 => (
            styled_header(&["Title", "Artist(s)", "Year", "No. Songs", "Duration"]),
            vec![
                Constraint::Percentage(50),
                Constraint::Percentage(20),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
            ],
        ),
        3 => (styled_header(&["Name"]), vec![Constraint::Percentage(100)]),
        _ => (
            Row::new(vec![] as Vec<Cell>),
            vec![Constraint::Percentage(100)],
        ),
    };

    let col_min_widths = calculate_min_widths(&col_widths, width);

    let rows = match selected_subtab {
        0 => likes_view
            .iter()
            .map(|track| {
                Row::new(vec![
                    truncate_with_ellipsis(&track.title, col_min_widths[0]),
                    truncate_with_ellipsis(&track.artists, col_min_widths[1]),
                    truncate_with_ellipsis(&track.duration, col_min_widths[2]),
                    truncate_with_ellipsis(&track.playback_count, col_min_widths[3]),
                ])
            })
            .collect(),
        1 => playlists
            .iter()
            .map(|playlist| {
                Row::new(vec![
                    truncate_with_ellipsis(&playlist.title, col_min_widths[0]),
                    truncate_with_ellipsis(&playlist.track_count, col_min_widths[1]),
                    truncate_with_ellipsis(&playlist.duration, col_min_widths[2]),
                ])
            })
            .collect(),
        2 => albums
            .iter()
            .map(|album| {
                Row::new(vec![
                    truncate_with_ellipsis(&album.title, col_min_widths[0]),
                    truncate_with_ellipsis(&album.artists, col_min_widths[1]),
                    truncate_with_ellipsis(&album.release_year, col_min_widths[2]),
                    truncate_with_ellipsis(&album.track_count, col_min_widths[3]),
                    truncate_with_ellipsis(&album.duration, col_min_widths[4]),
                ])
            })
            .collect(),
        3 => following
            .iter()
            .map(|artist| {
                Row::new(vec![truncate_with_ellipsis(
                    &artist.name,
                    col_min_widths[0],
                )])
            })
            .collect(),
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

    let state: &mut TableState = match selected_subtab {
        0 => likes_state,
        1 => playlists_state,
        2 => albums_state,
        3 => following_state,
        _ => likes_state,
    };

    let table_chunk_idx = if search_popup_visible { 2 } else { 1 };
    let table = Table::new(rows, col_widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .column_spacing(1);
    frame.render_stateful_widget(table, subchunks[table_chunk_idx], state);
}
