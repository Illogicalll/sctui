use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::Clear,
};

use super::common::{frame_block, gradient_cyan_magenta, spectrum_bands};

const SPECTRUM_BAR_WIDTH: usize = 2;
const SPECTRUM_BAR_GAP: usize = 1;
const SPECTRUM_TRIM_RIGHT_BARS: usize = 4;
const SPECTRUM_MAX_HEIGHT_FRACTION: f32 = 0.85;

pub fn render_spectrum_bars(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
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
    let bars_full = spectrum_bands(samples, compute_bar_count);
    let bars = &bars_full[..draw_bar_count.min(bars_full.len())];
    draw_spectrum_bars(
        frame,
        inner,
        bars,
        &bar_widths,
        SPECTRUM_BAR_GAP,
    );
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
