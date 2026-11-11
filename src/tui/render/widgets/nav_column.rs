//! The navigation column of Sidebar and Columns: the Library sections, Search and the
//! Feed, with the user's playlists listed under Playlists when the layout asks.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders},
};

use crate::theme;
use crate::tui::logic::nav::{self, NavItem};
use crate::tui::logic::state::{AppData, AppState};

use super::{brand_mark, fit, put};

/// Columns the navigation column takes.
pub const WIDTH: u16 = 26;

fn icon(item: &NavItem) -> &'static str {
    match (item.tab, item.sub) {
        (0, Some(0)) => "♪",
        (0, Some(1)) => "≡",
        (0, Some(2)) => "◉",
        (0, _) => "☺",
        (1, _) => "/",
        _ => "~",
    }
}

/// Draw the column in `area`. The current entry carries the selection bar while the
/// column has focus and a dim bar while it does not, and the list scrolls to keep it
/// in view. Playlist entries are indented under `Playlists`.
pub fn draw_nav_column(frame: &mut Frame, area: Rect, state: &AppState, data: &AppData, with_playlists: bool) {
    let area = area.intersection(frame.area());
    if area.width < 4 || area.height < 3 {
        return;
    }
    let t = theme::current();
    let focused = nav::in_nav(state);
    let block = Block::default()
        .title(Line::styled(format!(" {}sctui ", brand_mark(state)), Style::new().fg(t.accent).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused { t.accent } else { t.dim }));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items = nav::nav_items(state, data, with_playlists);
    let at = nav::nav_index(state, data, &items);
    let visible = inner.height as usize;
    let offset = at.saturating_sub(visible / 2).min(items.len().saturating_sub(visible));
    for (i, item) in items.iter().enumerate().skip(offset).take(visible) {
        let row = Rect { y: inner.y + (i - offset) as u16, height: 1, ..inner };
        let style = if i == at && focused {
            Style::new().bg(t.selection_bg).fg(t.selection_fg).add_modifier(Modifier::BOLD)
        } else if i == at {
            Style::new().bg(t.dim).fg(t.fg)
        } else if item.playlist_uri.is_some() {
            Style::new().fg(t.muted)
        } else {
            Style::new().fg(t.fg)
        };
        if i == at {
            frame.buffer_mut().set_style(row, style);
        }
        let text = match item.playlist_uri {
            Some(_) => format!("   {}", item.label),
            None => format!("{} {}", icon(item), item.label),
        };
        let cell = Rect { x: row.x + 1, width: row.width - 2, ..row };
        put(frame, cell, Line::styled(fit(&text, cell.width as usize), style));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Playlist;
    use crate::config::Layout;
    use crate::tui::render::testkit::{draw_buffer, fixture, go, text};

    fn column(height: u16, state: &AppState, data: &AppData, with_playlists: bool) -> (Vec<String>, ratatui::buffer::Buffer) {
        let buf = draw_buffer(WIDTH, height, |f| draw_nav_column(f, f.area(), state, data, with_playlists));
        (text(&buf).lines().map(|l| l.trim_end().to_string()).collect(), buf)
    }

    #[test]
    fn nav_column_lists_sections_and_playlists_and_marks_the_current_one() {
        let (mut state, mut data, ..) = fixture();
        state.settings.layout = Layout::Sidebar;
        let (lines, buf) = column(16, &state, &data, true);
        assert!(lines[0].starts_with("╭ sctui ─"), "{lines:?}");
        let want = [
            "│ ♪ Likes", "│ ≡ Playlists", "│    Playlist 0", "│    Playlist 1", "│    Playlist 2",
            "│    Playlist 3", "│    Playlist 4", "│ ◉ Albums", "│ ☺ Following", "│ / Search", "│ ~ Feed",
        ];
        for (line, want) in lines[1..].iter().zip(want) {
            assert!(line.starts_with(want), "{line:?} is not {want:?}");
        }
        // Unfocused: a dim border and a dim bar on the current entry (Likes).
        let t = theme::current();
        assert_eq!((buf[(0, 0)].fg, buf[(3, 1)].bg, buf[(3, 2)].bg), (t.dim, t.dim, ratatui::style::Color::Reset));
        // Focused on a playlist: accent border, the selection bar on that playlist.
        go(&mut state, &mut data, 0, 1, 1);
        state.nav_focused = true;
        *nav::cursor_mut(&mut state, nav::PaneId::Playlists) = 3;
        let (_, buf) = column(16, &state, &data, true);
        assert_eq!((buf[(0, 0)].fg, buf[(3, 6)].bg, buf[(3, 2)].bg), (t.accent, t.selection_bg, ratatui::style::Color::Reset));
        // Sections only (Columns), and without the Feed when it is hidden.
        state.settings.hide_feed_tab = true;
        let (lines, _) = column(16, &state, &data, false);
        let labels: Vec<&str> = lines[1..6].iter().map(|l| l.trim_matches(['│', ' '])).collect();
        assert_eq!(labels, ["♪ Likes", "≡ Playlists", "◉ Albums", "☺ Following", "/ Search"]);
        assert_eq!(lines[6].trim_matches(['│', ' ']), "");
    }

    #[test]
    fn nav_column_scrolls_to_the_current_entry_and_cuts_long_names() {
        let (mut state, mut data, ..) = fixture();
        state.settings.layout = Layout::Sidebar;
        let template = data.playlists[0].clone();
        data.playlists = (0..40)
            .map(|i| Playlist {
                title: format!("{i:02} {}", crate::tui::render::testkit::WIDE_TITLES[i % 3]),
                tracks_uri: format!("uri:{i}"),
                ..template.clone()
            })
            .collect();
        go(&mut state, &mut data, 0, 1, 1);
        state.nav_focused = true;
        for at in [0, 17, 39] {
            *nav::cursor_mut(&mut state, nav::PaneId::Playlists) = at;
            for height in [3, 4, 8, 24] {
                let (lines, buf) = column(height, &state, &data, true);
                assert!(lines.iter().any(|l| l.contains(&format!("   {at:02} "))), "{at} at {height} rows: {lines:?}");
                // Every row keeps its right border, however wide the name.
                for y in 1..height - 1 {
                    assert_eq!(buf[(WIDTH - 1, y)].symbol(), "│", "row {y}: {lines:?}");
                }
            }
        }
        // The current entry sits mid-box while there is more list either side of it,
        // and the end of the list is never scrolled past.
        let (lines, _) = column(8, &state, &data, true);
        assert!(lines[4].contains("   39 ") && lines[6].contains("☺ Following"), "{lines:?}");
        go(&mut state, &mut data, 2, 0, 0);
        let (lines, _) = column(8, &state, &data, true);
        assert!(lines[6].contains("~ Feed") && lines[1].contains("   38 "), "{lines:?}");
        // Too small to draw is not a panic.
        for (w, h) in [(3, 10), (26, 2), (0, 0)] {
            draw_buffer(26, 10, |f| draw_nav_column(f, Rect::new(0, 0, w, h), &state, &data, true));
        }
    }
}
