use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType},
};

use crate::tui::logic::state::VisualizerMode;

use super::common::{downsample, normalize, split_channels, MAX_POINTS};

const OSCILLOSCOPE_WINDOW_SAMPLES: usize = 1024;

pub fn render_oscilloscope(frame: &mut Frame, area: Rect, samples: &[f32], mode: VisualizerMode) {
    let samples = oscilloscope_window(samples);
    let (left, right) = split_channels(samples);
    let left = downsample(&normalize(&left), MAX_POINTS);
    let right = downsample(&normalize(&right), MAX_POINTS);
    let max_x = left.len().max(1) as f64;

    let left_points: Vec<(f64, f64)> = left
        .iter()
        .enumerate()
        .map(|(i, sample)| (i as f64, *sample as f64))
        .collect();
    let right_points: Vec<(f64, f64)> = right
        .iter()
        .enumerate()
        .map(|(i, sample)| (i as f64, *sample as f64))
        .collect();

    let datasets: Vec<Dataset> = vec![
        Dataset::default()
            .marker(ratatui::symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&left_points),
        Dataset::default()
            .marker(ratatui::symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Magenta))
            .data(&right_points),
    ];

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .title("sctui")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .x_axis(Axis::default().bounds([0.0, max_x]))
        .y_axis(Axis::default().bounds([-1.0, 1.0]));

    frame.render_widget(chart, area);
}

fn oscilloscope_window(samples: &[f32]) -> &[f32] {
    let mut end = samples.len();
    end -= end % 2;
    if end == 0 {
        return &[];
    }

    let max_len = OSCILLOSCOPE_WINDOW_SAMPLES - (OSCILLOSCOPE_WINDOW_SAMPLES % 2);
    let start = end.saturating_sub(max_len);
    &samples[start..end]
}
