//! The panes of the current location side by side, as many as the width allows.

use ratatui::{Frame, layout::Rect, style::Style, text::Line};

use crate::theme;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::nav::{self, PaneKind};
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::render::NowPlaying;

use super::pane::{PaneStyle, counter, draw_pane, pane_view};
use super::{fit, put, text_width};

/// Pane-row width from which three panes, and two, are shown side by side.
pub const PANES_3: u16 = 104;
pub const PANES_2: u16 = 64;

/// How many panes fit side by side in a row `width` cells wide.
pub fn max_panes(width: u16) -> usize {
    if width >= PANES_3 {
        3
    } else if width >= PANES_2 {
        2
    } else {
        1
    }
}

/// Which of `n` panes to show when at most `max` fit: a half-open range that always
/// holds `focus`, never starts before `first`, and prefers to show the focused
/// pane's child (the pane to its right) over its parent.
pub fn visible_window(n: usize, focus: usize, first: usize, max: usize) -> (usize, usize) {
    let max = max.max(1);
    if n.saturating_sub(first) <= max {
        return (first.min(n), n);
    }
    let child = (focus + 1 < n && max > 1) as usize;
    let start = (focus + child).saturating_sub(max - 1).max(first).min(n - max);
    (start, start + max)
}

/// Draw the current location's panes in `area`. `weights` gives each pane's share of
/// the width, indexed like `nav::panes`; without it panes are weighted by kind. Panes
/// that do not fit are named on the bottom border (`‹ Playlists`, `liked ›`).
#[allow(clippy::too_many_arguments)]
pub fn draw_pane_row(
    frame: &mut Frame,
    area: Rect,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    style: PaneStyle,
    weights: Option<&[u16]>,
    np: &NowPlaying,
) {
    let area = area.intersection(frame.area());
    if area.is_empty() {
        return;
    }
    let panes = nav::panes(state);
    let focus = nav::focus(state);
    // Sidebar lists the playlists itself, and no pane is lit while its column has focus.
    let first = nav::first_pane(state);
    let lit = !nav::in_nav(state);
    let (a, b) = visible_window(panes.len(), focus, first, max_panes(area.width));
    let weight = |i: usize| {
        weights.and_then(|w| w.get(i).copied()).unwrap_or(match panes[i].kind() {
            PaneKind::Tracks => 23,
            PaneKind::People => 9,
            PaneKind::Feed => 16,
            _ => 12,
        }) as u32
    };
    let total: u32 = (a..b).map(weight).sum::<u32>().max(1);
    let dim = Style::new().fg(theme::current().dim);
    let mut x = area.x;
    for i in a..b {
        let width = if i + 1 == b { area.right() - x } else { (area.width as u32 * weight(i) / total) as u16 };
        let rect = Rect::new(x, area.y, width, area.height);
        draw_pane(frame, rect, panes[i], lit && i == focus, style, state, data, views, np);

        // Name what is off-screen on the bottom border, clear of the counter.
        let bottom = area.bottom() - 1;
        let mut left_end = rect.x + 1;
        if i == a && a > first && rect.height >= 3 {
            let label = format!("‹ {}", fit(&pane_view(panes[a - 1], state, data, views).0, 10));
            let w = (text_width(&label) as u16).min(rect.width.saturating_sub(4));
            put(frame, Rect::new(rect.x + 2, bottom, w, 1), Line::styled(label, dim));
            left_end = rect.x + 2 + w;
        }
        if i + 1 == b && b < panes.len() && rect.height >= 3 {
            let label = format!("{} ›", fit(&pane_view(panes[b], state, data, views).0, 10));
            let w = text_width(&label) as u16;
            let count = counter(nav::cursor(state, panes[i]), nav::len(state, data, views, panes[i]));
            let end = rect.right().saturating_sub(2 + text_width(&count) as u16);
            if end >= left_end + 1 + w {
                put(frame, Rect::new(end - w, bottom, w, 1), Line::styled(label, dim));
            }
        }
        x += width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::render::testkit::{draw_buffer, fixture, go, selected_label, text};

    #[test]
    fn visible_window_cases() {
        // Everything fits.
        assert_eq!(visible_window(3, 0, 0, 3), (0, 3));
        assert_eq!(visible_window(2, 1, 0, 3), (0, 2));
        assert_eq!(visible_window(1, 0, 0, 1), (0, 1));
        // Two of three: the focused pane and, when it has one, its child.
        assert_eq!(visible_window(3, 0, 0, 2), (0, 2));
        assert_eq!(visible_window(3, 1, 0, 2), (1, 3));
        assert_eq!(visible_window(3, 2, 0, 2), (1, 3));
        // One at a time: the focused pane.
        assert_eq!(visible_window(3, 0, 0, 1), (0, 1));
        assert_eq!(visible_window(3, 1, 0, 1), (1, 2));
        assert_eq!(visible_window(3, 2, 0, 1), (2, 3));
        assert_eq!(visible_window(2, 1, 0, 1), (1, 2));
        // A layout that draws the first pane elsewhere (Sidebar's playlists) starts at 1.
        assert_eq!(visible_window(2, 1, 1, 1), (1, 2));
        assert_eq!(visible_window(2, 1, 1, 3), (1, 2));
        assert_eq!(visible_window(3, 1, 1, 1), (1, 2));
        assert_eq!(visible_window(3, 2, 1, 1), (2, 3));
        assert_eq!(visible_window(4, 1, 1, 2), (1, 3));
        // Degenerate input is clamped, never a panic.
        assert_eq!(visible_window(0, 0, 0, 3), (0, 0));
        assert_eq!(visible_window(3, 1, 0, 0), (1, 2));
        assert_eq!(visible_window(2, 0, 5, 1), (2, 2));
    }

    /// Draw Library › Following's pane row alone at `width`, with pane `focus` focused.
    fn following(width: u16, focus: usize) -> (String, String) {
        let (mut state, mut data, views, np) = fixture();
        go(&mut state, &mut data, 0, 3, focus);
        let buf = draw_buffer(width, 16, |f| {
            draw_pane_row(f, f.area(), &mut state, &mut data, &views, PaneStyle { boxed_title: true }, Some(&[20, 40, 40]), &np)
        });
        (text(&buf), selected_label(&state, &data, &views).unwrap())
    }

    #[test]
    fn pane_count_follows_width() {
        for (width, panes) in [(110, 3), (104, 3), (103, 2), (80, 2), (64, 2), (63, 1), (50, 1)] {
            for focus in 0..3 {
                let (screen, label) = following(width, focus);
                let top = screen.lines().next().unwrap();
                assert_eq!(top.matches('╭').count(), panes, "{width} cols, focus {focus}:\n{screen}");
                assert!(screen.contains(&label), "{width} cols: focused pane {focus} is not shown:\n{screen}");
            }
        }
        // One pane at a time: both neighbours are named on the border.
        let (screen, _) = following(50, 1);
        let bottom = screen.lines().last().unwrap();
        assert!(bottom.contains("‹ Following") && bottom.contains("liked ›"), "{screen}");
        // Two panes: focusing the middle one shows it with its child and names the parent.
        let (screen, _) = following(80, 1);
        let bottom = screen.lines().last().unwrap();
        assert!(bottom.contains("‹ Following") && !bottom.contains('›'), "{screen}");
        let (screen, _) = following(80, 0);
        assert!(screen.lines().last().unwrap().contains("liked ›"), "{screen}");
    }

    #[test]
    fn unfocused_pane_has_no_selection_bar() {
        let (mut state, mut data, views, np) = fixture();
        go(&mut state, &mut data, 0, 1, 1);
        let buf = draw_buffer(120, 16, |f| {
            draw_pane_row(f, f.area(), &mut state, &mut data, &views, PaneStyle { boxed_title: true }, Some(&[33, 67]), &np)
        });
        let t = theme::current();
        let split = 120 * 33 / 100;
        let bar: Vec<u16> = (0..120).filter(|&x| (0..16).any(|y| buf[(x, y)].bg == t.selection_bg)).collect();
        assert!(!bar.is_empty(), "the focused pane has no selection bar");
        assert!(bar.iter().all(|&x| x >= split), "the unfocused pane has a selection bar: {bar:?}");
        // The unfocused pane marks its row instead, and only the focused one has the accent border.
        let screen = text(&buf);
        assert!(screen.lines().any(|l| l.starts_with("│›")), "{screen}");
        assert_eq!(buf[(split, 0)].fg, t.accent);
        assert_eq!(buf[(0, 0)].fg, t.dim);
    }
}
