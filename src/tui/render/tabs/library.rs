use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
};
use crate::theme;

use crate::api::Track;
use crate::tui::logic::filtering::{FilteredViews, is_filter_active};
use crate::tui::logic::state::{AppData, AppState, FollowingTracksFocus, SUBTAB_TITLES};

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
    state: &AppState,
    data: &mut AppData,
    views: &FilteredViews,
) {
    let filter_active = is_filter_active(state);
    let selected_subtab = state.selected_subtab;
    let selected_row = state.selected_row;
    let search_popup_visible = state.search_popup_visible;
    let likes_view = if filter_active && selected_subtab == 0 {
        &views.likes
    } else {
        &data.likes
    };
    let playlist_tracks = if filter_active && selected_subtab == 1 {
        &views.playlist_tracks
    } else {
        &data.playlist_tracks
    };
    let albums = if filter_active && selected_subtab == 2 {
        &views.albums
    } else {
        &data.albums
    };
    let following = if filter_active && selected_subtab == 3 {
        &views.following
    } else {
        &data.following
    };
    let following_focus_is_likes = state.following_tracks_focus == FollowingTracksFocus::Likes;

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

    let subtabs: Vec<_> = SUBTAB_TITLES.iter().map(|t| Span::raw(*t)).collect();
    let subtabs_widget = Tabs::new(subtabs)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(selected_subtab)
        .style(Style::default().fg(theme::current().fg))
        .highlight_style(
            Style::default()
                .fg(theme::current().accent)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(subtabs_widget, subchunks[0]);

    if search_popup_visible {
        let input = Paragraph::new(state.search_query.to_string())
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
            &mut data.likes_state,
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

    let table_width = if selected_subtab == 1 {
        width * 33 / 100
    } else {
        width
    };
    let col_min_widths = calculate_min_widths(&col_widths, table_width);

    let rows = match selected_subtab {
        1 => data.playlists
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

    let artists_focused = selected_subtab == 3 && state.following_tracks_focus == FollowingTracksFocus::Artists;
    let rows: Vec<_> = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            if i == selected_row {
                if artists_focused {
                    row.style(Style::default().bg(theme::current().selection_bg).fg(theme::current().fg))
                } else {
                    row.style(Style::default().bg(theme::current().muted).fg(theme::current().selection_fg))
                }
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
        frame.render_stateful_widget(left_table, columns[0], &mut data.playlists_state);

        track_table(
            frame,
            columns[1],
            &mut data.playlist_tracks_state,
            None,
            TRACK_HEADER,
            &TRACK_WIDTHS,
            playlist_tracks,
            track_cells,
            state.selected_playlist_track_row,
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
        frame.render_stateful_widget(left_table, columns[0], &mut data.albums_state);

        track_table(
            frame,
            columns[1],
            &mut data.album_tracks_state,
            None,
            SHORT_TRACK_HEADER,
            &ALBUM_TRACK_WIDTHS,
            &data.album_tracks,
            short_track_cells,
            state.selected_album_track_row,
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
        frame.render_stateful_widget(left_table, columns[0], &mut data.following_state);

        track_table(
            frame,
            columns[1],
            &mut data.following_tracks_state,
            Some("tracks"),
            SHORT_TRACK_HEADER,
            &PUBLISHED_TRACK_WIDTHS,
            &data.following_tracks,
            short_track_cells,
            state.selected_following_track_row,
            !following_focus_is_likes,
        );

        track_table(
            frame,
            columns[2],
            &mut data.following_likes_state,
            Some("liked"),
            TRACK_HEADER,
            &TRACK_WIDTHS,
            &data.following_likes_tracks,
            track_cells,
            state.selected_following_like_row,
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
    frame.render_stateful_widget(table, table_area, &mut data.likes_state);
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
            row = row.style(Style::default().fg(theme::current().dim));
        }
        if i == selected {
            row = if track.is_playable() {
                if focused {
                    row.style(Style::default().bg(theme::current().selection_bg).fg(theme::current().fg))
                } else {
                    row.style(Style::default().bg(theme::current().muted).fg(theme::current().selection_fg))
                }
            } else {
                row.style(Style::default().bg(theme::current().dim).fg(theme::current().muted))
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
