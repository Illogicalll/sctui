use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Clear,
};

use super::common::{frame_block, gradient_cyan_magenta, spectrum_bands};

const BAR_WIDTH: usize = 2;
const BAR_GAP: usize = 1;
const TRIM_RIGHT_BARS: usize = 4;
const MAX_HEIGHT_FRACTION: f32 = 0.92;

const LOWER_EIGHTHS: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

/// Spectrum bars growing up *and* down from a centre line.
pub fn render_mirror_spectrum(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Clear, inner);

    if inner.width == 0 || inner.height < 2 {
        return;
    }

    let w = inner.width as usize;
    let h = inner.height as usize;
    let bar_count = ((w + BAR_GAP) / (BAR_WIDTH + BAR_GAP)).max(1);
    let bands = spectrum_bands(samples, bar_count + TRIM_RIGHT_BARS);

    let half = h / 2;
    let seam = h % 2 == 1; // odd height: one middle row shared by both halves
    let max_units = ((half * 8) as f32 * MAX_HEIGHT_FRACTION).round().max(1.0) as usize;

    let buf = frame.buffer_mut();

    for (i, &v) in bands.iter().take(bar_count).enumerate() {
        let units = (v.clamp(0.0, 1.0) * max_units as f32).round() as usize;
        let x0 = i * (BAR_WIDTH + BAR_GAP);

        for dx in 0..BAR_WIDTH {
            let x_col = x0 + dx;
            if x_col >= w {
                break;
            }
            let x = inner.x + x_col as u16;

            for row in 0..half {
                let remaining = units.saturating_sub(row * 8);
                let cell_units = remaining.min(8);
                let t = if half <= 1 { 1.0 } else { row as f32 / (half - 1) as f32 };
                let color = gradient_cyan_magenta(t);

                // Upper half: ordinary lower-block glyphs growing upward.
                let y_up = inner.y + (half - 1 - row) as u16;
                if let Some(cell) = buf.cell_mut((x, y_up)) {
                    cell.set_symbol(LOWER_EIGHTHS[cell_units])
                        .set_style(Style::default().fg(color));
                }

                // Lower half: same glyph set drawn REVERSED. A lower (8-k)/8
                // block with fg/bg swapped reads as an upper k/8 block, which
                // is the mirror image we want.
                let y_dn = inner.y + (h - half + row) as u16;
                if let Some(cell) = buf.cell_mut((x, y_dn)) {
                    match cell_units {
                        0 => {
                            cell.set_symbol(" ");
                        }
                        8 => {
                            cell.set_symbol("█").set_style(Style::default().fg(color));
                        }
                        k => {
                            cell.set_symbol(LOWER_EIGHTHS[8 - k]).set_style(
                                Style::default().fg(color).add_modifier(Modifier::REVERSED),
                            );
                        }
                    }
                }
            }

            if seam && units > 0 {
                let y_mid = inner.y + half as u16;
                if let Some(cell) = buf.cell_mut((x, y_mid)) {
                    cell.set_symbol("█")
                        .set_style(Style::default().fg(gradient_cyan_magenta(0.0)));
                }
            }
        }
    }
}
