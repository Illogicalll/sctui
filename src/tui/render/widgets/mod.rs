//! Drawing shared by every layout: panes, the pane row, the navigation column, the
//! player readouts, the hint line and toasts.

pub mod header;
pub mod hints;
pub mod nav_column;
pub mod pane;
pub mod pane_row;
pub mod player;
pub mod toasts;

use ratatui::{Frame, layout::Rect, style::Style, text::{Line, Span}, widgets::Clear};


/// The Nerd Font SoundCloud mark (`nf-fa-soundcloud`), one cell wide.
pub const SOUNDCLOUD_GLYPH: &str = "\u{f1be}";

/// SoundCloud's terms want it credited as the source wherever its content is shown:
/// its mark, and a space, in front of every `sctui`.
pub const BRAND_MARK: &str = "\u{f1be} ";

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

/// Blank `area` for a popup to draw in. A double-width glyph whose first half is the
/// cell just left of the area would be drawn over the popup's first column by the
/// terminal (and its second cell is never sent), so that glyph is blanked too.
pub fn clear(frame: &mut Frame, area: Rect) {
    let area = area.intersection(frame.area());
    if area.is_empty() {
        return;
    }
    frame.render_widget(Clear, area);
    if area.x > frame.area().x {
        for y in area.top()..area.bottom() {
            let cell = &mut frame.buffer_mut()[(area.x - 1, y)];
            if text_width(cell.symbol()) > 1 {
                cell.set_symbol(" ");
            }
        }
    }
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

    /// A terminal draws a double-width glyph over the cell to its right, so a popup
    /// whose left border lands on that cell must blank the glyph or lose the border.
    #[test]
    fn popup_border_survives_a_wide_glyph_under_it() {
        use crate::tui::render::overlays::utils::centered_rect;
        use crate::tui::render::testkit::{EXTRAS, OVERLAYS, fixture, render_buffer, text};
        // The CJK row's glyphs start on odd columns: at 120 columns the queue popup's
        // border (column 18) is the second half of one; at 122 it is not.
        for w in [120, 121, 122] {
            let (mut state, mut data, views, np) = fixture();
            state.queue_visible = true;
            let buf = render_buffer(w, 30, &mut state, &mut data, &views, &np);
            let popup = centered_rect(70, 60, buf.area);
            let screen = text(&buf);
            assert!(screen.contains("日本語"), "the wide row is on screen:\n{screen}");
            for y in popup.top()..popup.bottom() {
                assert!(text_width(buf[(popup.x - 1, y)].symbol()) <= 1, "{w} cols, row {y}: half a glyph left of the popup\n{screen}");
                assert!(["╭", "│", "╰"].contains(&buf[(popup.x, y)].symbol()), "{w} cols, row {y}:\n{screen}");
                // As the terminal shows it: the row's text has the border where the popup starts.
                let row: String = screen.lines().nth(y as usize).unwrap().chars().collect();
                assert!(row.contains(['╭', '│', '╰']), "{w} cols, row {y}: {row:?}");
            }
            // Every popup and toast: nothing is drawn on the hidden half of a wide glyph.
            for (name, open) in OVERLAYS.iter().chain(EXTRAS) {
                let (mut state, mut data, views, np) = fixture();
                open(&mut state);
                let buf = render_buffer(w, 30, &mut state, &mut data, &views, &np);
                for y in 0..30 {
                    for x in 0..w - 1 {
                        let hidden = text_width(buf[(x, y)].symbol()) > 1 && buf[(x + 1, y)].symbol() != " ";
                        assert!(!hidden, "{name}, {w} cols: ({}, {y}) is under a wide glyph\n{}", x + 1, text(&buf));
                    }
                }
            }
        }
    }
}
