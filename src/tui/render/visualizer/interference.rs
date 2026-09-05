use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::Clear,
};

use super::common::{dim, frame_block, gradient_cyan_magenta, spectrum_bands, tick_dt};

const RAMP: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
const BANDS: usize = 16;
/// (kx, ky, speed): spatial frequency in radians per cell, and phase speed.
/// One wave per band group: bass, low-mid, mid, treble.
const WAVES: [(f32, f32, f32); 4] = [
    (0.18, 0.00, 1.3),
    (0.00, 0.22, 0.9),
    (0.12, 0.12, 1.7),
    (-0.09, 0.15, 1.1),
];
const BAND_GROUPS: [std::ops::Range<usize>; 4] = [0..3, 3..7, 7..11, 11..16];
const CELL_ASPECT: f32 = 2.0; // a cell is ~twice as tall as it is wide

#[derive(Default)]
struct FieldState {
    t: f32,
    last: Option<Instant>,
}

static STATE: OnceLock<Mutex<FieldState>> = OnceLock::new();

/// ASCII density ramp driven by four travelling waves whose amplitudes come
/// from the band groups. Louder music = faster, higher-contrast field.
pub fn render_interference_field(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Clear, inner);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let bands = spectrum_bands(samples, BANDS);
    let amps: [f32; 4] = std::array::from_fn(|k| {
        let r = BAND_GROUPS[k].clone();
        let n = r.len() as f32;
        0.15 + 0.85 * (bands[r].iter().sum::<f32>() / n)
    });
    let energy = amps.iter().sum::<f32>() / 4.0;
    let total: f32 = amps.iter().sum();

    let t = {
        let mut state = STATE.get_or_init(|| Mutex::new(FieldState::default())).lock().unwrap();
        let dt = tick_dt(&mut state.last);
        state.t += dt * (0.6 + 2.5 * energy);
        state.t
    };

    let buf = frame.buffer_mut();
    for row in 0..inner.height {
        let py = row as f32 * CELL_ASPECT;
        for col in 0..inner.width {
            let px = col as f32;
            let mut v = 0.0_f32;
            for (k, &(kx, ky, sp)) in WAVES.iter().enumerate() {
                v += amps[k] * (px * kx + py * ky + t * sp).sin();
            }
            let u = ((v / total) + 1.0) * 0.5; // 0..1
            let idx = (u * (RAMP.len() - 1) as f32).round() as usize;
            let ch = RAMP[idx.min(RAMP.len() - 1)];
            if let Some(cell) = buf.cell_mut((inner.x + col, inner.y + row)) {
                cell.set_char(ch).set_style(
                    Style::default().fg(dim(gradient_cyan_magenta(u), 0.35 + 0.65 * u)),
                );
            }
        }
    }
}
