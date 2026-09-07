use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;
use crate::theme;

use ratatui::{
    Frame,
    layout::Rect,
    style::Color,
    symbols::Marker,
    widgets::canvas::{Canvas, Line},
};

use super::common::{dim, dot_bounds, frame_block, rms, split_channels};

const PUSH_INTERVAL_MS: u128 = 30;
const WINDOW_SAMPLES: usize = 2048;
const PEAK_DECAY: f32 = 0.995;
const FILL: f64 = 0.9;

#[derive(Default)]
struct SeisState {
    left: VecDeque<f32>,
    right: VecDeque<f32>,
    last_push: Option<Instant>,
    peak: f32,
}

static STATE: OnceLock<Mutex<SeisState>> = OnceLock::new();

/// Loudness over the last few seconds. Left channel draws upward from the
/// centre line, right channel mirrors it downward; newest at the right edge.
pub fn render_seismograph(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    let (w, h) = dot_bounds(inner);
    if w < 8.0 || h < 8.0 {
        frame.render_widget(block, area);
        return;
    }

    let tail = &samples[samples.len().saturating_sub(WINDOW_SAMPLES)..];
    let (l, r) = split_channels(tail);
    let (rms_l, rms_r) = (rms(&l), rms(&r));

    let cap = w as usize;
    let mut state = STATE.get_or_init(|| Mutex::new(SeisState::default())).lock().unwrap();
    let due = state
        .last_push
        .is_none_or(|t| t.elapsed().as_millis() >= PUSH_INTERVAL_MS);
    if due {
        state.left.push_back(rms_l);
        state.right.push_back(rms_r);
        while state.left.len() > cap {
            state.left.pop_front();
            state.right.pop_front();
        }
        state.last_push = Some(Instant::now());
    }
    let cur_peak = state
        .left
        .iter()
        .chain(state.right.iter())
        .copied()
        .fold(0.0_f32, f32::max);
    state.peak = (state.peak * PEAK_DECAY).max(cur_peak).max(0.02);

    let cy = h / 2.0;
    let scale = (h / 2.0) * FILL / state.peak as f64;
    let len = state.left.len();
    let x_of = |j: usize| (w - 1.0) - (len - 1 - j) as f64;

    let polyline = |vals: &VecDeque<f32>, sign: f64, color: Color| -> Vec<Line> {
        vals.iter()
            .enumerate()
            .zip(vals.iter().skip(1))
            .map(|((j, &a), &b)| Line {
                x1: x_of(j),
                y1: cy + sign * a as f64 * scale,
                x2: x_of(j + 1),
                y2: cy + sign * b as f64 * scale,
                color,
            })
            .collect()
    };
    let left_lines = polyline(&state.left, 1.0, theme::current().accent);
    let right_lines = polyline(&state.right, -1.0, theme::current().secondary);
    drop(state);

    let baseline = Line {
        x1: 0.0,
        y1: cy,
        x2: w - 1.0,
        y2: cy,
        color: dim(theme::current().fg, 0.2),
    };

    let canvas = Canvas::default()
        .block(block)
        .marker(Marker::Braille)
        .x_bounds([0.0, w])
        .y_bounds([0.0, h])
        .paint(|ctx| {
            ctx.draw(&baseline);
            for line in left_lines.iter().chain(right_lines.iter()) {
                ctx.draw(line);
            }
        });
    frame.render_widget(canvas, area);
}
