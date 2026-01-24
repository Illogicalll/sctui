use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::{self},
    text::{Span, Text},
    widgets::{Axis, Block, Chart, Dataset, Gauge, Paragraph},
};
use ratatui_image::{StatefulImage, thread::ThreadProtocol};

use crate::api::Track;

fn format_duration(duration_ms: u64) -> String {
    let duration_sec = duration_ms / 1000;
    let hours = duration_sec / 3600;
    let minutes = (duration_sec % 3600) / 60;
    let seconds = duration_sec % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

pub fn render_now_playing(
    frame: &mut Frame,
    area: Rect,
    data: &mut Vec<(f64, f64)>,
    window: &mut [f64; 2],
    progress: &mut u64,
    selected_track: Track,
    cover_art_async: &mut ThreadProtocol,
    current_volume: f32,
    shuffle_enabled: bool,
    repeat_enabled: bool,
) {
    let subchunks = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(15),
                Constraint::Percentage(70),
                Constraint::Percentage(15),
            ]
            .as_ref(),
        )
        .split(area);

    let now_playing = Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded);

    frame.render_widget(now_playing, subchunks[1]);

    let padding = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage(1),
            Constraint::Percentage(98),
            Constraint::Percentage(1),
        ])
        .split(subchunks[1]);

    let horizontal_split = Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            Constraint::Percentage(1),
            Constraint::Length(12),
            Constraint::Percentage(5),
            Constraint::Min(0),
            Constraint::Percentage(5),
            Constraint::Length(9),
            Constraint::Percentage(1),
        ])
        .split(padding[1]);

    let image_padding = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(horizontal_split[1]);

    let subsubchunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(12),
            Constraint::Percentage(20),
        ])
        .split(horizontal_split[3]);

    frame.render_stateful_widget(StatefulImage::new(), image_padding[1], cover_art_async);

    let song_name = Paragraph::new(selected_track.title.clone())
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(song_name, subsubchunks[1]);

    let artist = Paragraph::new(selected_track.artists.clone())
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(artist, subsubchunks[3]);

    let max_time: f64 = selected_track.duration_ms.clone() as f64;

    let progress_float = *progress as f64;

    let label = Span::styled(
        format!(
            "{} / {}",
            format_duration(*progress),
            selected_track.duration.clone()
        ),
        Style::default().fg(Color::White),
    );

    let ratio = (progress_float / max_time).min(1.0).max(0.0);

    let progress_bar = Gauge::default()
        .style(Style::default().bg(Color::LightBlue))
        .gauge_style(Color::Cyan)
        .ratio(ratio)
        .label(label);

    frame.render_widget(progress_bar, subsubchunks[5]);

    let shuffle_indicator = if shuffle_enabled { "✔︎" } else { "×" };
    let repeat_indicator = if repeat_enabled { "✔︎" } else { "×" };

    let lines = vec![
        "".to_string(),
        "".to_string(),
        format!("shf:   {}", shuffle_indicator),
        format!("vol: {:.1}", current_volume),
        format!("rep:   {}", repeat_indicator),
    ];

    let text = Text::from(lines.join("\n"));

    let song_name = Paragraph::new(text)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Right);

    frame.render_widget(song_name, horizontal_split[5]);
    let datasets = vec![
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(Color::Cyan))
            .data(&data),
    ];

    let chart = Chart::new(datasets)
        .block(Block::default())
        .x_axis(Axis::default().bounds([window[0], window[1]]))
        .y_axis(Axis::default().bounds([-10.0, 10.0]));

    frame.render_widget(&chart, subchunks[2]);
    frame.render_widget(&chart, subchunks[0]);
}
