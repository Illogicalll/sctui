use std::collections::VecDeque;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Row, Table},
};

use crate::api::Track;
use crate::tui::logic::state::QueuedTrack;
use crate::tui::render::utils::{styled_header, truncate_with_ellipsis};

use super::utils::centered_rect;

pub fn render_queue(
    frame: &mut Frame,
    queue_tracks: &Vec<Track>,
    manual_queue: &VecDeque<QueuedTrack>,
    auto_queue: &VecDeque<usize>,
    current_playing_track: Option<Track>,
    previous_playing_track: Option<Track>,
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
            .style(Style::default().fg(Color::DarkGray)),
        );
    } else {
        rows.push(
            Row::new(vec!["Previous: None", "", ""]).style(Style::default().fg(Color::DarkGray)),
        );
    }

    if let Some(track) = current_playing_track {
        rows.push(
            Row::new(vec![
                truncate_with_ellipsis(&track.title, title_width),
                truncate_with_ellipsis(&track.artists, artist_width),
                track.duration.clone(),
            ])
            .style(Style::default().bg(Color::LightBlue).fg(Color::White)),
        );
    } else {
        rows.push(Row::new(vec!["Now Playing: None", "", ""]));
    }

    let max_rows = popup_area.height.saturating_sub(3) as usize;
    let remaining_slots = max_rows.saturating_sub(rows.len());
    let mut remaining = remaining_slots;
    for queued in manual_queue.iter() {
        if remaining == 0 {
            break;
        }
        let track = &queued.track;
        let mut row = Row::new(vec![
            truncate_with_ellipsis(&track.title, title_width),
            truncate_with_ellipsis(&track.artists, artist_width),
            track.duration.clone(),
        ]);
        if !track.is_playable() {
            row = row.style(Style::default().fg(Color::DarkGray));
        }
        rows.push(row);
        remaining -= 1;
    }

    for idx in auto_queue.iter() {
        if remaining == 0 {
            break;
        }
        if let Some(track) = queue_tracks.get(*idx) {
            let mut row = Row::new(vec![
                truncate_with_ellipsis(&track.title, title_width),
                truncate_with_ellipsis(&track.artists, artist_width),
                track.duration.clone(),
            ]);
            if !track.is_playable() {
                row = row.style(Style::default().fg(Color::DarkGray));
            }
            rows.push(row);
            remaining -= 1;
        }
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
