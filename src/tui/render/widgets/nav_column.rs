//! The navigation column of Columns: the Library sections, Search and the Feed.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders},
};

use crate::keymap::Action;
use crate::theme;
use crate::tui::logic::nav::{self, NavItem};
use crate::tui::logic::state::{AppState, TAB_TITLES};

use super::{fit, put, text_width};

fn icon(item: &NavItem) -> &'static str {
    match (item.tab, item.sub) {
        (0, Some(0)) | (1, Some(0)) => "♪",
        (0, Some(1)) | (1, Some(2)) => "≡",
        (0, Some(2)) | (1, Some(1)) => "◉",
        _ => "☺",
    }
}

/// `[1] Library`: the tab's key, when it has one, then its name.
fn tab_title(state: &AppState, tab: usize) -> String {
    let action = [Action::TabLibrary, Action::TabSearch, Action::TabFeed][tab.min(2)];
    match state.keymap.hint_labels(action).split(", ").next() {
        Some(key) if !key.is_empty() => format!("[{key}] {}", TAB_TITLES[tab.min(2)]),
        _ => TAB_TITLES[tab.min(2)].to_string(),
    }
}

/// An entry's text: a section indented under its tab's heading, or a whole tab.
fn entry_text(state: &AppState, item: &NavItem) -> String {
    match item.sub {
        Some(_) => format!("  {} {}", icon(item), item.label),
        None => tab_title(state, item.tab),
    }
}

/// Columns the column takes: its widest row, a cell of padding each side, the border,
/// and four cells of air so the longest row does not touch the edge.
pub fn width(state: &AppState) -> u16 {
    let entries = nav::nav_items(state);
    let rows = entries.iter().map(|item| entry_text(state, item)).chain((0..3).map(|tab| tab_title(state, tab)));
    rows.map(|row| text_width(&row)).max().unwrap_or(0) as u16 + 8
}

/// One line of the column: a main tab's heading, which is not an entry, or an entry.
enum Row<'a> {
    Heading(usize),
    Entry(usize, &'a NavItem),
}

/// Draw the column in `area`. The current entry carries the selection bar while the
/// column has focus and a dim bar while it does not, and the list scrolls to keep it
/// in view. A main tab that is split into sections gets a heading with its tab key
/// (`[1] Library`) and its sections indented beneath; a tab with a single entry
/// carries the key itself (`[3] Feed`).
pub fn draw_nav_column(frame: &mut Frame, area: Rect, state: &AppState) {
    let area = area.intersection(frame.area());
    if area.width < 4 || area.height < 3 {
        return;
    }
    let t = theme::current();
    let focused = nav::in_nav(state);
    // No title: the breadcrumb above the columns already carries `sctui` and its mark.
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { t.accent } else { t.dim }));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items = nav::nav_items(state);
    let at = nav::nav_index(state, &items);
    let mut rows = Vec::with_capacity(items.len() + 2);
    for (i, item) in items.iter().enumerate() {
        if item.sub.is_some() && (i == 0 || items[i - 1].tab != item.tab) {
            rows.push(Row::Heading(item.tab));
        }
        rows.push(Row::Entry(i, item));
    }
    let at_row = rows.iter().position(|row| matches!(row, Row::Entry(i, _) if *i == at)).unwrap_or(0);
    let visible = inner.height as usize;
    let offset = at_row.saturating_sub(visible / 2).min(rows.len().saturating_sub(visible));
    for (y, row) in rows.iter().skip(offset).take(visible).enumerate() {
        let line = Rect { y: inner.y + y as u16, height: 1, ..inner };
        let cell = Rect { x: line.x + 1, width: line.width - 2, ..line };
        let (text, style) = match row {
            Row::Heading(tab) => (tab_title(state, *tab), Style::new().fg(t.fg).add_modifier(Modifier::BOLD)),
            Row::Entry(i, item) => {
                let style = if *i == at && focused {
                    Style::new().bg(t.selection_bg).fg(t.selection_fg).add_modifier(Modifier::BOLD)
                } else if *i == at {
                    Style::new().bg(t.dim).fg(t.fg)
                } else {
                    Style::new().fg(t.fg)
                };
                if *i == at {
                    frame.buffer_mut().set_style(line, style);
                }
                (entry_text(state, item), style)
            }
        };
        put(frame, cell, Line::styled(fit(&text, cell.width as usize), style));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Layout;
    use crate::tui::render::testkit::{draw_buffer, fixture, go, text};

    const WIDTH: u16 = 26;

    fn column(height: u16, state: &AppState) -> (Vec<String>, ratatui::buffer::Buffer) {
        let buf = draw_buffer(WIDTH, height, |f| draw_nav_column(f, f.area(), state));
        (text(&buf).lines().map(|l| l.trim_end().to_string()).collect(), buf)
    }

    #[test]
    fn nav_column_lists_sections_and_marks_the_current_one() {
        let (mut state, mut data, ..) = fixture();
        state.settings.layout = Layout::Columns;
        let (lines, buf) = column(20, &state);
        assert!(lines[0].starts_with("╭──") && !lines[0].contains("sctui"), "the breadcrumb has the brand: {lines:?}");
        assert_eq!(width(&state), 21, "`  ≡ Playlists`, padding, border and air");
        // No type pills beside it, so the search types are listed under Search.
        let want = [
            "│ [1] Library", "│   ♪ Likes", "│   ≡ Playlists", "│   ◉ Albums", "│   ☺ Following",
            "│ [2] Search", "│   ♪ Tracks", "│   ◉ Albums", "│   ≡ Playlists", "│   ☺ People", "│ [3] Feed",
        ];
        for (line, want) in lines[1..].iter().zip(want) {
            assert!(line.starts_with(want), "{line:?} is not {want:?}");
        }
        // Unfocused: a dim border and a dim bar on the current entry (Likes).
        let t = theme::current();
        assert_eq!((buf[(0, 0)].fg, buf[(3, 2)].bg, buf[(3, 3)].bg), (t.dim, t.dim, ratatui::style::Color::Reset));
        // Focused on Playlists: accent border, the selection bar on that entry.
        go(&mut state, &mut data, 0, 1, 1);
        state.nav_focused = true;
        let (_, buf) = column(16, &state);
        assert_eq!((buf[(0, 0)].fg, buf[(3, 3)].bg, buf[(3, 2)].bg), (t.accent, t.selection_bg, ratatui::style::Color::Reset));
        // Without the Feed when it is hidden.
        state.settings.hide_feed_tab = true;
        let (lines, _) = column(16, &state);
        assert!(lines[10].starts_with("│   ☺ People"), "{lines:?}");
        assert_eq!(lines[11].trim_matches(['│', ' ']), "");
    }

    #[test]
    fn nav_column_scrolls_to_the_current_entry() {
        let (mut state, mut data, ..) = fixture();
        state.settings.layout = Layout::Columns;
        for (tab, sub, label) in [(0, 0, "♪ Likes"), (0, 3, "☺ Following"), (1, 3, "☺ People"), (2, 0, "[3] Feed")] {
            go(&mut state, &mut data, tab, sub, 0);
            state.nav_focused = true;
            for height in [3, 4, 8, 24] {
                let (lines, buf) = column(height, &state);
                assert!(lines.iter().any(|l| l.contains(label)), "{label} at {height} rows: {lines:?}");
                for y in 1..height - 1 {
                    assert_eq!(buf[(WIDTH - 1, y)].symbol(), "│", "row {y}: {lines:?}");
                }
            }
        }
        // The end of the list is never scrolled past, and the current entry sits
        // mid-box while there is more list either side of it.
        let (lines, _) = column(8, &state);
        assert!(lines[6].contains("[3] Feed") && lines[1].contains("[2] Search"), "{lines:?}");
        go(&mut state, &mut data, 0, 3, 0);
        let (lines, _) = column(8, &state);
        assert!(lines[4].contains("☺ Following") && lines[1].contains("♪ Likes"), "{lines:?}");
        // Too small to draw is not a panic.
        for (w, h) in [(3, 10), (26, 2), (0, 0)] {
            draw_buffer(26, 10, |f| draw_nav_column(f, Rect::new(0, 0, w, h), &state));
        }
    }
}
