use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
};

use crate::api::{Album, Artist, Playlist, Track};

use crate::tui::render::utils::{calculate_min_widths, styled_header, truncate_with_ellipsis};

pub(super) const TRACK_HEADER: &[&str] = &["Title", "Artist(s)", "Duration", "Streams"];
pub(super) const TRACK_WIDTHS: [Constraint; 4] = [
    Constraint::Percentage(55),
    Constraint::Percentage(25),
    Constraint::Percentage(10),
    Constraint::Percentage(10),
];
pub(super) const SHORT_TRACK_HEADER: &[&str] = &["Title", "Duration", "Streams"];
pub(super) const ALBUM_TRACK_WIDTHS: [Constraint; 3] = [
    Constraint::Percentage(55),
    Constraint::Percentage(25),
    Constraint::Percentage(20),
];
pub(super) const PUBLISHED_TRACK_WIDTHS: [Constraint; 3] = [
    Constraint::Percentage(70),
    Constraint::Percentage(15),
    Constraint::Percentage(15),
];

pub fn render_library(
    frame: &mut Frame,
    area: Rect,
    width: usize,
    likes_view: &Vec<Track>,
    likes_state: &mut TableState,
    playlists: &Vec<Playlist>,
    playlists_state: &mut TableState,
    playlist_tracks: &Vec<Track>,
    playlist_tracks_state: &mut TableState,
    album_tracks: &Vec<Track>,
    album_tracks_state: &mut TableState,
    albums: &Vec<Album>,
    albums_state: &mut TableState,
    following: &Vec<Artist>,
    following_state: &mut TableState,
    following_tracks: &Vec<Track>,
    following_tracks_state: &mut TableState,
    following_likes_tracks: &Vec<Track>,
    following_likes_state: &mut TableState,
    selected_subtab: usize,
    subtab_titles: &[&str],
    selected_row: usize,
    selected_playlist_track_row: usize,
    selected_album_track_row: usize,
    selected_following_track_row: usize,
    selected_following_like_row: usize,
    following_focus_is_likes: bool,
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

    let table_chunk_idx = if search_popup_visible { 2 } else { 1 };
    let table_area = subchunks[table_chunk_idx];

    if selected_subtab == 0 {
        track_table(
            frame,
            table_area,
            likes_state,
            None,
            TRACK_HEADER,
            &TRACK_WIDTHS,
            likes_view,
            track_cells,
            selected_row,
            true,
        );
        return;
    }

    let (header, col_widths) = match selected_subtab {
        1 => (
            styled_header(&["Name", "No. Songs", "Duration"]),
            vec![
                Constraint::Percentage(70),
                Constraint::Percentage(15),
                Constraint::Percentage(15),
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
                row.style(Style::default().bg(Color::Gray).fg(Color::Black))
            } else {
                row
            }
        })
        .collect();

    if selected_subtab == 1 {
        let columns = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(33), Constraint::Percentage(67)].as_ref())
            .split(table_area);

        let left_table = Table::new(rows, col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(left_table, columns[0], playlists_state);

        track_table(
            frame,
            columns[1],
            playlist_tracks_state,
            None,
            TRACK_HEADER,
            &TRACK_WIDTHS,
            playlist_tracks,
            track_cells,
            selected_playlist_track_row,
            true,
        );
        return;
    }

    if selected_subtab == 2 {
        let columns = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)].as_ref())
            .split(table_area);

        let left_table = Table::new(rows, col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(left_table, columns[0], albums_state);

        track_table(
            frame,
            columns[1],
            album_tracks_state,
            None,
            SHORT_TRACK_HEADER,
            &ALBUM_TRACK_WIDTHS,
            album_tracks,
            short_track_cells,
            selected_album_track_row,
            true,
        );
        return;
    }

    if selected_subtab == 3 {
        let columns = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(40),
                Constraint::Percentage(40),
            ])
            .split(table_area);

        let left_table = Table::new(rows, col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(left_table, columns[0], following_state);

        track_table(
            frame,
            columns[1],
            following_tracks_state,
            Some("tracks"),
            SHORT_TRACK_HEADER,
            &PUBLISHED_TRACK_WIDTHS,
            following_tracks,
            short_track_cells,
            selected_following_track_row,
            !following_focus_is_likes,
        );

        track_table(
            frame,
            columns[2],
            following_likes_state,
            Some("liked"),
            TRACK_HEADER,
            &TRACK_WIDTHS,
            following_likes_tracks,
            track_cells,
            selected_following_like_row,
            following_focus_is_likes,
        );
        return;
    }

    let table = Table::new(rows, col_widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .column_spacing(1);
    frame.render_stateful_widget(table, table_area, likes_state);
}

pub(super) fn track_cells(t: &Track) -> Vec<&str> {
    vec![
        t.title.as_str(),
        t.artists.as_str(),
        t.duration.as_str(),
        t.playback_count.as_str(),
    ]
}

pub(super) fn short_track_cells(t: &Track) -> Vec<&str> {
    vec![t.title.as_str(), t.duration.as_str(), t.playback_count.as_str()]
}

pub(super) fn track_table(
    frame: &mut Frame,
    area: Rect,
    state: &mut TableState,
    title: Option<&str>,
    header: &[&str],
    col_widths: &[Constraint],
    tracks: &[Track],
    cells: impl Fn(&Track) -> Vec<&str>,
    selected: usize,
    focused: bool,
) {
    let min_widths = calculate_min_widths(col_widths, area.width as usize);
    let rows = tracks.iter().enumerate().map(|(i, track)| {
        let mut row = Row::new(
            cells(track)
                .into_iter()
                .zip(&min_widths)
                .map(|(cell, &w)| truncate_with_ellipsis(cell, w)),
        );
        if !track.is_playable() {
            row = row.style(Style::default().fg(Color::DarkGray));
        }
        if i == selected {
            row = if track.is_playable() {
                if focused {
                    row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
                } else {
                    row.style(Style::default().bg(Color::Gray).fg(Color::Black))
                }
            } else {
                row.style(Style::default().bg(Color::DarkGray).fg(Color::Gray))
            };
        }
        row
    });
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    if let Some(title) = title {
        block = block.title(title).title_alignment(Alignment::Center);
    }
    let table = Table::new(rows, col_widths.iter().copied())
        .header(styled_header(header))
        .block(block)
        .column_spacing(1);
    frame.render_stateful_widget(table, area, state);
}
