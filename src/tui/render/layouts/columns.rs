//! Columns: the location as a path. A breadcrumb row, then Miller columns (the
//! navigation column followed by the location's panes, a window of up to three with
//! the focused one in view), a track card on the right edge of a very wide terminal,
//! and the player and hint line across the full width underneath.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};
use ratatui_image::thread::ThreadProtocol;

use crate::keymap::Action;
use crate::theme;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::nav::{self, PaneKind};
use crate::tui::logic::state::{AppData, AppState, SEARCHFILTERS, SUBTAB_TITLES, TAB_TITLES};
use crate::tui::render::NowPlaying;
use crate::tui::render::widgets::nav_column::draw_nav_column;
use crate::tui::render::widgets::pane::{PaneStyle, draw_pane, pane_view};
use crate::tui::render::widgets::pane_row::visible_window;
use crate::tui::render::widgets::player::{draw_player, player_size};
use crate::tui::render::widgets::{brand_mark, fit, header, hints, put, put_right, text_width};

use super::{take_bottom, take_top};

/// Terminal columns from which three columns, and two, are shown side by side.
pub const COLS_3: u16 = 100;
pub const COLS_2: u16 = 60;
/// Terminal columns from which the track card is shown, and the columns it takes.
pub const COLS_CARD: u16 = 124;
pub const CARD_WIDTH: u16 = 30;

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
    let (a, b) = visible_window(panes.len() + 1, focus, 0, max_columns(screen.width));
    let on_tracks = focus > 0 && panes[focus - 1].kind() == PaneKind::Tracks;
    if screen.width >= COLS_CARD && on_tracks && focus + 1 == b {
        rest.width -= CARD_WIDTH;
        draw_track_card(frame, Rect { x: rest.right(), width: CARD_WIDTH, ..rest }, state, data, views, np);
    }
    let weight = |i: usize| match i.checked_sub(1).map(|pane| panes[pane].kind()) {
        None => 8u32,
        Some(PaneKind::Tracks) => 26,
        Some(PaneKind::People) => 10,
        Some(_) => 13,
    };
    let total: u32 = (a..b).map(weight).sum();
    let mut x = rest.x;
    for i in a..b {
        let width = if i + 1 == b { rest.right() - x } else { (rest.width as u32 * weight(i) / total) as u16 };
        let rect = Rect { x, width, ..rest };
        match i.checked_sub(1) {
            None => draw_nav_column(frame, rect, state, data, false),
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
    let brand = format!(" {}sctui", brand_mark(state));
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

/// The focused pane's selected track in full, down the right edge: title, artist,
/// duration, streams, whether it is liked, and the keys that act on it. Text only: the
/// one cover image is the playing track's and the player below has it, so the playing
/// track is marked with its state instead.
fn draw_track_card(frame: &mut Frame, area: Rect, state: &AppState, data: &AppData, views: &FilteredViews, np: &NowPlaying) {
    let t = theme::current();
    let block = Block::default()
        .title(Line::styled(" Track ", Style::new().fg(t.muted)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.dim));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(track) = nav::selected_track(state, data, views) else { return };
    if inner.width < 6 {
        return;
    }
    let text = Rect { x: inner.x + 1, width: inner.width - 2, ..inner };
    let width = text.width as usize;
    let mut y = text.y + 1;
    // The next free row; `None` once the card is full.
    let mut row = |skip: u16| {
        y += skip;
        y += 1;
        (y <= text.bottom()).then_some(Rect { y: y - 1, height: 1, ..text })
    };
    let accent = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(t.dim);

    if np.has_track && track.track_urn == np.track.track_urn
        && let Some(r) = row(0)
    {
        put(frame, r, Line::styled(if np.playing { "▶ playing" } else { "⏸ paused" }, accent).centered());
    }
    if let Some(r) = row(0) {
        put(frame, r, Line::styled(fit(&track.title, width), Style::new().fg(t.fg).add_modifier(Modifier::BOLD)).centered());
    }
    if let Some(r) = row(0) {
        put(frame, r, Line::styled(fit(&track.artists, width), Style::new().fg(t.muted)).centered());
    }
    let liked = if data.liked_track_urns.contains(&track.track_urn) { "yes" } else { "no" };
    let facts = [("Duration", track.duration.as_str()), ("Streams", track.playback_count.as_str()), ("Liked", liked)];
    for (i, (name, value)) in facts.into_iter().enumerate() {
        if let Some(r) = row((i == 0) as u16) {
            put(frame, r, Line::styled(name, dim));
            put_right(frame, Rect { x: r.x + 9, width: r.width.saturating_sub(9), ..r }, value, Style::new().fg(t.fg));
        }
    }
    let keys = [(Action::StartStation, "start station"), (Action::AddToPlaylist, "add to playlist"), (Action::AddToQueue, "add to queue")];
    let mut gap = 1;
    for (action, what) in keys {
        let key = state.keymap.hint_label(action);
        if !key.is_empty()
            && let Some(r) = row(std::mem::take(&mut gap))
        {
            put(frame, r, Line::from(vec![Span::styled(key, accent), Span::styled(format!(" {what}"), dim)]));
        }
    }
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
        assert_eq!(crumb(&mut state, 0, 0, 0, 100), " sctui › Library › Likes");
        assert_eq!(crumb(&mut state, 0, 1, 0, 100), " sctui › Library › Playlists");
        assert_eq!(crumb(&mut state, 0, 1, 1, 100), " sctui › Library › Playlists › Playlist 0");
        assert_eq!(crumb(&mut state, 0, 3, 1, 100), " sctui › Library › Following › Friend 0 › tracks");
        assert_eq!(crumb(&mut state, 0, 3, 2, 100), " sctui › Library › Following › Friend 0 › liked");
        assert_eq!(crumb(&mut state, 2, 0, 1, 100), " sctui › Feed › Poster 0");
        // Search names its type in the path and shows the query at the right.
        let search = crumb(&mut state, 1, 2, 1, 100);
        assert!(search.starts_with(" sctui › Search › Playlists › Found playlist 0  ") && search.ends_with("  / found"), "{search:?}");
        assert_eq!(search.chars().count(), 99, "one cell short of the edge");
        // Too narrow for the whole path: it loses its front, never its end.
        assert_eq!(crumb(&mut state, 0, 3, 2, 36), " sctui › … › Friend 0 › liked");
        // With the navigation column focused the path stops at the location.
        state.nav_focused = true;
        assert_eq!(crumb(&mut state, 0, 3, 2, 100), " sctui › Library › Following");
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
        assert!(lines[1].starts_with("╭ sctui ─") && lines[1].ends_with('╮') && lines[2].starts_with("│ ♪ Likes"), "{screen}");
        assert!(lines[26].starts_with('╰') && lines[27].starts_with('╭') && lines[28].contains("Song 04"), "{screen}");
        assert!(lines[31].starts_with(" [ ] out / in   enter open"), "{screen}");
        // Sections only: the playlists are a column of their own, not entries of the first.
        assert_eq!(screen.matches("Playlist 0").count(), 2, "its row and its tracks' title:\n{screen}");
    }

    #[test]
    fn track_card_only_from_124_cols_and_tracks_focus() {
        let (mut state, mut data, views, np) = columns();
        let card = |state: &mut AppState, data: &mut AppData, tab, sub, focus, w| {
            go(state, data, tab, sub, focus);
            let screen = render_text(w, 32, state, data, &views, &np);
            (screen.contains("╭ Track ─"), screen)
        };
        let data = &mut data;
        assert!(card(&mut state, data, 0, 1, 1, 124).0);
        assert!(!card(&mut state, data, 0, 1, 1, 123).0);
        assert!(!card(&mut state, data, 0, 1, 0, 160).0, "a list of playlists is focused");
        assert!(!card(&mut state, data, 0, 3, 1, 160).0, "the focused tracks are not the last column");
        assert!(card(&mut state, data, 0, 3, 2, 160).0);
        assert!(card(&mut state, data, 0, 0, 0, 160).0 && card(&mut state, data, 1, 0, 0, 160).0);
        state.nav_focused = true;
        assert!(!card(&mut state, data, 0, 0, 0, 160).0, "the navigation column is focused");
        state.nav_focused = false;

        // The last 30 cells of the column rows, with the track in full and the live keys.
        state.keymap.apply_toml("[keys]\nadd_to_queue = \"z\"\nadd_to_playlist = []\n");
        let (_, screen) = card(&mut state, data, 0, 1, 1, 130);
        let lines: Vec<&str> = screen.lines().collect();
        let cell = |y: usize| lines[y].chars().skip(100).collect::<String>();
        assert_eq!(cell(1), format!("╭ Track {}╮", "─".repeat(21)), "{screen}");
        assert!(cell(26).starts_with('╰') && lines[27].ends_with('╮'), "the player spans the width:\n{screen}");
        let rows: Vec<String> = (2..13).map(|y| cell(y).trim_matches(['│', ' ']).to_string()).collect();
        let squeezed: Vec<String> = rows.iter().map(|r| r.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
        assert_eq!(
            squeezed,
            ["", "Listed 00", "Artist 2", "", "Duration 02:40", "Streams 101.0K", "Liked no", "", "S start station", "z add to queue", ""],
            "{screen}"
        );
        // An empty pane has an empty card.
        data.playlist_tracks.clear();
        let (shown, screen) = card(&mut state, data, 0, 1, 1, 130);
        assert!(shown && !screen.contains("Duration"), "{screen}");
    }

    #[test]
    fn track_card_has_no_cover_unless_selected_is_playing() {
        let (mut state, mut data, views, mut np) = columns();
        let mut card = |state: &mut AppState, np: &NowPlaying| {
            // By cell: the Likes rows beside the card hold double-width titles.
            let buf = render_buffer(130, 32, state, &mut data, &views, np);
            let row = |y: u16| (101..129).map(|x| buf[(x, y)].symbol()).collect::<String>().trim().to_string();
            [2, 3, 4, 5].map(row)
        };
        // Text from the top: no rows are kept for an image, whatever is selected.
        assert_eq!(card(&mut state, &np)[..3], ["", "Song 00", "Artist 0"]);
        // The selected track is the playing one: the card says so in place of a cover
        // (the image itself stays in the player bar, the one place that draws it).
        *nav::cursor_mut(&mut state, nav::PaneId::Likes) = 4;
        assert_eq!(card(&mut state, &np), ["", "▶ playing", "Song 04", "Artist 4"]);
        np.playing = false;
        assert_eq!(card(&mut state, &np)[1], "⏸ paused");
        np.has_track = false;
        assert_eq!(card(&mut state, &np)[1], "Song 04");
    }
}
