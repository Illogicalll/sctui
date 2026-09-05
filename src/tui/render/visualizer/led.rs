use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::Clear,
};

use super::common::{frame_block, spectrum_bands};

const COL_PITCH: usize = 3; // "●  "
const TRIM_RIGHT_BANDS: usize = 2;
const RED: Color = Color::Rgb(255, 92, 92);
const YELLOW: Color = Color::Rgb(255, 209, 102);
const GREEN: Color = Color::Rgb(92, 247, 138);
const UNLIT: Color = Color::Rgb(38, 38, 48);

/// Discrete LED dots, green at the bottom through yellow to red at the top.
pub fn render_led_matrix(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Clear, inner);

    if inner.width < COL_PITCH as u16 || inner.height == 0 {
        return;
    }

    let w = inner.width as usize;
    let rows = inner.height as usize;
    let cols = w / COL_PITCH;
    let bands = spectrum_bands(samples, cols + TRIM_RIGHT_BANDS);
    let buf = frame.buffer_mut();

    for (i, &v) in bands.iter().take(cols).enumerate() {
        let lit = (v.clamp(0.0, 1.0) * rows as f32).round() as usize;
        let x = inner.x + (i * COL_PITCH + 1) as u16;
        for row in 0..rows {
            let t = if rows <= 1 { 0.0 } else { row as f32 / (rows - 1) as f32 };
            let color = if t > 0.8 {
                RED
            } else if t > 0.55 {
                YELLOW
            } else {
                GREEN
            };
            let y = inner.y + (rows - 1 - row) as u16;
            if let Some(cell) = buf.cell_mut((x, y)) {
                if row < lit {
                    cell.set_symbol("●").set_style(Style::default().fg(color));
                } else {
                    cell.set_symbol("○").set_style(Style::default().fg(UNLIT));
                }
            }
        }
    }
}
