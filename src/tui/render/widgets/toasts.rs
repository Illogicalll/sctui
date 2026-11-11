//! Toasts: one-line notices in small boxes, stacked down the top-right corner over
//! whatever else is on screen.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, BorderType},
};

use crate::theme;
use crate::tui::logic::toast::{Toast, ToastKind};

use super::{clear, fit, put, text_width};

/// Draw `toasts` from row 1 down, oldest first. A box is as wide as its text, up to
/// the terminal less a cell either side; ones that do not fit below are left out.
pub fn draw(frame: &mut Frame, toasts: &[Toast]) {
    let area = frame.area();
    let t = theme::current();
    let mut y = area.y + 1;
    for toast in toasts {
        let (mark, colour) = match toast.kind {
            ToastKind::Ok => ("✓", t.accent),
            ToastKind::Err => ("✗", t.warning),
        };
        let text = format!("{mark} {}", toast.msg);
        let width = (text_width(&text) + 4).min(area.width.saturating_sub(2) as usize) as u16;
        if width < 5 || y + 3 > area.bottom() {
            break;
        }
        let rect = Rect::new(area.right() - width - 1, y, width, 3);
        let style = Style::new().fg(colour);
        clear(frame, rect);
        frame.render_widget(Block::bordered().border_type(BorderType::Rounded).border_style(style), rect);
        put(frame, Rect::new(rect.x + 2, y + 1, width - 4, 1), Line::styled(fit(&text, width as usize - 4), style));
        y += 3;
    }
}

#[cfg(test)]
mod tests {
    use crate::tui::logic::state::AppState;
    use crate::tui::render::testkit::{WIDE_TITLES, fixture, render_buffer, text};

    #[test]
    fn toast_box_clamps_to_width() {
        let (mut state, mut data, views, np) = fixture();
        state.toast("Added to queue · 3");
        state.toast_err(format!("Adding to playlist failed: {}", WIDE_TITLES[2]));
        for w in [36, 40, 80, 240] {
            let buf = render_buffer(w, 24, &mut state, &mut data, &views, &np);
            let screen = text(&buf);
            let lines: Vec<&str> = screen.lines().collect();
            // Two boxes, rows 1-3 and 4-6, each ending one cell short of the right edge.
            for (top, start) in [(1u16, "✓ Added to queue · 3"), (4, "✗ Adding to")] {
                let edge: Vec<&str> = (top..top + 3).map(|y| buf[(w - 2, y)].symbol()).collect();
                assert_eq!(edge, ["╮", "│", "╯"], "{w} cols: the box ends one cell short of the edge\n{screen}");
                assert!(lines[top as usize + 1].contains(start), "{w} cols:\n{screen}");
            }
            // The long one is cut with an ellipsis inside its box, a cell clear of the left edge.
            let long = (0..w).find(|&x| buf[(x, 4)].symbol() == "╭").unwrap();
            assert!(long >= 1, "{w} cols:\n{screen}");
            if w < 240 {
                assert_eq!(long, 1, "{w} cols: the long toast takes the full width\n{screen}");
                assert!(lines[5].contains('…'), "{w} cols:\n{screen}");
            }
        }
        // Too short for a third box: it is left out, not clipped. Nothing at all when too small.
        state.toast("Shuffle on");
        let screen = text(&render_buffer(80, 8, &mut state, &mut data, &views, &np));
        assert!(screen.contains("✓ Added") && screen.contains('✗') && !screen.contains("Shuffle on"), "{screen}");
        assert!(text(&render_buffer(80, 10, &mut state, &mut data, &views, &np)).contains("Shuffle on"));
        let mut state = AppState::default();
        state.toast("x");
        assert!(!text(&render_buffer(20, 6, &mut state, &mut data, &views, &np)).contains('✓'));
    }
}
