use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Axis, Chart, Dataset, GraphType},
};
use crate::theme;

use super::common::{MAX_POINTS, downsample, frame_block, normalize, split_channels};

const OSCILLOSCOPE_WINDOW_SAMPLES: usize = 1024;

pub type Points = Vec<(f64, f64)>;

/// Left/right traces of the most recent window, normalised to -1..1.
pub fn scope_points(samples: &[f32]) -> (Points, Points) {
    let samples = oscilloscope_window(samples);
    let (left, right) = split_channels(samples);
    let to_points = |ch: &[f32]| -> Points {
        downsample(&normalize(ch), MAX_POINTS)
            .iter()
            .enumerate()
            .map(|(i, s)| (i as f64, *s as f64))
            .collect()
    };
    (to_points(&left), to_points(&right))
}

pub fn scope_dataset(points: &[(f64, f64)], color: Color) -> Dataset<'_> {
    Dataset::default()
        .marker(ratatui::symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::default().fg(color))
        .data(points)
}

pub fn scope_chart<'a>(datasets: Vec<Dataset<'a>>, max_x: f64) -> Chart<'a> {
    Chart::new(datasets)
        .x_axis(Axis::default().bounds([0.0, max_x]))
        .y_axis(Axis::default().bounds([-1.0, 1.0]))
}

pub fn render_oscilloscope(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let (left, right) = scope_points(samples);
    let max_x = left.len().max(1) as f64;

    let datasets = vec![
        scope_dataset(&left, theme::current().accent),
        scope_dataset(&right, theme::current().secondary),
    ];

    frame.render_widget(scope_chart(datasets, max_x).block(frame_block()), area);
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
