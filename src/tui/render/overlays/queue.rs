use std::collections::VecDeque;
use crate::theme;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint},
    style::{Style},
    widgets::{Block, Borders, Clear, Row, Table},
};

use crate::api::Track;
use crate::tui::logic::state::QueuedTrack;
use crate::tui::render::utils::{styled_header, truncate_with_ellipsis};

use super::utils::centered_rect;

pub fn render_queue(
    frame: &mut Frame,
    queue_tracks: &[Track],
    manual_queue: &VecDeque<QueuedTrack>,
    auto_queue: &VecDeque<usize>,
    current_playing_track: Option<Track>,
    previous_playing_track: Option<Track>,
    scroll: usize,
) {
    let popup_area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, popup_area);

    let total_width = popup_area.width as usize;
    let title_width = (total_width * 65) / 100;
    let artist_width = (total_width * 25) / 100;

    let mut rows: Vec<Row> = Vec::new();

    if let Some(track) = previous_playing_track {
        rows.push(
            Row::new(vec![
                truncate_with_ellipsis(&track.title, title_width),
                truncate_with_ellipsis(&track.artists, artist_width),
                track.duration.clone(),
            ])
            .style(Style::default().fg(theme::current().dim)),
        );
    } else {
        rows.push(
            Row::new(vec!["Previous: None", "", ""]).style(Style::default().fg(theme::current().dim)),
        );
    }

    if let Some(track) = current_playing_track {
        rows.push(
            Row::new(vec![
                truncate_with_ellipsis(&track.title, title_width),
                truncate_with_ellipsis(&track.artists, artist_width),
                track.duration.clone(),
            ])
            .style(Style::default().bg(theme::current().selection_bg).fg(theme::current().fg)),
        );
    } else {
        rows.push(Row::new(vec!["Now Playing: None", "", ""]));
    }

    // Upcoming tracks, manual queue first; `scroll` of them are off the top.
    let max_rows = popup_area.height.saturating_sub(3) as usize;
    let upcoming = manual_queue
        .iter()
        .map(|queued| &queued.track)
        .chain(auto_queue.iter().filter_map(|idx| queue_tracks.get(*idx)));
    let scroll = scroll.min((manual_queue.len() + auto_queue.len()).saturating_sub(1));
    for track in upcoming.skip(scroll).take(max_rows.saturating_sub(rows.len())) {
        let mut row = Row::new(vec![
            truncate_with_ellipsis(&track.title, title_width),
            truncate_with_ellipsis(&track.artists, artist_width),
            track.duration.clone(),
        ]);
        if !track.is_playable() {
            row = row.style(Style::default().fg(theme::current().dim));
        }
        rows.push(row);
    }

    if rows.len() <= 2 {
        rows.push(Row::new(vec!["Queue is empty", "", ""]));
    }

    let header = styled_header(&["Title", "Artist", "Duration"]);
    let table = Table::new(
        rows,
        vec![
            Constraint::Percentage(65),
            Constraint::Percentage(25),
            Constraint::Percentage(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("Queue")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded),
    )
    .column_spacing(1);
    frame.render_widget(table, popup_area);
}
