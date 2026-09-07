use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    symbols::{self},
    text::{Span, Text},
    widgets::{Axis, Block, Chart, Dataset, Gauge, Paragraph},
};
use ratatui_image::{Resize, StatefulImage, thread::ThreadProtocol};
use crate::theme;

use crate::api::format_duration;
use crate::player::Player;
use crate::tui::logic::state::AppState;

pub fn render_now_playing(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    player: &Player,
    cover_art_async: &mut ThreadProtocol,
) {
    let selected_track = player.current_track();
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

    frame.render_widget(now_playing.clone(), subchunks[1]);
    let inner_area = now_playing.inner(subchunks[1]);

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

    let raw_image_area = horizontal_split[1];
    let x1 = raw_image_area.x.max(inner_area.x);
    let y1 = raw_image_area.y.max(inner_area.y);
    let x2 = raw_image_area
        .x
        .saturating_add(raw_image_area.width)
        .min(inner_area.x.saturating_add(inner_area.width));
    let y2 = raw_image_area
        .y
        .saturating_add(raw_image_area.height)
        .min(inner_area.y.saturating_add(inner_area.height));
    let image_area = ratatui::layout::Rect {
        x: x1,
        y: y1,
        width: x2.saturating_sub(x1),
        height: y2.saturating_sub(y1),
    };
    let image_widget = StatefulImage::new().resize(Resize::Scale(None));
    frame.render_stateful_widget(image_widget, image_area, cover_art_async);

    let song_name = Paragraph::new(selected_track.title.clone())
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(song_name, subsubchunks[1]);

    let artist = Paragraph::new(selected_track.artists.clone())
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(artist, subsubchunks[3]);

    let max_time: f64 = selected_track.duration_ms as f64;

    let progress_float = state.progress as f64;

    let label = Span::styled(
        format!(
            "{} / {}",
            format_duration(state.progress),
            selected_track.duration.clone()
        ),
        Style::default().fg(theme::current().fg),
    );

    let ratio = (progress_float / max_time).min(1.0).max(0.0);

    let progress_bar = Gauge::default()
        .style(Style::default().bg(theme::current().selection_bg))
        .gauge_style(theme::current().accent)
        .ratio(ratio)
        .label(label);

    frame.render_widget(progress_bar, subsubchunks[5]);

    let shuffle_indicator = if state.shuffle_enabled { "✔︎" } else { "×" };
    let repeat_indicator = if state.repeat_enabled { "✔︎" } else { "×" };

    let lines = ["".to_string(),
        "".to_string(),
        format!("shf:   {}", shuffle_indicator),
        format!("vol: {:.1}", player.get_volume()),
        format!("rep:   {}", repeat_indicator)];

    let text = Text::from(lines.join("\n"));

    let song_name = Paragraph::new(text)
        .style(Style::default().add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Right);

    frame.render_widget(song_name, horizontal_split[5]);
    let data: Vec<(f64, f64)> = (0..200)
        .map(|i| {
            let x = state.tick + i as f64 * 0.1;
            (x, (x / 2.0).sin() * 10.0)
        })
        .collect();
    let datasets = vec![
        Dataset::default()
            .marker(symbols::Marker::Braille)
            .style(Style::default().fg(theme::current().accent))
            .data(&data),
    ];

    let chart = Chart::new(datasets)
        .block(Block::default())
        .x_axis(Axis::default().bounds([state.tick, state.tick + 20.0]))
        .y_axis(Axis::default().bounds([-10.0, 10.0]));

    frame.render_widget(&chart, subchunks[2]);
    frame.render_widget(&chart, subchunks[0]);
}
