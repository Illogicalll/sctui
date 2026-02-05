use std::f32::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Clear},
};

use crate::tui::logic::state::VisualizerMode;

use rustfft::{FftPlanner, num_complex::Complex, num_traits::Zero};

const FFT_SIZE: usize = 2048;
const DB_FLOOR: f32 = -60.0;
const DB_CEIL: f32 = -6.0;
const OFFSET_SMOOTH: f32 = 0.08;
const BAR_RISE: f32 = 0.55;
const BAR_FALL: f32 = 0.14;
const SPECTRUM_BAR_WIDTH: usize = 2;
const SPECTRUM_BAR_GAP: usize = 1;
const SPECTRUM_TRIM_RIGHT_BARS: usize = 4;
const SPECTRUM_MIN_BIN: usize = 2;
const SPECTRUM_CONTRAST_GAMMA: f32 = 1.8;
const SPECTRUM_NOISE_GATE: f32 = 0.06;
const SPECTRUM_NEIGHBOR_SMOOTH: f32 = 0.08;
const SPECTRUM_PREEMPH_STRENGTH: f32 = 0.55;
const SPECTRUM_PREEMPH_MAX: f32 = 3.2;
const SPECTRUM_LOW_SHELF: f32 = 0.78;
const SPECTRUM_MAX_HEIGHT_FRACTION: f32 = 0.85;

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
        if FFT_SIZE <= 1 {
            return vec![1.0];
        }
        (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE - 1) as f32).cos()))
            .collect()
    })
}

fn spectrum_state() -> &'static Mutex<SpectrumState> {
    SPECTRUM_STATE.get_or_init(|| Mutex::new(SpectrumState::default()))
}

pub fn render_spectrum_bars(frame: &mut Frame, area: Rect, samples: &[f32], mode: VisualizerMode) {
    let block = Block::default()
        .title("sctui")
        .title_alignment(ratatui::layout::Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Clear, inner);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let inner_w = inner.width as usize;
    let draw_bar_count =
        ((inner_w + SPECTRUM_BAR_GAP) / (SPECTRUM_BAR_WIDTH + SPECTRUM_BAR_GAP)).max(1);

    let base_bar_width = SPECTRUM_BAR_WIDTH.min(inner_w).max(1);
    let mut bar_widths = vec![base_bar_width; draw_bar_count];
    let used_base = base_bar_width * draw_bar_count
        + SPECTRUM_BAR_GAP * draw_bar_count.saturating_sub(1);
    if used_base < inner_w {
        let extra = inner_w - used_base;
        for i in 0..extra {
            let idx = draw_bar_count.saturating_sub(1 + i);
            bar_widths[idx] = bar_widths[idx].saturating_add(1);
        }
    }

    let compute_bar_count = draw_bar_count.saturating_add(SPECTRUM_TRIM_RIGHT_BARS);
    let bars_full = compute_spectrum_bars(samples, compute_bar_count);
    let bars = &bars_full[..draw_bar_count.min(bars_full.len())];
    draw_spectrum_bars(
        frame,
        inner,
        bars,
        &bar_widths,
        SPECTRUM_BAR_GAP,
    );
}

fn compute_spectrum_bars(samples: &[f32], bar_count: usize) -> Vec<f32> {
    if bar_count == 0 {
        return Vec::new();
    }

    // Mix down interleaved stereo into mono (L+R)/2.
    let mut mono = Vec::with_capacity(samples.len() / 2);
    for chunk in samples.chunks(2) {
        match chunk {
            [l, r] => mono.push((*l + *r) * 0.5),
            [m] => mono.push(*m),
            _ => {}
        }
    }

    let window = hann_window();
    let fft = fft_plan();

    let mut state = spectrum_state().lock().unwrap();

    if state.fft_buffer.len() != FFT_SIZE {
        state.fft_buffer = vec![Complex::zero(); FFT_SIZE];
    }
    if state.bars.len() != bar_count {
        state.bars.clear();
        state.bars.resize(bar_count, 0.0);
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
    let bounds = log_bin_bounds(bar_count, SPECTRUM_MIN_BIN, max_bin_exclusive);

    let mut bars_db = vec![DB_FLOOR; bar_count];
    for i in 0..bar_count {
        let start = bounds[i].clamp(1, max_bin_exclusive.saturating_sub(1));
        let end = bounds[i + 1].clamp(start + 1, max_bin_exclusive);

        let slice = &mags[start..end];

        let mut mag = 0.0_f32;
        for &v in slice {
            mag = mag.max(v);
        }

        let t = if bar_count <= 1 {
            0.0
        } else {
            i as f32 / (bar_count - 1) as f32
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

    let mut norms = vec![0.0_f32; bar_count];
    for i in 0..bar_count {
        let db = bars_db[i] + state.db_offset;
        let mut norm = ((db - DB_FLOOR) / (DB_CEIL - DB_FLOOR)).clamp(0.0, 1.0);

        let t = if bar_count <= 1 {
            0.0
        } else {
            i as f32 / (bar_count - 1) as f32
        };
        let gate = (SPECTRUM_NOISE_GATE * (1.0 - 0.65 * t)).clamp(0.0, 0.25);
        let gamma = (SPECTRUM_CONTRAST_GAMMA - 0.75 * t).clamp(1.05, 3.0);
        norm = ((norm - gate) / (1.0 - gate)).clamp(0.0, 1.0);
        norm = norm.powf(gamma);

        norms[i] = norm;
    }

    let mut freq_smoothed = vec![0.0_f32; bar_count];
    for i in 0..bar_count {
        let prev = norms[i.saturating_sub(1)];
        let cur = norms[i];
        let next = norms[(i + 1).min(bar_count - 1)];
        let a = SPECTRUM_NEIGHBOR_SMOOTH.clamp(0.0, 0.49);
        freq_smoothed[i] = (cur * (1.0 - 2.0 * a) + (prev + next) * a).clamp(0.0, 1.0);
    }

    for i in 0..bar_count {
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

fn draw_spectrum_bars(
    frame: &mut Frame,
    area: Rect,
    bars: &[f32],
    bar_widths: &[usize],
    bar_gap: usize,
) {
    if bars.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }

    let height = area.height as usize;
    let units_per_cell = 8usize;
    let full_units = height * units_per_cell;
    let max_units = ((full_units as f32) * SPECTRUM_MAX_HEIGHT_FRACTION).round() as usize;
    let max_units = max_units.max(1).min(full_units);

    let buf = frame.buffer_mut();

    let mut x_pos = 0usize;
    let n = bars.len().min(bar_widths.len());
    for (i, &v) in bars.iter().take(n).enumerate() {
        let units = (v.clamp(0.0, 1.0) * max_units as f32).round() as usize;
        let bar_width = bar_widths[i].max(1);

        for dx in 0..bar_width {
            let x_col = x_pos + dx;
            if x_col >= area.width as usize {
                break;
            }

            for row_from_bottom in 0..height {
                let remaining = units.saturating_sub(row_from_bottom * units_per_cell);
                let cell_units = remaining.clamp(0, units_per_cell);

                let symbol = match cell_units {
                    0 => " ",
                    1 => "▁",
                    2 => "▂",
                    3 => "▃",
                    4 => "▄",
                    5 => "▅",
                    6 => "▆",
                    7 => "▇",
                    _ => "█",
                };

                let y = area.y + (height - 1 - row_from_bottom) as u16;
                let x = area.x + x_col as u16;

                let t = if height <= 1 {
                    1.0
                } else {
                    row_from_bottom as f32 / (height - 1) as f32
                };
                let color = gradient_cyan_magenta(t);
                buf.get_mut(x, y)
                    .set_symbol(symbol)
                    .set_style(Style::default().fg(color));
            }
        }

        x_pos = x_pos.saturating_add(bar_width);
        if i + 1 < n {
            x_pos = x_pos.saturating_add(bar_gap);
        }
        if x_pos >= area.width as usize {
            break;
        }
    }
}

fn gradient_cyan_magenta(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (r0, g0, b0) = (0u8, 255u8, 255u8);
    let (r1, g1, b1) = (255u8, 0u8, 255u8);

    let lerp = |a: u8, b: u8| -> u8 {
        (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
    };

    Color::Rgb(lerp(r0, r1), lerp(g0, g1), lerp(b0, b1))
}
