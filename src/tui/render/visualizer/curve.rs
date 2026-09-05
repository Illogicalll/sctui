use std::f64::consts::PI;

use ratatui::{
    Frame,
    layout::Rect,
    symbols::Marker,
    widgets::canvas::{Canvas, Line},
};

use super::common::{dot_bounds, frame_block, gradient_cyan_magenta, spectrum_bands};

const SLABS: usize = 8;
const DOTS_PER_BAND: f64 = 6.0;
const TRIM_RIGHT_BANDS: usize = 2;
const MAX_HEIGHT_FRACTION: f64 = 0.95;

/// Smooth curve through the band magnitudes with the area beneath filled,
/// coloured in horizontal slabs from cyan (bottom) to magenta (top).
pub fn render_filled_curve(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    let (w, h) = dot_bounds(inner);
    if w < 4.0 || h < 4.0 {
        frame.render_widget(block, area);
        return;
    }

    let n = ((w / DOTS_PER_BAND) as usize).max(8);
    let bands = spectrum_bands(samples, n + TRIM_RIGHT_BANDS);
    let bands = &bands[..n];

    let cols = w as usize;
    let top = (h - 1.0) * MAX_HEIGHT_FRACTION;
    let slab_h = h / SLABS as f64;

    // One vertical line per dot column, split into slabs so each slab can be
    // its own colour.
    let mut lines: Vec<Vec<Line>> = vec![Vec::new(); SLABS];
    for x in 0..cols {
        let pos = x as f64 / (cols - 1).max(1) as f64 * (n - 1) as f64;
        let i = (pos.floor() as usize).min(n - 2);
        let f = pos - i as f64;
        let f = (1.0 - (f * PI).cos()) * 0.5; // cosine ease between band centres
        let v = bands[i] as f64 * (1.0 - f) + bands[i + 1] as f64 * f;
        let y = v * top;
        if y < 0.5 {
            continue;
        }
        for (s, slab) in lines.iter_mut().enumerate() {
            let y0 = s as f64 * slab_h;
            if y0 > y {
                break;
            }
            let y1 = ((s + 1) as f64 * slab_h - 0.01).min(y);
            slab.push(Line {
                x1: x as f64,
                y1: y0,
                x2: x as f64,
                y2: y1,
                color: gradient_cyan_magenta(s as f32 / (SLABS - 1) as f32),
            });
        }
    }

    let canvas = Canvas::default()
        .block(block)
        .marker(Marker::Braille)
        .x_bounds([0.0, w])
        .y_bounds([0.0, h])
        .paint(|ctx| {
            for slab in &lines {
                for line in slab {
                    ctx.draw(line);
                }
            }
        });
    frame.render_widget(canvas, area);
}
