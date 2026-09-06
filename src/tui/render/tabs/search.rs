use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Tabs},
};

use crate::tui::logic::state::{AppData, AppState, FollowingTracksFocus, SEARCHFILTERS};

use crate::tui::render::utils::{calculate_min_widths, styled_header, truncate_with_ellipsis};

use super::library::{
    short_track_cells, track_cells, track_table, ALBUM_TRACK_WIDTHS, PUBLISHED_TRACK_WIDTHS,
    SHORT_TRACK_HEADER, TRACK_HEADER, TRACK_WIDTHS,
};

const NUM_SEARCHFILTERS: usize = 4;

pub fn render_search(
    frame: &mut Frame,
    area: Rect,
    width: usize,
    state: &AppState,
    data: &mut AppData,
) {
    let selected_searchfilter = state.selected_searchfilter;
    let selected_row = state.selected_row;
    let people_focus_is_likes = state.search_people_tracks_focus == FollowingTracksFocus::Likes;
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

    let (input_text, input_style) = if state.search_typing {
        (
            format!("{}▏", state.query),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )
    } else if state.query.is_empty() {
        ("press / to search".to_string(), Style::default().fg(Color::DarkGray))
    } else {
        (state.query.clone(), Style::default().fg(Color::White))
    };
    let input = Paragraph::new(input_text)
        .style(input_style)
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
        track_table(
            frame,
            table_area,
            &mut data.search_tracks_state,
            None,
            &["♥", "Title", "Artist(s)", "Duration", "Streams"],
            &[
                Constraint::Length(1),
                Constraint::Percentage(53),
                Constraint::Percentage(26),
                Constraint::Percentage(11),
                Constraint::Percentage(10),
            ],
            &data.search_tracks,
            |t| {
                let liked = if data.liked_track_urns.contains(&t.track_urn) {
                    "♥"
                } else {
                    ""
                };
                vec![
                    liked,
                    t.title.as_str(),
                    t.artists.as_str(),
                    t.duration.as_str(),
                    t.playback_count.as_str(),
                ]
            },
            selected_row,
            true,
        );
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

        let left_rows = data.search_playlists
            .iter()
            .enumerate()
            .map(|(i, playlist)| {
                let liked = if data.liked_playlist_uris.contains(&playlist.tracks_uri) {
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
        frame.render_stateful_widget(left_table, columns[0], &mut data.search_playlists_state);

        track_table(
            frame,
            columns[1],
            &mut data.search_playlist_tracks_state,
            None,
            TRACK_HEADER,
            &TRACK_WIDTHS,
            &data.search_playlist_tracks,
            track_cells,
            state.search_selected_playlist_track_row,
            true,
        );
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

        let left_rows = data.search_albums
            .iter()
            .enumerate()
            .map(|(i, album)| {
                let liked = if data.liked_album_uris.contains(&album.tracks_uri) {
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
        frame.render_stateful_widget(left_table, columns[0], &mut data.search_albums_state);

        track_table(
            frame,
            columns[1],
            &mut data.search_album_tracks_state,
            None,
            SHORT_TRACK_HEADER,
            &ALBUM_TRACK_WIDTHS,
            &data.search_album_tracks,
            short_track_cells,
            state.search_selected_album_track_row,
            true,
        );
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

        let left_rows = data.search_people
            .iter()
            .enumerate()
            .map(|(i, artist)| {
                let liked = if data.followed_user_urns.contains(&artist.urn) {
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
        frame.render_stateful_widget(left_table, columns[0], &mut data.search_people_state);

        track_table(
            frame,
            columns[1],
            &mut data.search_people_tracks_state,
            Some("tracks"),
            SHORT_TRACK_HEADER,
            &PUBLISHED_TRACK_WIDTHS,
            &data.search_people_tracks,
            short_track_cells,
            state.search_selected_person_track_row,
            !people_focus_is_likes,
        );

        track_table(
            frame,
            columns[2],
            &mut data.search_people_likes_state,
            Some("liked"),
            TRACK_HEADER,
            &TRACK_WIDTHS,
            &data.search_people_likes_tracks,
            track_cells,
            state.search_selected_person_like_row,
            people_focus_is_likes,
        );
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

    let searchfilter: Vec<Span<'static>> = SEARCHFILTERS
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
