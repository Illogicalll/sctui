use std::collections::VecDeque;
use std::f64::consts::PI;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use ratatui::{
    Frame,
    layout::Rect,
    symbols::Marker,
    widgets::canvas::{Canvas, Line},
};

use super::common::{dim, dot_bounds, frame_block, gradient_cyan_magenta, spectrum_bands};

const ROWS: usize = 22;
const PUSH_INTERVAL_MS: u128 = 60;
const DOTS_PER_BAND: f64 = 4.0;
const STACK_HEIGHT: f64 = 0.55; // fraction of height the baselines span
const AMPLITUDE: f64 = 0.40; // fraction of height a full band reaches
const MARGIN_X: f64 = 0.06;

#[derive(Default)]
struct RidgeState {
    history: VecDeque<Vec<f32>>,
    last_push: Option<Instant>,
}

static STATE: OnceLock<Mutex<RidgeState>> = OnceLock::new();

/// Each frame's spectrum as a line; older frames stack up behind and fade.
pub fn render_ridgeline(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    let (w, h) = dot_bounds(inner);
    if w < 8.0 || h < 8.0 {
        frame.render_widget(block, area);
        return;
    }

    let n = ((w / DOTS_PER_BAND) as usize).max(8);
    let bands = spectrum_bands(samples, n);

    let mut state = STATE.get_or_init(|| Mutex::new(RidgeState::default())).lock().unwrap();
    if state.history.front().is_some_and(|row| row.len() != n) {
        state.history.clear();
    }
    let due = state
        .last_push
        .is_none_or(|t| t.elapsed().as_millis() >= PUSH_INTERVAL_MS);
    if due {
        state.history.push_front(bands);
        state.history.truncate(ROWS);
        state.last_push = Some(Instant::now());
    }

    let spacing = h * STACK_HEIGHT / ROWS as f64;
    let base0 = h * 0.04;
    let amp = h * AMPLITUDE;
    let x_of = |i: usize| w * (MARGIN_X + (1.0 - 2.0 * MARGIN_X) * i as f64 / (n - 1) as f64);

    let mut lines = Vec::new();
    // Oldest first so the newest ridge's colour wins where they overlap.
    for (k, row) in state.history.iter().enumerate().rev() {
        let age = k as f32 / ROWS as f32;
        let color = dim(gradient_cyan_magenta(age), 1.0 - 0.75 * age);
        let base = base0 + k as f64 * spacing;
        let mut prev: Option<(f64, f64)> = None;
        for (i, &v) in row.iter().enumerate() {
            // Taper toward the edges so the ridges read as hills, not a graph.
            let taper = (PI * i as f64 / (n - 1) as f64).sin().sqrt();
            let p = (x_of(i), base + v as f64 * amp * taper);
            if let Some(q) = prev {
                lines.push(Line { x1: q.0, y1: q.1, x2: p.0, y2: p.1, color });
            }
            prev = Some(p);
        }
    }
    drop(state);

    let canvas = Canvas::default()
        .block(block)
        .marker(Marker::Braille)
        .x_bounds([0.0, w])
        .y_bounds([0.0, h])
        .paint(|ctx| {
            for line in &lines {
                ctx.draw(line);
            }
        });
    frame.render_widget(canvas, area);
}
