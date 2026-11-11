use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders},
};

use crate::theme;
use crate::tui::logic::palette::{PaletteState, matches};
use crate::tui::logic::state::AppState;
use crate::tui::render::widgets::{fit, put, put_right, text_width};

/// The command palette: the query on the first row, then the matching commands, each
/// with its keys (or `current` / `on`) at the right. In the middle of the screen.
pub fn render_palette(frame: &mut Frame, state: &AppState, palette: &PaletteState) {
    let t = theme::current();
    let screen = frame.area();
    let width = 64.min(screen.width.saturating_sub(2));
    let height = 16.min(screen.height.saturating_sub(3));
    if width < 6 || height < 3 {
        return;
    }
    let area = super::utils::centered_rect_fixed(width, height, screen);
    crate::tui::render::widgets::clear(frame, area);
    let accent = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(t.dim);
    let block = Block::default()
        .title(Line::styled(" Command ", accent))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // One cell of padding either side of the text.
    let row = |dy: u16| Rect { x: inner.x + 1, y: inner.y + dy, width: inner.width - 2, height: 1 };

    put(frame, row(0), Line::styled(format!("› {}▏", palette.query), Style::new().fg(t.fg)));
    if inner.height < 3 {
        return;
    }
    put(frame, Rect { y: inner.y + 1, height: 1, ..inner }, Line::styled("─".repeat(inner.width as usize), dim));

    let found = matches(state, &palette.query);
    if found.is_empty() {
        put(frame, row(2), Line::styled("no matching command", dim));
        return;
    }
    let visible = (inner.height - 2) as usize;
    let selected = palette.selected.min(found.len() - 1);
    // Keep the selection near the middle while there is more list either side.
    let offset = selected.saturating_sub(visible / 2).min(found.len().saturating_sub(visible));
    for (i, (label, keys, _)) in found.iter().enumerate().skip(offset).take(visible) {
        let line = row(2 + (i - offset) as u16);
        let (text, right) = if i == selected {
            let bar = Style::new().bg(t.selection_bg).fg(t.selection_fg).add_modifier(Modifier::BOLD);
            frame.buffer_mut().set_style(Rect { y: line.y, height: 1, ..inner }, bar);
            (bar, bar)
        } else {
            (Style::new().fg(t.fg), dim)
        };
        let keys = fit(keys, line.width as usize / 2);
        let room = (line.width as usize).saturating_sub(text_width(&keys) + 2);
        put(frame, line, Line::styled(fit(label, room), text));
        put_right(frame, line, &keys, right);
    }
}
