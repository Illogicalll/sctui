use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType},
};

use crate::tui::logic::state::VisualizerMode;

const MAX_POINTS: usize = 512;
const VOLUME_FLOOR: f32 = 0.2;
const MAX_GAIN: f32 = 1.2;
const GAIN_SMOOTH: f32 = 0.05;
const WAVE_SMOOTH: f32 = 0.12;

static SMOOTH_GAIN: OnceLock<Mutex<f32>> = OnceLock::new();

fn split_channels(samples: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for (i, sample) in samples.iter().copied().enumerate() {
        if i % 2 == 0 {
            left.push(sample);
        } else {
            right.push(sample);
        }
    }
    if right.is_empty() {
        right = left.clone();
    }
    (left, right)
}

fn normalize(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let rms = (samples.iter().map(|v| v * v).sum::<f32>() / samples.len() as f32).sqrt();
    let target_gain = (0.9 / rms.max(VOLUME_FLOOR)).min(MAX_GAIN);

    let gain_lock = SMOOTH_GAIN.get_or_init(|| Mutex::new(1.0));
    let mut gain = gain_lock.lock().unwrap();
    *gain = *gain + (target_gain - *gain) * GAIN_SMOOTH;
    let applied_gain = *gain;

    let mut out = Vec::with_capacity(samples.len());
    let mut prev = 0.0_f32;
    for sample in samples {
        let smoothed = prev + (sample - prev) * WAVE_SMOOTH;
        prev = smoothed;
        out.push((smoothed * applied_gain).clamp(-1.0, 1.0));
    }
    out
}

fn downsample(samples: &[f32], max_points: usize) -> Vec<f32> {
    if samples.len() <= max_points {
        return samples.to_vec();
    }
    let step = samples.len() as f32 / max_points as f32;
    (0..max_points)
        .map(|i| {
            let idx = (i as f32 * step).floor() as usize;
            samples[idx.min(samples.len() - 1)]
        })
        .collect()
}

pub fn render_visualizer(
    frame: &mut Frame,
    area: Rect,
    wave_buffer: &Arc<Mutex<VecDeque<f32>>>,
    _mode: VisualizerMode,
) {
    let samples: Vec<f32> = {
        let buffer = wave_buffer.lock().unwrap();
        buffer.iter().copied().collect()
    };

    let (left, right) = split_channels(&samples);
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
                .title_alignment(ratatui::layout::Alignment::Center)
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded),
        )
        .x_axis(Axis::default().bounds([0.0, max_x]))
        .y_axis(Axis::default().bounds([-1.0, 1.0]));

    frame.render_widget(chart, area);
}
