//! Drawing shared by every layout: panes, the pane row, the player readouts, the hint
//! line and toasts.

pub mod header;
pub mod hints;
pub mod pane;
pub mod pane_row;
pub mod player;
pub mod toasts;

use ratatui::{Frame, layout::Rect, style::Style, text::{Line, Span}};

/// Terminal cells `s` occupies (wide glyphs count twice).
pub fn text_width(s: &str) -> usize {
    Span::raw(s).width()
}

/// `s` cut to at most `width` cells, ending in `…` when anything was cut. Never
/// splits a char, and a wide glyph that would straddle the limit is dropped whole.
pub fn fit(s: &str, width: usize) -> String {
    if text_width(s) <= width {
        return s.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let mut buf = [0u8; 4];
        let w = text_width(c.encode_utf8(&mut buf));
        if used + w + 1 > width {
            break;
        }
        used += w;
        out.push(c);
    }
    if width > 0 {
        out.push('…');
    }
    out
}

/// Draw one line of text in `rect`, clipped to the terminal, so callers can compute
/// positions without worrying about a tiny frame. Alignment is the line's own.
pub fn put(frame: &mut Frame, rect: Rect, line: Line) {
    let rect = rect.intersection(frame.area());
    if !rect.is_empty() {
        frame.render_widget(line, rect);
    }
}

/// Draw `text` at the right end of `rect`, cut to fit. Only the cells under the text
/// are touched: a right-aligned `Line` would restyle the whole row.
pub fn put_right(frame: &mut Frame, rect: Rect, text: &str, style: Style) {
    let text = fit(text, rect.width as usize);
    let width = text_width(&text) as u16;
    put(frame, Rect { x: rect.right() - width, width, ..rect }, Line::styled(text, style));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_counts_cells_not_chars() {
        assert_eq!(fit("hello", 5), "hello");
        assert_eq!(fit("hello!", 5), "hell…");
        // Each CJK glyph is two cells: two of them plus the ellipsis is five.
        assert_eq!(fit("日本語のタイトル", 5), "日本…");
        assert_eq!(fit("日本語", 6), "日本語");
        assert_eq!(fit("anything", 0), "");
        assert_eq!(fit("anything", 1), "…");
        for width in 0..40 {
            assert!(text_width(&fit("🎧 emoji 日本語 title that goes on", width)) <= width);
        }
    }
}
