//! Sidebar: a navigation column on the left in place of the tab rows, the panes beside
//! it, and the player and hint line across the full width underneath. On a narrow
//! terminal the column is drawn only while it has focus, and a one-row strip names
//! the location instead. A setting docks the queue on the right of a wide one.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders},
};
use ratatui_image::thread::ThreadProtocol;

use crate::theme;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::nav;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::render::NowPlaying;
use crate::tui::render::overlays::queue::draw_up_next;
use crate::tui::render::widgets::nav_column::{self, draw_nav_column};
use crate::tui::render::widgets::pane::PaneStyle;
use crate::tui::render::widgets::pane_row::draw_pane_row;
use crate::tui::render::widgets::player::{draw_player, player_size};
use crate::tui::render::widgets::{fit, header, hints, put};

use super::{take_bottom, take_top};

/// Terminal columns below which the navigation column is hidden unless it has focus.
pub const COLS_NAV: u16 = 90;
/// Terminal columns from which the queue can be docked.
pub const COLS_DOCK: u16 = 120;
/// Columns the docked queue takes.
pub const DOCK_WIDTH: u16 = 32;

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

    let t = theme::current();
    let wide = screen.width >= COLS_NAV;
    if wide || nav::in_nav(state) {
        let width = nav_column::WIDTH.min(rest.width);
        draw_nav_column(frame, Rect { width, ..rest }, state, data, true);
        if !wide {
            // No room for both: while the column has focus it is all there is.
            return;
        }
        rest.x += width;
        rest.width -= width;
    }

    if state.settings.dock_queue && screen.width >= COLS_DOCK {
        rest.width -= DOCK_WIDTH;
        let block = Block::default()
            .title(Line::styled(" Up next ", Style::new().fg(t.muted)))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(t.dim));
        let dock = Rect { x: rest.right(), width: DOCK_WIDTH, ..rest };
        let inner = block.inner(dock);
        frame.render_widget(block, dock);
        draw_up_next(frame, Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner }, state, data);
    }

    // The strip: the search types and query on Search, and, with the column hidden,
    // the name of where the user is.
    if state.selected_tab == 1 || !wide {
        let mut strip = take_top(&mut rest, 1);
        if !wide {
            put(frame, Rect { x: strip.x + 1, width: 1, ..strip }, Line::styled("≡", Style::new().fg(t.dim)));
            strip.x += 2;
            strip.width -= 2;
        }
        if state.selected_tab == 1 {
            header::draw_subtabs(frame, strip, state);
        } else {
            let items = nav::nav_items(state, data, true);
            let label = &items[nav::nav_index(state, data, &items)].label;
            let room = Rect { x: strip.x + 1, width: strip.width.saturating_sub(2), ..strip };
            let accent = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
            put(frame, room, Line::styled(fit(label, room.width as usize), accent));
        }
    }
    draw_pane_row(frame, rest, state, data, views, PaneStyle { boxed_title: false }, None, np);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Layout;
    use crate::tui::logic::state::PlaybackSource;
    use crate::tui::render::testkit::{fixture, go, render_buffer, render_text, text};

    fn sidebar() -> (AppState, AppData, FilteredViews, NowPlaying) {
        let (mut state, data, views, np) = fixture();
        state.settings.layout = Layout::Sidebar;
        (state, data, views, np)
    }

    #[test]
    fn sidebar_is_nav_column_panes_player_and_hints() {
        let (mut state, mut data, views, np) = sidebar();
        go(&mut state, &mut data, 0, 3, 0);
        let screen = render_text(140, 32, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = screen.lines().collect();
        // The column and the panes share rows 0..=26; no header rows at all.
        assert!(lines[0].starts_with("╭ sctui ─") && lines[0].chars().nth(26) == Some('╭'), "{screen}");
        assert!(lines[1].starts_with("│ ♪ Likes") && lines[9].starts_with("│ ☺ Following"), "{screen}");
        assert!(lines[26].starts_with('╰') && lines[26].chars().nth(25) == Some('╯'), "{screen}");
        assert_eq!(lines[0].matches('╭').count(), 4, "the column and three panes:\n{screen}");
        // The 4-row player and the hint line run the full width underneath.
        assert!(lines[27].starts_with('╭') && lines[27].ends_with('╮') && lines[30].starts_with('╰'), "{screen}");
        assert!(lines[28].contains("Song 04") && lines[31].contains(": commands   ? keys"), "{screen}");
        assert!(!screen.contains("1 Library"), "{screen}");

        // Search: a strip above the panes carries the type pills and the query.
        go(&mut state, &mut data, 1, 2, 0);
        let screen = render_text(140, 32, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = screen.lines().collect();
        let strip: String = lines[0].chars().skip(26).collect();
        assert!(strip.starts_with("  Tracks   Albums   Playlists   People") && strip.trim_end().ends_with("/ found"), "{screen}");
        assert!(lines[1].chars().nth(26) == Some('╭') && lines[10].starts_with("│ / Search"), "{screen}");
        // Library has no strip: its filter shows in the pane's title.
        go(&mut state, &mut data, 0, 0, 0);
        (state.search_popup_visible, state.search_query) = (true, "song".into());
        let screen = render_text(140, 32, &mut state, &mut data, &views, &np);
        assert!(screen.lines().next().unwrap().contains("╭ Likes  /song "), "{screen}");
    }

    #[test]
    fn playlist_entry_replaces_the_playlists_pane() {
        let (mut state, mut data, views, np) = sidebar();
        go(&mut state, &mut data, 0, 1, 0);
        *nav::cursor_mut(&mut state, nav::PaneId::Playlists) = 2;
        let buf = render_buffer(120, 32, &mut state, &mut data, &views, &np);
        let screen = text(&buf);
        let top = screen.lines().next().unwrap();
        // One pane beside the column: the selected playlist's tracks, focused.
        assert_eq!(top.matches('╭').count(), 2, "{screen}");
        assert!(top.contains("╭ Playlist 2 ─") && screen.contains("Listed 00") && !screen.contains("Songs"), "{screen}");
        let t = theme::current();
        assert_eq!((buf[(26, 0)].fg, buf[(0, 0)].fg), (t.accent, t.dim));
        // The column marks that playlist, and nothing names a pane off to the left.
        assert_eq!(buf[(3, 5)].bg, t.dim, "{screen}");
        assert!(!screen.contains('‹'), "{screen}");
        // With the column focused no pane has the accent border or a selection bar.
        state.nav_focused = true;
        let buf = render_buffer(120, 32, &mut state, &mut data, &views, &np);
        assert_eq!((buf[(26, 0)].fg, buf[(0, 0)].fg, buf[(3, 5)].bg), (t.dim, t.accent, t.selection_bg));
        assert!((26..120).all(|x| (0..27).all(|y| buf[(x, y)].bg != t.selection_bg)));
        let hints = text(&buf).lines().last().unwrap().to_string();
        assert!(hints.starts_with(" j k move   ] enter open   : commands   ? keys"), "{hints:?}");
    }

    #[test]
    fn nav_hidden_under_90_cols_unless_focused() {
        let (mut state, mut data, views, np) = sidebar();
        go(&mut state, &mut data, 0, 3, 0);
        let wide = render_text(90, 24, &mut state, &mut data, &views, &np);
        assert!(wide.starts_with("╭ sctui ─") && wide.contains("Friend 0"), "{wide}");
        // 89 columns: no column; a strip names the location above full-width panes.
        let narrow = render_text(89, 24, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = narrow.lines().collect();
        assert!(!narrow.contains("sctui") && !narrow.contains("♪ Likes"), "{narrow}");
        assert_eq!(lines[0].trim_end(), " ≡ Following", "{narrow}");
        assert!(lines[1].starts_with("╭ Following ─") && lines[1].ends_with('╮') && narrow.contains("Friend 0"), "{narrow}");
        // On a playlist the strip names the playlist; on Search it carries the pills.
        go(&mut state, &mut data, 0, 1, 0);
        let narrow = render_text(80, 24, &mut state, &mut data, &views, &np);
        assert!(narrow.starts_with(" ≡ Playlist 0 ") && narrow.contains("Listed 00"), "{narrow}");
        state.nav_on_header = true;
        assert!(render_text(80, 24, &mut state, &mut data, &views, &np).starts_with(" ≡ Playlists "));
        go(&mut state, &mut data, 1, 0, 0);
        let narrow = render_text(80, 24, &mut state, &mut data, &views, &np);
        let strip = narrow.lines().next().unwrap();
        assert!(strip.starts_with(" ≡  Tracks   Albums   Playlists   People") && strip.trim_end().ends_with("/ found"), "{narrow}");

        // Focused, the column is drawn alone, as tall as the panes were.
        go(&mut state, &mut data, 0, 3, 0);
        state.nav_focused = true;
        let focused = render_text(89, 24, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = focused.lines().collect();
        assert!(lines[0].starts_with("╭ sctui ─") && lines[9].starts_with("│ ☺ Following"), "{focused}");
        assert!(lines[18].starts_with('╰') && lines[19].starts_with('╭'), "{focused}");
        assert!(!focused.contains("Friend 0"), "{focused}");
        assert!((0..19).all(|y| lines[y].chars().skip(26).all(|c| c == ' ')), "{focused}");
        assert!(lines[20].contains("Song 04") && lines[23].starts_with(" j k move"), "{focused}");
        // Narrower than the column itself: it takes what there is.
        let tiny = render_text(36, 8, &mut state, &mut data, &views, &np);
        assert!(tiny.starts_with("╭ sctui ─"), "{tiny}");
    }

    #[test]
    fn dock_queue_only_from_120_cols() {
        let (mut state, mut data, views, np) = sidebar();
        // Albums: no double-width titles, so a row's chars are its cells.
        go(&mut state, &mut data, 0, 2, 0);
        let docked = |state: &mut AppState, data: &mut AppData, w: u16| {
            let screen = render_text(w, 32, state, data, &views, &np);
            (screen.contains(" Up next "), screen)
        };
        assert!(!docked(&mut state, &mut data, 160).0, "off by default");
        state.settings.dock_queue = true;
        assert!(!docked(&mut state, &mut data, 119).0);
        let (shown, screen) = docked(&mut state, &mut data, 120);
        assert!(shown, "{screen}");
        let lines: Vec<&str> = screen.lines().collect();
        // Column, panes, dock: the dock is the last 32 cells of rows 0..=26.
        assert_eq!(lines[0].chars().skip(88).collect::<String>(), format!("╭ Up next {}╮", "─".repeat(21)), "{screen}");
        let dock = |y: usize| lines[y].chars().skip(90).take(28).collect::<String>().trim_end().to_string();
        assert_eq!([dock(1), dock(2), dock(3), dock(4), dock(5)], ["QUEUED BY YOU", "1  Song 10", "2  Song 11", "FROM LIKES", "3  Song 05"], "{screen}");
        assert!(lines[26].ends_with('╯') && lines[27].ends_with('╮'), "the player still spans the width:\n{screen}");
        assert!(screen.contains("Album 0") && screen.contains("Album 0 ›"), "the panes are still there:\n{screen}");
        // Display only: the queue key still opens the popup, over the dock.
        state.queue_visible = true;
        let (shown, screen) = docked(&mut state, &mut data, 120);
        assert!(shown && screen.contains("Duration"), "{screen}");
        state.queue_visible = false;
        // Shuffle and the source are named; an empty queue says so; long titles are cut.
        state.shuffle_enabled = true;
        state.manual_queue.clear();
        assert!(docked(&mut state, &mut data, 120).1.contains("│ FROM LIKES · SHUFFLED"));
        state.auto_queue = [1, 2, 3].into();
        let buf = render_buffer(120, 32, &mut state, &mut data, &views, &np);
        assert!((0..27).all(|y| buf[(119, y)].symbol() != " "), "a wide title broke the dock's border:\n{}", text(&buf));
        // A station's queue is its own list, empty until related tracks arrive.
        state.playback_source = PlaybackSource::Radio;
        data.playback_tracks.clear();
        assert!(docked(&mut state, &mut data, 120).1.contains("queue is empty"));
        data.playback_tracks = data.likes[30..40].to_vec();
        assert!(docked(&mut state, &mut data, 120).1.contains("│ FROM STATION · SHUFFLED"));
        // Only Sidebar docks.
        state.settings.layout = Layout::Tighten;
        assert!(!docked(&mut state, &mut data, 160).0);
    }
}
