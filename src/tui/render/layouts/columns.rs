//! Columns: the location as a path. A breadcrumb row, then Miller columns (the
//! navigation column followed by the location's panes, a window of up to three with
//! the focused one in view), and the player and hint line across the full width
//! underneath.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};
use ratatui_image::thread::ThreadProtocol;

use crate::theme;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::nav::{self, PaneKind};
use crate::tui::logic::state::{AppData, AppState, SEARCHFILTERS, SUBTAB_TITLES, TAB_TITLES};
use crate::tui::render::NowPlaying;
use crate::tui::render::widgets::nav_column::{self, draw_nav_column};
use crate::tui::render::widgets::pane::{PaneStyle, draw_pane, pane_view};
use crate::tui::render::widgets::pane_row::visible_window;
use crate::tui::render::widgets::player::{draw_player, player_size};
use crate::tui::render::widgets::{BRAND_MARK, fit, header, hints, put, put_right, text_width};

use super::{take_bottom, take_top};

/// Terminal columns from which three columns, and two, are shown side by side.
pub const COLS_3: u16 = 100;
pub const COLS_2: u16 = 60;

/// How many columns a terminal `cols` wide shows side by side.
pub fn max_columns(cols: u16) -> usize {
    if cols >= COLS_3 {
        3
    } else if cols >= COLS_2 {
        2
    } else {
        1
    }
}

pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
) {
    let screen = frame.area();
    let mut rest = screen;
    let hint_rows = hints::rows(state, screen);
    take_bottom(&mut rest, hint_rows);
    let size = player_size(false, screen.height);
    let player = take_bottom(&mut rest, size.height());
    draw_player(frame, player, size, state, data, np, cover);
    draw_breadcrumb(frame, take_top(&mut rest, 1), state, data, views);

    // Column 0 is the navigation column; pane `i` is column `i + 1`.
    let panes = nav::panes(state);
    let focus = if nav::in_nav(state) { 0 } else { nav::focus(state) + 1 };
    let (a, b) = visible_window(panes.len() + 1, focus, max_columns(screen.width));
    // The navigation column is only as wide as its rows; the panes share the rest.
    let weight = |i: usize| match panes[i - 1].kind() {
        PaneKind::Tracks => 26u32,
        PaneKind::People => 10,
        _ => 13,
    };
    let nav_width = if a == 0 { nav_column::width(state).min(rest.width) } else { 0 };
    let total: u32 = (a.max(1)..b).map(weight).sum();
    let mut x = rest.x;
    for i in a..b {
        let width = if i + 1 == b {
            rest.right() - x
        } else if i == 0 {
            nav_width
        } else {
            ((rest.width - nav_width) as u32 * weight(i) / total) as u16
        };
        let rect = Rect { x, width, ..rest };
        match i.checked_sub(1) {
            None => draw_nav_column(frame, rect, state),
            Some(pane) => {
                draw_pane(frame, rect, panes[pane], i == focus, PaneStyle { boxed_title: false }, state, data, views, np)
            }
        }
        x += width;
    }
}

/// The path to the focused column: main tab, sub-tab (search type on Search), then,
/// inside a container, the row it was opened from and the pane's own name when it has
/// one (`Friend 0 › liked`).
fn crumbs(state: &AppState, data: &AppData, views: &FilteredViews) -> Vec<String> {
    let (tab, sub) = nav::location(state);
    let mut out = vec![TAB_TITLES[tab].to_string()];
    match tab {
        0 => out.push(SUBTAB_TITLES[sub].to_string()),
        1 => out.push(SEARCHFILTERS[sub].to_string()),
        _ => {}
    }
    let panes = nav::panes(state);
    let focus = nav::focus(state);
    if !nav::in_nav(state) && focus > 0 {
        let parent = pane_view(panes[0], state, data, views).1.label(nav::cursor(state, panes[0])).unwrap_or("—").to_string();
        let title = pane_view(panes[focus], state, data, views).0;
        out.push(parent);
        if out.last() != Some(&title) {
            out.push(title);
        }
    }
    out
}

/// Row 0: `sctui › Library › Playlists › rave tunes`, the last part bold, with the
/// filter or search query at the right. Parts are dropped from the front (`… ›`) when
/// the row is too narrow for the whole path.
fn draw_breadcrumb(frame: &mut Frame, area: Rect, state: &AppState, data: &AppData, views: &FilteredViews) {
    if area.is_empty() {
        return;
    }
    let t = theme::current();
    let (find, find_style) = header::find_text(state);
    let find = fit(&find, area.width as usize / 2);
    let find_w = text_width(&find);
    let room = (area.width as usize).saturating_sub(1 + if find_w > 0 { find_w + 2 } else { 0 });

    let mut parts: Vec<String> = crumbs(state, data, views).iter().map(|part| fit(part, 24)).collect();
    let brand = format!(" {}sctui", BRAND_MARK);
    let width = |parts: &[String], cut: bool| {
        text_width(&brand) + cut as usize * 4 + parts.iter().map(|p| 3 + text_width(p)).sum::<usize>()
    };
    let mut cut = false;
    while parts.len() > 1 && width(&parts, cut) > room {
        parts.remove(0);
        cut = true;
    }
    let dim = Style::new().fg(t.dim);
    let mut spans = vec![Span::styled(brand.clone(), Style::new().fg(t.accent).add_modifier(Modifier::BOLD))];
    if cut {
        spans.push(Span::styled(" › …", dim));
    }
    let last = parts.len() - 1;
    for (i, part) in parts.into_iter().enumerate() {
        spans.push(Span::styled(" › ", dim));
        let style = if i == last { Style::new().fg(t.fg).add_modifier(Modifier::BOLD) } else { Style::new().fg(t.muted) };
        spans.push(Span::styled(part, style));
    }
    put(frame, Rect { width: room as u16, ..area }, Line::from(spans));
    put_right(frame, Rect { width: area.width - 1, ..area }, &find, find_style);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Layout;
    use crate::tui::render::testkit::{fixture, go, render_buffer, render_text, selected_label, text};

    fn columns() -> (AppState, AppData, FilteredViews, NowPlaying) {
        let (mut state, data, views, np) = fixture();
        state.settings.layout = Layout::Columns;
        (state, data, views, np)
    }

    #[test]
    fn breadcrumb_names_location_and_focused_path() {
        let (mut state, mut data, views, np) = columns();
        let mut crumb = |state: &mut AppState, tab, sub, focus, w| {
            go(state, &mut data, tab, sub, focus);
            render_text(w, 24, state, &mut data, &views, &np).lines().next().unwrap().trim_end().to_string()
        };
        assert_eq!(crumb(&mut state, 0, 0, 0, 100), " \u{f1be} sctui › Library › Likes");
        assert_eq!(crumb(&mut state, 0, 1, 0, 100), " \u{f1be} sctui › Library › Playlists");
        assert_eq!(crumb(&mut state, 0, 1, 1, 100), " \u{f1be} sctui › Library › Playlists › Playlist 0");
        assert_eq!(crumb(&mut state, 0, 3, 1, 100), " \u{f1be} sctui › Library › Following › Friend 0 › tracks");
        assert_eq!(crumb(&mut state, 0, 3, 2, 100), " \u{f1be} sctui › Library › Following › Friend 0 › liked");
        assert_eq!(crumb(&mut state, 2, 0, 1, 100), " \u{f1be} sctui › Feed › Poster 0");
        // Search names its type in the path and shows the query at the right.
        let search = crumb(&mut state, 1, 2, 1, 100);
        assert!(search.starts_with(" \u{f1be} sctui › Search › Playlists › Found playlist 0  ") && search.ends_with("  / found"), "{search:?}");
        assert_eq!(search.chars().count(), 99, "one cell short of the edge");
        // Too narrow for the whole path: it loses its front, never its end.
        assert_eq!(crumb(&mut state, 0, 3, 2, 36), " \u{f1be} sctui › … › Friend 0 › liked");
        // With the navigation column focused the path stops at the location.
        state.nav_focused = true;
        assert_eq!(crumb(&mut state, 0, 3, 2, 100), " \u{f1be} sctui › Library › Following");
        state.nav_focused = false;

        // The last part is bold, the earlier ones muted, and a kept filter sits at the
        // right in the warning colour without recolouring the path.
        go(&mut state, &mut data, 0, 1, 1);
        (state.search_popup_visible, state.search_query) = (true, "listed".into());
        let buf = render_buffer(100, 24, &mut state, &mut data, &views, &np);
        let row = text(&buf).lines().next().unwrap().to_string();
        assert!(row.trim_end().ends_with("  / listed"), "{row:?}");
        let t = theme::current();
        let at = |s: &str| row.chars().position(|c| c == s.chars().next().unwrap()).unwrap() as u16;
        assert_eq!((buf[(1, 0)].fg, buf[(at("L"), 0)].fg, buf[(98, 0)].fg), (t.accent, t.muted, t.warning));
        let last = row.find("Playlist 0").map(|byte| row[..byte].chars().count()).unwrap() as u16;
        assert!(buf[(last, 0)].modifier.contains(Modifier::BOLD) && !buf[(at("L"), 0)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn window_keeps_focused_column_visible_at_3_2_1() {
        assert_eq!([100, 99, 60, 59].map(max_columns), [3, 2, 2, 1]);
        let (mut state, mut data, views, np) = columns();
        // Library › Following is four columns: the navigation column and three panes.
        for (w, shown) in [(123, 3), (100, 3), (99, 2), (60, 2), (59, 1), (36, 1)] {
            for column in 0..4usize {
                go(&mut state, &mut data, 0, 3, column.saturating_sub(1));
                state.nav_focused = column == 0;
                let screen = render_text(w, 24, &mut state, &mut data, &views, &np);
                assert_eq!(screen.lines().nth(1).unwrap().matches('╭').count(), shown, "{w} cols, column {column}:\n{screen}");
                let label = selected_label(&state, &data, &views).unwrap();
                assert!(screen.contains(&label), "{w} cols: column {column} ({label}) is not shown:\n{screen}");
                // The focused column's child is shown before its parent.
                let has = |s: &str| screen.contains(s);
                let seen = [has("♪ Likes"), has("Friend 1"), has("Upload 01"), has("Their like 01")];
                let want: [bool; 4] = match (shown, column) {
                    (3, 0 | 1) => [true, true, true, false],
                    (3, _) => [false, true, true, true],
                    (2, 0) => [true, true, false, false],
                    (2, 1) => [false, true, true, false],
                    (2, _) => [false, false, true, true],
                    (_, at) => std::array::from_fn(|i| i == at),
                };
                assert_eq!(seen, want, "{w} cols, column {column}:\n{screen}");
            }
        }
        // The columns fill the rows between the breadcrumb and the player, edge to edge.
        go(&mut state, &mut data, 0, 1, 0);
        state.nav_focused = false;
        let screen = render_text(100, 32, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = screen.lines().collect();
        assert!(lines[1].starts_with("╭───────────────────╮╭ Playlists ─") && lines[1].ends_with('╮') && lines[3].starts_with("│   ♪ Likes"), "{screen}");
        assert!(lines[26].starts_with('╰') && lines[27].starts_with('╭') && lines[28].contains("Song 04"), "{screen}");
        assert!(lines[31].starts_with(" [ ] out / in   enter open"), "{screen}");
        // Sections only: the playlists are a column of their own, not entries of the first.
        assert_eq!(screen.matches("Playlist 0").count(), 2, "its row and its tracks' title:\n{screen}");
    }
}
