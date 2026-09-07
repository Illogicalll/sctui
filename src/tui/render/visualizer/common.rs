use std::f32::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

use ratatui::{
    layout::{Alignment, Rect},
    style::Color,
    widgets::{Block, BorderType, Borders},
};
use rustfft::{FftPlanner, num_complex::Complex, num_traits::Zero};

pub const MAX_POINTS: usize = 512;
pub const VOLUME_FLOOR: f32 = 0.2;
pub const MAX_GAIN: f32 = 1.2;
pub const GAIN_SMOOTH: f32 = 0.05;
pub const WAVE_SMOOTH: f32 = 0.12;

static SMOOTH_GAIN: OnceLock<Mutex<f32>> = OnceLock::new();

/// The rounded "sctui" frame every visualiser mode draws inside.
pub fn frame_block() -> Block<'static> {
    Block::default()
        .title("sctui")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
}

/// Braille canvas resolution for `inner` in dots: 2 across, 4 down per cell.
/// Using these as the canvas bounds gives square dots, so circles are round.
pub fn dot_bounds(inner: Rect) -> (f64, f64) {
    (inner.width as f64 * 2.0, inner.height as f64 * 4.0)
}

pub fn split_channels(samples: &[f32]) -> (Vec<f32>, Vec<f32>) {
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

/// Mix interleaved stereo down to mono (L+R)/2.
pub fn mono(samples: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(samples.len() / 2 + 1);
    for chunk in samples.chunks(2) {
        match chunk {
            [l, r] => out.push((*l + *r) * 0.5),
            [m] => out.push(*m),
            _ => {}
        }
    }
    out
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|v| v * v).sum::<f32>() / samples.len() as f32).sqrt()
}

pub fn normalize(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }

    let rms = rms(samples);
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

pub fn downsample(samples: &[f32], max_points: usize) -> Vec<f32> {
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

/// Visualiser gradient between the theme's accent and secondary colours.
/// (Named for what it was before themes existed.)
pub fn gradient_cyan_magenta(t: f32) -> Color {
    crate::theme::current().gradient(t)
}

/// Scale an RGB colour's brightness by `k` (0..1).
pub fn dim(color: Color, k: f32) -> Color {
    let k = k.clamp(0.0, 1.0);
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            (r as f32 * k) as u8,
            (g as f32 * k) as u8,
            (b as f32 * k) as u8,
        ),
        other => other,
    }
}

// ---------------------------------------------------------------------------
// FFT → log-spaced, smoothed, 0..1 band magnitudes. Shared by every
// spectrum-derived mode.
// ---------------------------------------------------------------------------

const FFT_SIZE: usize = 2048;
const DB_FLOOR: f32 = -60.0;
const DB_CEIL: f32 = -6.0;
const OFFSET_SMOOTH: f32 = 0.08;
const BAR_RISE: f32 = 0.55;
const BAR_FALL: f32 = 0.14;
const SPECTRUM_MIN_BIN: usize = 2;
const SPECTRUM_CONTRAST_GAMMA: f32 = 1.8;
const SPECTRUM_NOISE_GATE: f32 = 0.06;
const SPECTRUM_NEIGHBOR_SMOOTH: f32 = 0.08;
const SPECTRUM_PREEMPH_STRENGTH: f32 = 0.55;
const SPECTRUM_PREEMPH_MAX: f32 = 3.2;
const SPECTRUM_LOW_SHELF: f32 = 0.78;

static FFT_PLAN: OnceLock<Arc<dyn rustfft::Fft<f32>>> = OnceLock::new();
static HANN_WINDOW: OnceLock<Vec<f32>> = OnceLock::new();
static SPECTRUM_STATE: OnceLock<Mutex<SpectrumState>> = OnceLock::new();

#[derive(Default)]
struct SpectrumState {
    db_offset: f32,
    bars: Vec<f32>,
    fft_buffer: Vec<Complex<f32>>,
}

fn fft_plan() -> &'static Arc<dyn rustfft::Fft<f32>> {
    FFT_PLAN.get_or_init(|| {
        let mut planner = FftPlanner::<f32>::new();
        planner.plan_fft_forward(FFT_SIZE)
    })
}

fn hann_window() -> &'static [f32] {
    HANN_WINDOW.get_or_init(|| {
        (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos()))
            .collect()
    })
}

fn spectrum_state() -> &'static Mutex<SpectrumState> {
    SPECTRUM_STATE.get_or_init(|| Mutex::new(SpectrumState::default()))
}

/// `band_count` log-spaced bands in 0..1, auto-levelled and smoothed over time.
/// Index 0 is the lowest frequency.
pub fn spectrum_bands(samples: &[f32], band_count: usize) -> Vec<f32> {
    if band_count == 0 {
        return Vec::new();
    }

    let mono = mono(samples);
    let window = hann_window();
    let fft = fft_plan();

    let mut state = spectrum_state().lock().unwrap();

    if state.fft_buffer.len() != FFT_SIZE {
        state.fft_buffer = vec![Complex::zero(); FFT_SIZE];
    }
    if state.bars.len() != band_count {
        state.bars.clear();
        state.bars.resize(band_count, 0.0);
    }

    let available = mono.len().min(FFT_SIZE);
    let src_start = mono.len().saturating_sub(available);
    let dst_start = FFT_SIZE - available;
    for i in 0..FFT_SIZE {
        let sample = if i >= dst_start {
            mono[src_start + (i - dst_start)]
        } else {
            0.0
        };
        state.fft_buffer[i] = Complex::new(sample * window[i], 0.0);
    }

    fft.process(&mut state.fft_buffer);

    let half = FFT_SIZE / 2;
    let mut mags = vec![0.0_f32; half];
    for i in 1..half {
        let c = state.fft_buffer[i];
        mags[i] = (c.re * c.re + c.im * c.im).sqrt() / FFT_SIZE as f32;
    }

    let max_bin_exclusive = half;
    let bounds = log_bin_bounds(band_count, SPECTRUM_MIN_BIN, max_bin_exclusive);

    let mut bars_db = vec![DB_FLOOR; band_count];
    for i in 0..band_count {
        let start = bounds[i].clamp(1, max_bin_exclusive.saturating_sub(1));
        let end = bounds[i + 1].clamp(start + 1, max_bin_exclusive);

        let slice = &mags[start..end];

        let mut mag = 0.0_f32;
        for &v in slice {
            mag = mag.max(v);
        }

        let t = if band_count <= 1 {
            0.0
        } else {
            i as f32 / (band_count - 1) as f32
        };
        let center_bin = (start as f32 + end as f32) * 0.5;
        let ratio = (center_bin / SPECTRUM_MIN_BIN.max(1) as f32).max(1.0);
        let preemph = (1.0 + SPECTRUM_PREEMPH_STRENGTH * ratio.ln()).clamp(0.85, SPECTRUM_PREEMPH_MAX);
        let low_shelf = (SPECTRUM_LOW_SHELF + (1.0 - SPECTRUM_LOW_SHELF) * t).clamp(0.5, 1.2);
        mag *= preemph * low_shelf;

        bars_db[i] = 20.0 * (mag.max(1e-9)).log10();
    }

    let max_db = bars_db
        .iter()
        .copied()
        .fold(DB_FLOOR, |acc, v| acc.max(v));
    let target_offset = if max_db <= DB_FLOOR + 0.5 {
        0.0
    } else {
        (DB_CEIL - max_db).clamp(-24.0, 24.0)
    };
    state.db_offset = state.db_offset + (target_offset - state.db_offset) * OFFSET_SMOOTH;

    let mut norms = vec![0.0_f32; band_count];
    for i in 0..band_count {
        let db = bars_db[i] + state.db_offset;
        let mut norm = ((db - DB_FLOOR) / (DB_CEIL - DB_FLOOR)).clamp(0.0, 1.0);

        let t = if band_count <= 1 {
            0.0
        } else {
            i as f32 / (band_count - 1) as f32
        };
        let gate = (SPECTRUM_NOISE_GATE * (1.0 - 0.65 * t)).clamp(0.0, 0.25);
        let gamma = (SPECTRUM_CONTRAST_GAMMA - 0.75 * t).clamp(1.05, 3.0);
        norm = ((norm - gate) / (1.0 - gate)).clamp(0.0, 1.0);
        norm = norm.powf(gamma);

        norms[i] = norm;
    }

    let mut freq_smoothed = vec![0.0_f32; band_count];
    for i in 0..band_count {
        let prev = norms[i.saturating_sub(1)];
        let cur = norms[i];
        let next = norms[(i + 1).min(band_count - 1)];
        let a = SPECTRUM_NEIGHBOR_SMOOTH.clamp(0.0, 0.49);
        freq_smoothed[i] = (cur * (1.0 - 2.0 * a) + (prev + next) * a).clamp(0.0, 1.0);
    }

    for i in 0..band_count {
        let norm = freq_smoothed[i];
        let prev = state.bars[i];
        let k = if norm > prev { BAR_RISE } else { BAR_FALL };
        state.bars[i] = prev + (norm - prev) * k;
    }

    state.bars.clone()
}

fn log_bin_bounds(bar_count: usize, min_bin: usize, max_bin_exclusive: usize) -> Vec<usize> {
    let bar_count = bar_count.max(1);
    let max_bin_exclusive = max_bin_exclusive.max(2);
    let min_bin = min_bin.clamp(1, max_bin_exclusive.saturating_sub(1));

    let min_log = (min_bin as f32).ln();
    let max_log = (max_bin_exclusive as f32).ln();

    let mut bounds = Vec::with_capacity(bar_count + 1);
    bounds.push(min_bin);

    let mut last = min_bin;
    for i in 1..bar_count {
        let t = i as f32 / bar_count as f32;
        let raw = (min_log + t * (max_log - min_log)).exp();
        let mut b = raw.round() as usize;

        let remaining = bar_count - i;
        let max_allowed = max_bin_exclusive.saturating_sub(remaining).max(min_bin + 1);

        if max_allowed > last {
            b = b.clamp(last + 1, max_allowed);
        } else {
            b = last;
        }

        bounds.push(b);
        last = b;
    }

    bounds.push(max_bin_exclusive);
    bounds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrum_bands_shape_and_range() {
        // 1 kHz-ish tone, interleaved stereo.
        let samples: Vec<f32> = (0..4096)
            .flat_map(|i| {
                let v = (i as f32 * 0.14).sin();
                [v, v]
            })
            .collect();
        for _ in 0..30 {
            let bands = spectrum_bands(&samples, 32);
            assert_eq!(bands.len(), 32);
            assert!(bands.iter().all(|v| (0.0..=1.0).contains(v)));
        }
        let bands = spectrum_bands(&samples, 32);
        assert!(bands.iter().any(|&v| v > 0.3), "tone should light some band");
        assert!(spectrum_bands(&samples, 0).is_empty());
        assert_eq!(spectrum_bands(&[], 8).len(), 8);
    }
}

/// Seconds since the previous call for this state, clamped so a stalled frame
/// (tabbed away, resize) never makes a simulation explode.
pub fn tick_dt(last: &mut Option<std::time::Instant>) -> f32 {
    let now = std::time::Instant::now();
    let dt = last
        .map(|l| now.duration_since(l).as_secs_f32())
        .unwrap_or(0.0);
    *last = Some(now);
    dt.min(0.05)
}
