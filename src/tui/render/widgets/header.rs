//! The compact header: two plain rows in place of Classic's boxes. Tighten's header,
//! Classic's on a short terminal, and the left side of the Player panel.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::keymap::Action;
use crate::theme;
use crate::tui::logic::state::{AppData, AppState, SEARCHFILTERS, SUBTAB_TITLES, visible_tabs};

use super::{fit, put, put_right, text_width};

/// Rows the compact header takes.
pub const ROWS: u16 = 2;
/// Columns from which each main tab is prefixed with its key.
pub const COLS_TAB_KEYS: u16 = 70;
/// Columns from which the top row ends with the help key.
pub const COLS_HINTS: u16 = 90;

/// Draw the header in the top rows of `area` and return how many it used. Row 0:
/// `sctui │ 1 Library  2 Search  3 Feed`, with the help key at the right when wide.
/// Row 1: the sub-tabs as pills, the selected one filled, and the filter or search
/// query right-aligned.
pub fn draw_compact(frame: &mut Frame, area: Rect, state: &AppState, _data: &AppData) -> u16 {
    let t = theme::current();
    let accent = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(t.dim);
    let row = |y: u16| Rect { y: area.y + y, height: 1, ..area };
    if area.height == 0 {
        return 0;
    }

    let mut spans = vec![Span::styled(" sctui", accent), Span::styled(" │", dim)];
    let tab_keys = [Action::TabLibrary, Action::TabSearch, Action::TabFeed];
    for (i, title) in visible_tabs(state).iter().enumerate() {
        spans.push(Span::raw("  "));
        let key = state.keymap.hint_label(tab_keys[i]);
        if area.width >= COLS_TAB_KEYS && !key.is_empty() {
            spans.push(Span::styled(format!("{key} "), dim));
        }
        spans.push(Span::styled(*title, if i == state.selected_tab { accent } else { Style::new().fg(t.fg) }));
    }
    put(frame, row(0), Line::from(spans));
    let help = state.keymap.hint_label(Action::Help);
    if area.width >= COLS_HINTS && !help.is_empty() {
        put_right(frame, Rect { width: area.width - 1, ..row(0) }, &format!("{help} keys"), dim);
    }
    if area.height < 2 {
        return 1;
    }

    let (titles, selected): (&[&str], usize) = match state.selected_tab {
        0 => (&SUBTAB_TITLES, state.selected_subtab),
        1 => (&SEARCHFILTERS, state.selected_searchfilter),
        _ => (&[], 0),
    };
    let cursor = if state.search_typing { "▏" } else { "" };
    let (find, find_style) = match state.selected_tab {
        1 if state.query.is_empty() && !state.search_typing => ("/ search".to_string(), Style::new().fg(t.dim)),
        1 => (format!("/ {}{cursor}", state.query), Style::new().fg(t.fg)),
        0 if state.search_popup_visible => (
            format!("/ {}{cursor}", state.search_query),
            Style::new().fg(if state.search_typing { t.fg } else { t.warning }),
        ),
        _ => (String::new(), Style::new()),
    };
    // The query keeps at most half the row; the sub-tabs get what is left of it.
    let find = fit(&find, area.width as usize / 2);
    let room = (area.width as usize).saturating_sub(text_width(&find) + 2);
    let mut spans = Vec::new();
    let mut used = 1;
    for (i, title) in titles.iter().enumerate() {
        let width = title.chars().count() + 2;
        if used + width > room {
            break;
        }
        used += width + 1;
        spans.push(Span::raw(" "));
        let pill = Style::new().fg(t.accent).add_modifier(Modifier::BOLD | Modifier::REVERSED);
        spans.push(Span::styled(format!(" {title} "), if i == selected { pill } else { Style::new().fg(t.muted) }));
    }
    put(frame, row(1), Line::from(spans));
    put_right(frame, Rect { width: area.width.saturating_sub(1), ..row(1) }, &find, find_style);
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::render::testkit::{draw_buffer, fixture, go, text};

    fn header(width: u16, state: &AppState, data: &AppData) -> (Vec<String>, ratatui::buffer::Buffer) {
        let buf = draw_buffer(width, 2, |f| assert_eq!(draw_compact(f, f.area(), state, data), 2));
        (text(&buf).lines().map(|l| l.trim_end().to_string()).collect(), buf)
    }

    #[test]
    fn compact_header_names_tabs_keys_and_the_find_text() {
        let (mut state, mut data, ..) = fixture();
        go(&mut state, &mut data, 0, 1, 0);
        let (lines, buf) = header(120, &state, &data);
        assert!(lines[0].starts_with(" sctui │  1 Library  2 Search  3 Feed") && lines[0].ends_with("? keys"), "{lines:?}");
        assert_eq!(lines[1], "  Likes   Playlists   Albums   Following");
        // The selected sub-tab is the filled pill; the selected main tab is accented.
        let x = lines[1].find("Playlists").unwrap() as u16;
        let t = theme::current();
        assert!(buf[(x, 1)].modifier.contains(Modifier::REVERSED) && !buf[(2, 1)].modifier.contains(Modifier::REVERSED));
        assert_eq!((buf[(12, 0)].fg, buf[(23, 0)].fg), (t.accent, t.fg));

        // Narrower: no help key, then no tab numbers. The keys are the live ones.
        assert_eq!(header(89, &state, &data).0[0], " sctui │  1 Library  2 Search  3 Feed");
        assert_eq!(header(69, &state, &data).0[0], " sctui │  Library  Search  Feed");
        state.keymap.apply_toml("[keys]\ntab_search = \"F2\"\ntab_feed = []\n");
        assert_eq!(header(89, &state, &data).0[0], " sctui │  1 Library  f2 Search  Feed");
        state.settings.hide_feed_tab = true;
        assert_eq!(header(89, &state, &data).0[0], " sctui │  1 Library  f2 Search");

        // A kept Library filter and the Search query sit at the right of row 1.
        (state.search_popup_visible, state.search_query) = (true, "listed".into());
        let (lines, buf) = header(80, &state, &data);
        assert!(lines[1].starts_with("  Likes   Playlists   Albums   Following   ") && lines[1].ends_with(" / listed"), "{lines:?}");
        assert_eq!(lines[1].chars().count(), 79, "one cell short of the edge");
        assert_eq!((buf[(2, 1)].fg, buf[(78, 1)].fg), (t.muted, t.warning), "the filter text does not recolour the pills");
        go(&mut state, &mut data, 1, 3, 0);
        let (lines, _) = header(80, &state, &data);
        assert!(lines[1].starts_with("  Tracks   Albums   Playlists   People") && lines[1].ends_with("/ found"), "{lines:?}");
        // One row of room draws one row; none draws nothing.
        let buf = draw_buffer(80, 1, |f| assert_eq!(draw_compact(f, f.area(), &state, &data), 1));
        assert!(text(&buf).starts_with(" sctui │"));
        draw_buffer(80, 2, |f| assert_eq!(draw_compact(f, Rect::new(0, 0, 80, 0), &state, &data), 0));
    }
}
