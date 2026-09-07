use ratatui::{
    Frame,
    layout::Rect,
    symbols::Marker,
    widgets::canvas::{Canvas, Circle},
};

use super::common::{dim, dot_bounds, frame_block, gradient_cyan_magenta, spectrum_bands};

const RINGS: usize = 14;
const MIN_VISIBLE: f32 = 0.04;
const BASS_BREATHE: f64 = 0.06;

/// Concentric rings, one per band (lows inside). Brightness = magnitude, the
/// whole set breathes outward with the bass.
pub fn render_spectrum_rings(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    let (w, h) = dot_bounds(inner);
    if w < 8.0 || h < 8.0 {
        frame.render_widget(block, area);
        return;
    }

    let bands = spectrum_bands(samples, RINGS);
    let bass = (bands[0] + bands[1]) as f64 * 0.5;
    let (cx, cy) = (w / 2.0, h / 2.0);
    let r_max = w.min(h) / 2.0 - 1.0;
    let spacing = r_max / RINGS as f64;
    let breathe = 1.0 - BASS_BREATHE + BASS_BREATHE * bass;

    let mut circles = Vec::with_capacity(RINGS * 2);
    for (i, &v) in bands.iter().enumerate() {
        if v < MIN_VISIBLE {
            continue;
        }
        let radius = spacing * (i + 1) as f64 * breathe;
        let color = dim(
            gradient_cyan_magenta(i as f32 / (RINGS - 1) as f32),
            0.25 + 0.75 * v,
        );
        circles.push(Circle { x: cx, y: cy, radius, color });
        if v > 0.7 && radius > 2.0 {
            // Loud band: thicken the ring.
            circles.push(Circle { x: cx, y: cy, radius: radius - 1.0, color });
        }
    }

    let canvas = Canvas::default()
        .block(block)
        .marker(Marker::Braille)
        .x_bounds([0.0, w])
        .y_bounds([0.0, h])
        .paint(|ctx| {
            for c in &circles {
                ctx.draw(c);
            }
        });
    frame.render_widget(canvas, area);
}
