use std::f64::consts::PI;

use ratatui::{
    Frame,
    layout::Rect,
    symbols::Marker,
    widgets::canvas::{Canvas, Circle, Line},
};

use super::common::{dim, dot_bounds, frame_block, gradient_cyan_magenta, spectrum_bands};

const BANDS: usize = 48;
const HUB_FRACTION: f64 = 0.35;

/// Bands arranged around a circle, mirrored left/right, lows at the bottom.
pub fn render_radial_spectrum(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    let (w, h) = dot_bounds(inner);
    if w < 8.0 || h < 8.0 {
        frame.render_widget(block, area);
        return;
    }

    let bands = spectrum_bands(samples, BANDS);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r_max = w.min(h) / 2.0 - 1.0;
    let r0 = r_max * HUB_FRACTION;
    let len_max = r_max - r0;

    let mut lines = Vec::with_capacity(BANDS * 2);
    for (i, &v) in bands.iter().enumerate() {
        let t = (i as f64 + 0.5) / BANDS as f64; // 0 = bottom, 1 = top
        let len = (v as f64 * len_max).max(1.0);
        let color = gradient_cyan_magenta(v);
        for ang in [-PI / 2.0 - t * PI, -PI / 2.0 + t * PI] {
            let (s, c) = ang.sin_cos();
            lines.push(Line {
                x1: cx + r0 * c,
                y1: cy + r0 * s,
                x2: cx + (r0 + len) * c,
                y2: cy + (r0 + len) * s,
                color,
            });
        }
    }

    let hub = Circle {
        x: cx,
        y: cy,
        radius: (r0 - 1.5).max(1.0),
        color: dim(gradient_cyan_magenta(0.0), 0.35),
    };

    let canvas = Canvas::default()
        .block(block)
        .marker(Marker::Braille)
        .x_bounds([0.0, w])
        .y_bounds([0.0, h])
        .paint(|ctx| {
            ctx.draw(&hub);
            for line in &lines {
                ctx.draw(line);
            }
        });
    frame.render_widget(canvas, area);
}
