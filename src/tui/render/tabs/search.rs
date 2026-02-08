use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
};

use crate::api::{Album, Artist, Playlist, Track};
use std::collections::HashSet;

use crate::tui::render::utils::{calculate_min_widths, styled_header, truncate_with_ellipsis};

const NUM_SEARCHFILTERS: usize = 4;

pub fn render_search(
    frame: &mut Frame,
    area: Rect,
    width: usize,
    query: &str,
    searchfilters: &[&str],
    selected_searchfilter: usize,
    selected_row: usize,
    liked_track_urns: &HashSet<String>,
    liked_album_uris: &HashSet<String>,
    liked_playlist_uris: &HashSet<String>,
    followed_user_urns: &HashSet<String>,
    search_tracks: &Vec<Track>,
    search_tracks_state: &mut TableState,
    search_playlists: &Vec<Playlist>,
    search_playlists_state: &mut TableState,
    search_playlist_tracks: &Vec<Track>,
    search_playlist_tracks_state: &mut TableState,
    search_albums: &Vec<Album>,
    search_albums_state: &mut TableState,
    search_album_tracks: &Vec<Track>,
    search_album_tracks_state: &mut TableState,
    search_people: &Vec<Artist>,
    search_people_state: &mut TableState,
    search_people_tracks: &Vec<Track>,
    search_people_tracks_state: &mut TableState,
    search_people_likes_tracks: &Vec<Track>,
    search_people_likes_state: &mut TableState,
    selected_playlist_track_row: usize,
    selected_album_track_row: usize,
    selected_person_track_row: usize,
    selected_person_like_row: usize,
    people_focus_is_likes: bool,
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
                .border_type(BorderType::Rounded),
        )
        .alignment(Alignment::Center);
    frame.render_widget(input, subchunks[0]);

    let table_area = subchunks[1];

    if selected_searchfilter == 0 {
        let header = styled_header(&["♥", "Title", "Artist(s)", "Duration", "Streams"]);
        let col_widths = vec![
            Constraint::Length(1),
            Constraint::Percentage(53),
            Constraint::Percentage(26),
            Constraint::Percentage(11),
            Constraint::Percentage(10),
        ];
        let col_min_widths = calculate_min_widths(&col_widths, width);

        let selected_unplayable = search_tracks
            .get(selected_row)
            .map(|track| !track.is_playable())
            .unwrap_or(false);

        let rows = search_tracks
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let liked = if liked_track_urns.contains(&track.track_urn) {
                    "♥"
                } else {
                    ""
                };
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(liked, col_min_widths[0]),
                    truncate_with_ellipsis(&track.title, col_min_widths[1]),
                    truncate_with_ellipsis(&track.artists, col_min_widths[2]),
                    truncate_with_ellipsis(&track.duration, col_min_widths[3]),
                    truncate_with_ellipsis(&track.playback_count, col_min_widths[4]),
                ]);
                if !track.is_playable() {
                    row = row.style(Style::default().fg(Color::DarkGray));
                }
                if i == selected_row {
                    let style = if selected_unplayable {
                        Style::default().bg(Color::DarkGray).fg(Color::Gray)
                    } else {
                        Style::default().bg(Color::LightBlue).fg(Color::White)
                    };
                    row = row.style(style);
                }
                row
            })
            .collect::<Vec<_>>();

        let table = Table::new(rows, col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(table, table_area, search_tracks_state);
    } else if selected_searchfilter == 2 {
        let columns = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(33), Constraint::Percentage(67)].as_ref())
            .split(table_area);

        let header = styled_header(&["♥", "Name", "No. Songs", "Duration"]);
        let left_col_widths = vec![
            Constraint::Length(1),
            Constraint::Percentage(68),
            Constraint::Percentage(16),
            Constraint::Percentage(16),
        ];
        let left_min_widths = calculate_min_widths(&left_col_widths, columns[0].width as usize);

        let left_rows = search_playlists
            .iter()
            .enumerate()
            .map(|(i, playlist)| {
                let liked = if liked_playlist_uris.contains(&playlist.tracks_uri) {
                    "♥"
                } else {
                    ""
                };
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(liked, left_min_widths[0]),
                    truncate_with_ellipsis(&playlist.title, left_min_widths[1]),
                    truncate_with_ellipsis(&playlist.track_count, left_min_widths[2]),
                    truncate_with_ellipsis(&playlist.duration, left_min_widths[3]),
                ]);
                if i == selected_row {
                    row = row.style(Style::default().bg(Color::Gray).fg(Color::Black));
                }
                row
            })
            .collect::<Vec<_>>();

        let left_table = Table::new(left_rows, left_col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(left_table, columns[0], search_playlists_state);

        let track_header = styled_header(&["Title", "Artist(s)", "Duration", "Streams"]);
        let track_width = columns[1].width as usize;
        let track_col_widths = vec![
            Constraint::Percentage(55),
            Constraint::Percentage(25),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
        ];
        let track_min_widths = calculate_min_widths(&track_col_widths, track_width);
        let track_rows = search_playlist_tracks
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(&track.title, track_min_widths[0]),
                    truncate_with_ellipsis(&track.artists, track_min_widths[1]),
                    truncate_with_ellipsis(&track.duration, track_min_widths[2]),
                    truncate_with_ellipsis(&track.playback_count, track_min_widths[3]),
                ]);
                if !track.is_playable() {
                    row = row.style(Style::default().fg(Color::DarkGray));
                }
                if i == selected_playlist_track_row {
                    row = if track.is_playable() {
                        row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
                    } else {
                        row.style(Style::default().bg(Color::DarkGray).fg(Color::Gray))
                    };
                }
                row
            })
            .collect::<Vec<_>>();
        let right_table = Table::new(track_rows, track_col_widths)
            .header(track_header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(right_table, columns[1], search_playlist_tracks_state);
    } else if selected_searchfilter == 1 {
        let columns = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)].as_ref())
            .split(table_area);

        let header = styled_header(&["♥", "Title", "Artist(s)", "Year", "No. Songs", "Duration"]);
        let left_col_widths = vec![
            Constraint::Length(1),
            Constraint::Percentage(47),
            Constraint::Percentage(21),
            Constraint::Percentage(11),
            Constraint::Percentage(10),
            Constraint::Percentage(11),
        ];
        let left_min_widths = calculate_min_widths(&left_col_widths, columns[0].width as usize);

        let left_rows = search_albums
            .iter()
            .enumerate()
            .map(|(i, album)| {
                let liked = if liked_album_uris.contains(&album.tracks_uri) {
                    "♥"
                } else {
                    ""
                };
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(liked, left_min_widths[0]),
                    truncate_with_ellipsis(&album.title, left_min_widths[1]),
                    truncate_with_ellipsis(&album.artists, left_min_widths[2]),
                    truncate_with_ellipsis(&album.release_year, left_min_widths[3]),
                    truncate_with_ellipsis(&album.track_count, left_min_widths[4]),
                    truncate_with_ellipsis(&album.duration, left_min_widths[5]),
                ]);
                if i == selected_row {
                    row = row.style(Style::default().bg(Color::Gray).fg(Color::Black));
                }
                row
            })
            .collect::<Vec<_>>();

        let left_table = Table::new(left_rows, left_col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(left_table, columns[0], search_albums_state);

        let track_header = styled_header(&["Title", "Duration", "Streams"]);
        let track_width = columns[1].width as usize;
        let track_col_widths = vec![
            Constraint::Percentage(55),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
        ];
        let track_min_widths = calculate_min_widths(&track_col_widths, track_width);
        let track_rows = search_album_tracks
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(&track.title, track_min_widths[0]),
                    truncate_with_ellipsis(&track.duration, track_min_widths[1]),
                    truncate_with_ellipsis(&track.playback_count, track_min_widths[2]),
                ]);
                if !track.is_playable() {
                    row = row.style(Style::default().fg(Color::DarkGray));
                }
                if i == selected_album_track_row {
                    row = if track.is_playable() {
                        row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
                    } else {
                        row.style(Style::default().bg(Color::DarkGray).fg(Color::Gray))
                    };
                }
                row
            })
            .collect::<Vec<_>>();
        let right_table = Table::new(track_rows, track_col_widths)
            .header(track_header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(right_table, columns[1], search_album_tracks_state);
    } else if selected_searchfilter == 3 {
        let columns = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Percentage(40),
                Constraint::Percentage(40),
            ])
            .split(table_area);

        let header = styled_header(&["♥", "Name"]);
        let left_col_widths = vec![Constraint::Length(1), Constraint::Percentage(100)];
        let left_min_widths = calculate_min_widths(&left_col_widths, columns[0].width as usize);

        let left_rows = search_people
            .iter()
            .enumerate()
            .map(|(i, artist)| {
                let liked = if followed_user_urns.contains(&artist.urn) {
                    "♥"
                } else {
                    ""
                };
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(liked, left_min_widths[0]),
                    truncate_with_ellipsis(&artist.name, left_min_widths[1]),
                ]);
                if i == selected_row {
                    row = row.style(Style::default().bg(Color::Gray).fg(Color::Black));
                }
                row
            })
            .collect::<Vec<_>>();

        let left_table = Table::new(left_rows, left_col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(left_table, columns[0], search_people_state);

        let published_width = columns[1].width as usize;
        let published_header = styled_header(&["Title", "Duration", "Streams"]);
        let published_col_widths = vec![
            Constraint::Percentage(70),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
        ];
        let published_min_widths = calculate_min_widths(&published_col_widths, published_width);
        let published_rows = search_people_tracks
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(&track.title, published_min_widths[0]),
                    truncate_with_ellipsis(&track.duration, published_min_widths[1]),
                    truncate_with_ellipsis(&track.playback_count, published_min_widths[2]),
                ]);
                if !track.is_playable() {
                    row = row.style(Style::default().fg(Color::DarkGray));
                }
                if i == selected_person_track_row {
                    let focused = !people_focus_is_likes;
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
            })
            .collect::<Vec<_>>();
        let published_table = Table::new(published_rows, published_col_widths)
            .header(published_header)
            .block(
                Block::default()
                    .title("tracks")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(published_table, columns[1], search_people_tracks_state);

        let likes_width = columns[2].width as usize;
        let track_header = styled_header(&["Title", "Artist(s)", "Duration", "Streams"]);
        let likes_col_widths = vec![
            Constraint::Percentage(55),
            Constraint::Percentage(25),
            Constraint::Percentage(10),
            Constraint::Percentage(10),
        ];
        let likes_min_widths = calculate_min_widths(&likes_col_widths, likes_width);
        let likes_rows = search_people_likes_tracks
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(&track.title, likes_min_widths[0]),
                    truncate_with_ellipsis(&track.artists, likes_min_widths[1]),
                    truncate_with_ellipsis(&track.duration, likes_min_widths[2]),
                    truncate_with_ellipsis(&track.playback_count, likes_min_widths[3]),
                ]);
                if !track.is_playable() {
                    row = row.style(Style::default().fg(Color::DarkGray));
                }
                if i == selected_person_like_row {
                    let focused = people_focus_is_likes;
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
            })
            .collect::<Vec<_>>();
        let likes_table = Table::new(likes_rows, likes_col_widths)
            .header(track_header)
            .block(
                Block::default()
                    .title("liked")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_stateful_widget(likes_table, columns[2], search_people_likes_state);
    } else {
        let header = Row::new(vec![] as Vec<Cell>);
        let table = Table::new(Vec::<Row>::new(), vec![Constraint::Percentage(100)])
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_widget(table, table_area);
    }

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
