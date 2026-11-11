//! Player panel: Tighten's header and panes on the left, running down to the hint
//! line, and a permanent now-playing panel on the right (cover, track, progress,
//! state, and a lower area that pages between Up next, Lyrics, Spectrum and History).
//! No bottom player bar. On a terminal too narrow for the panel it is Tighten.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders},
};
use ratatui_image::thread::ThreadProtocol;

use crate::api::format_duration;
use crate::keymap::Action;
use crate::theme;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::state::{AppData, AppState, PanelPage};
use crate::tui::render::NowPlaying;
use crate::tui::render::overlays::queue::{draw_up_next, source_name};
use crate::tui::render::visualizer::{lyrics::draw_lyrics, spectrum::draw_bars};
use crate::tui::render::widgets::pane::PaneStyle;
use crate::tui::render::widgets::pane_row::draw_pane_row;
use crate::tui::render::widgets::player::{glyph, image, rate_limit_title, ratio, readout, rule};
use crate::tui::render::widgets::{border_credit, fit, header, hints, put, put_right, text_width};

use super::{take_bottom, take_top, tighten};

/// Terminal columns below which there is no panel and the layout is Tighten.
pub const COLS_PANEL: u16 = 110;
/// Widest the panel gets; narrower terminals give it 36% of their columns.
pub const PANEL_MAX: u16 = 46;

/// Columns the panel takes on a terminal `cols` wide.
pub fn panel_width(cols: u16) -> u16 {
    PANEL_MAX.min((cols as u32 * 36 / 100) as u16)
}

/// Rows of cover art in a panel `panel_rows` tall.
pub fn cover_rows(panel_rows: u16) -> u16 {
    match panel_rows {
        34.. => 9,
        26.. => 6,
        20.. => 3,
        _ => 0,
    }
}

pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
    wave: &Arc<Mutex<VecDeque<f32>>>,
) {
    let screen = frame.area();
    if screen.width < COLS_PANEL {
        return tighten::draw(frame, state, data, views, np, cover);
    }
    let mut rest = screen;
    let hint_rows = hints::rows(state, screen);
    take_bottom(&mut rest, hint_rows);
    let width = panel_width(screen.width);
    rest.width -= width;
    draw_panel(frame, Rect { x: rest.right(), width, ..rest }, state, data, np, cover, wave);
    header::draw_compact(frame, take_top(&mut rest, header::ROWS), state, data);
    draw_pane_row(frame, rest, state, data, views, PaneStyle { boxed_title: false }, None, np);
}

/// The row `skip` below the last one handed out, or `None` once `within` is full.
fn next_row(y: &mut u16, skip: u16, within: Rect) -> Option<Rect> {
    *y += skip + 1;
    (*y <= within.bottom()).then(|| Rect { y: *y - 1, height: 1, ..within })
}

/// The panel in `area`, top to bottom: cover, title, `artist · from <source>`,
/// progress, state, a rule naming the four pages, and the current page. Display only:
/// nothing in it takes focus. Without `cover` (tests, art still loading) the cover's
/// rows are left empty.
fn draw_panel(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    data: &AppData,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
    wave: &Arc<Mutex<VecDeque<f32>>>,
) {
    let t = theme::current();
    let dim = Style::new().fg(t.dim);
    let muted = Style::new().fg(t.muted);
    let accent = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let block = Block::default()
        .title(rate_limit_title(state, area.width).unwrap_or_else(|| Line::styled(" Now playing ", muted)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(dim);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    border_credit(frame, area, state);
    if inner.width < 8 || inner.height == 0 {
        return;
    }
    // One cell of padding either side of the text.
    let text = Rect { x: inner.x + 1, width: inner.width - 2, ..inner };
    let width = text.width as usize;
    let mut y = text.y;
    // Blank rows between the groups only where the panel has rows to spare.
    let gap = (area.height >= 20) as u16;

    let cover_h = cover_rows(area.height);
    if cover_h > 0 {
        // A terminal cell is about twice as tall as it is wide.
        let cover_w = (cover_h * 2).min(text.width);
        image(frame, Rect::new(text.x + (text.width - cover_w) / 2, y, cover_w, cover_h), cover);
        y += cover_h + 1;
    }
    if let Some(r) = next_row(&mut y, 0, text) {
        put(frame, r, Line::styled(fit(&np.track.title, width), Style::new().fg(t.fg).add_modifier(Modifier::BOLD)).centered());
    }
    if let Some(r) = next_row(&mut y, 0, text) {
        let from = match np.has_track {
            true => format!("{} · from {}", np.track.artists, source_name(state).to_lowercase()),
            false => np.track.artists.clone(),
        };
        put(frame, r, Line::styled(fit(&from, width), muted).centered());
    }
    if let Some(r) = next_row(&mut y, gap, text) {
        let position = format_duration(state.progress);
        let (position_w, duration_w) = (text_width(&position) as u16, text_width(&np.track.duration) as u16);
        let rule_w = r.width.saturating_sub(position_w + duration_w + 2);
        put(frame, r, Line::styled(position, muted));
        put(frame, Rect { x: r.x + position_w + 1, width: rule_w, ..r }, rule(rule_w, ratio(state, np)));
        put_right(frame, r, &np.track.duration, muted);
    }
    if let Some(r) = next_row(&mut y, 0, text) {
        let spans = vec![Span::styled(glyph(np), accent), Span::styled(format!("   {}", readout(state, np)), Style::new().fg(t.fg))];
        put(frame, r, Line::from(spans).centered());
    }

    // The rule: the page names, the current one in accent, and the key that pages.
    let Some(r) = next_row(&mut y, gap, inner) else { return };
    frame.buffer_mut().set_string(area.x, r.y, format!("├{}┤", "─".repeat(inner.width as usize)), dim);
    let mut spans = Vec::new();
    for page in PanelPage::ALL {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(page.label(), if page == state.panel_page { accent } else { muted }));
    }
    spans.push(Span::raw(" "));
    let names = Line::from(spans);
    let room = (inner.width as usize).saturating_sub(names.width());
    put(frame, r, names);
    let key = state.keymap.hint_label(Action::PanelPage);
    if let Some(hint) = [format!(" {key} page "), format!(" {key} ")].into_iter().find(|h| !key.is_empty() && text_width(h) <= room) {
        put_right(frame, r, &hint, dim);
    }

    let body = Rect { y, height: inner.bottom().saturating_sub(y), ..text };
    if body.is_empty() {
        return;
    }
    match state.panel_page {
        PanelPage::UpNext => draw_up_next(frame, body, state, data),
        PanelPage::Lyrics => draw_lyrics(frame, body, &state.lyrics, state.progress, np.track.duration_ms),
        PanelPage::Spectrum => {
            let samples: Vec<f32> = wave.lock().unwrap().iter().copied().collect();
            if samples.is_empty() {
                // Nothing has played yet: a flat line where the bars will stand.
                put(frame, Rect { y: body.bottom() - 1, height: 1, ..body }, Line::styled("▁".repeat(width), dim));
            } else {
                draw_bars(frame, body, &samples);
            }
        }
        PanelPage::History => {
            // Newest first, as the history popup lists it.
            let mut y = body.y;
            for queued in state.playback_history.iter().rev() {
                let Some(r) = next_row(&mut y, 0, body) else { break };
                put(frame, r, Line::styled(fit(&queued.track.title, width), muted));
            }
            if state.playback_history.is_empty() {
                put(frame, Rect { height: 1, ..body }, Line::styled("no history yet", dim));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Lyrics;
    use crate::config::Layout;
    use crate::tui::logic::state::LyricsStatus;
    use crate::tui::render::testkit::{draw_buffer, fixture, go, render_buffer, render_text, text};
    use crate::tui::render::render;

    fn panel() -> (AppState, AppData, FilteredViews, NowPlaying) {
        let (mut state, data, views, np) = fixture();
        state.settings.layout = Layout::PlayerPanel;
        (state, data, views, np)
    }

    /// The panel's rows at `w`×`h`, without its borders or padding. Likes is on the
    /// left, so cells are read from the buffer: its titles are double-width.
    fn panel_rows(w: u16, h: u16, state: &mut AppState, data: &mut AppData, views: &FilteredViews, np: &NowPlaying) -> Vec<String> {
        let buf = render_buffer(w, h, state, data, views, np);
        let x = w - panel_width(w);
        (0..h).map(|y| (x..w).map(|x| buf[(x, y)].symbol()).collect::<String>()).collect()
    }

    /// #55: every layout carries the rate-limit notice once, on whichever player it has.
    #[test]
    fn every_layout_shows_the_rate_limit_notice_once() {
        use chrono::TimeZone;
        let (mut state, mut data, views, np) = fixture();
        state.rate_limited_until = Some(chrono::Local.with_ymd_and_hms(2100, 1, 1, 14, 32, 10).unwrap());
        for layout in Layout::ALL {
            state.settings.layout = layout;
            // A player box (the panel's, from 110 columns), then the one-line player.
            for (w, h) in [(160, 40), (100, 24), (100, 12)] {
                let screen = render_text(w, h, &mut state, &mut data, &views, &np);
                assert_eq!(screen.matches("Rate limited by SoundCloud · resumes").count(), 1, "{layout:?} {w}x{h}:\n{screen}");
            }
        }
        state.settings.layout = Layout::PlayerPanel;
        let screen = render_text(110, 30, &mut state, &mut data, &views, &np);
        let top: String = screen.lines().next().unwrap().chars().skip(71).collect();
        assert!(top.starts_with("╭ Rate limited by SoundCloud · resum") && top.ends_with('╮'), "{screen}");
        assert!(!screen.contains("Now playing"), "the notice takes the title's place:\n{screen}");
    }

    #[test]
    fn panel_falls_back_to_tighten_under_110_cols() {
        let (mut state, mut data, views, np) = panel();
        go(&mut state, &mut data, 0, 1, 1);
        let both = |state: &mut AppState, data: &mut AppData, w: u16, h: u16| {
            state.settings.layout = Layout::Tighten;
            let tighten = render_text(w, h, state, data, &views, &np);
            state.settings.layout = Layout::PlayerPanel;
            (render_text(w, h, state, data, &views, &np), tighten)
        };
        for (w, h) in [(109, 30), (100, 28), (60, 18), (36, 8)] {
            let (panel, tighten) = both(&mut state, &mut data, w, h);
            assert_eq!(panel, tighten, "{w}x{h}");
            assert!(!panel.contains("Now playing") && !panel.contains("o page"), "{panel}");
        }
        // From 110 columns: the panel on the right for the full height above the hint
        // line, the header and panes on the left down to it, and no player bar.
        let (screen, tighten) = both(&mut state, &mut data, 110, 30);
        assert_ne!(screen, tighten);
        let lines: Vec<&str> = screen.lines().collect();
        assert_eq!((panel_width(110), panel_width(127), panel_width(128), panel_width(240)), (39, 45, 46, 46));
        let right = |y: usize| lines[y].chars().skip(71).collect::<String>();
        assert_eq!(right(0), format!("╭ Now playing {}╮", "─".repeat(24)), "{screen}");
        assert!(right(28).starts_with('╰') && right(28).ends_with('╯'), "{screen}");
        assert!(lines[0].starts_with(" sctui │  1 Library") && lines[1].starts_with("  Likes "), "{screen}");
        assert!(lines[2].starts_with('╭') && lines[28].starts_with('╰'), "the panes run to the hint line:\n{screen}");
        assert_eq!(screen.matches("Song 04").count(), 1, "one readout, in the panel:\n{screen}");
        // The hint line names the page key exactly when the panel is there.
        let wide = render_text(160, 30, &mut state, &mut data, &views, &np);
        assert!(wide.lines().last().unwrap().contains("   o page   : commands   ? keys"), "{wide}");
        let paged = |cols| hints::pairs(&state, &data, cols).iter().any(|(_, what)| *what == "page");
        assert!(paged(110) && !paged(109));
        // The header's right-hand keys end where the panel starts.
        assert!(lines[0].chars().take(71).collect::<String>().trim_end().len() <= 70, "{screen}");
    }

    #[test]
    fn cover_height_thresholds() {
        assert_eq!([80, 34, 33, 26, 25, 20, 19, 8].map(cover_rows), [9, 9, 6, 6, 3, 3, 0, 0]);
        let (mut state, mut data, views, np) = panel();
        // Panel rows are the terminal's less the hint line. The title sits under the
        // cover's rows and a blank one; they stay empty without an image to draw.
        for (h, title_row) in [(35, 11), (34, 8), (27, 8), (26, 5), (21, 5), (20, 1), (14, 1), (8, 1)] {
            let rows = panel_rows(130, h, &mut state, &mut data, &views, &np);
            assert_eq!(rows[title_row].trim_matches(['│', ' ']), "Song 04", "{h} rows:\n{}", rows.join("\n"));
            assert!((1..title_row).all(|y| rows[y].trim_matches(['│', ' ']).is_empty()), "{h} rows:\n{}", rows.join("\n"));
        }
    }

    #[test]
    fn panel_shows_the_track_its_state_and_the_page() {
        let (mut state, mut data, views, mut np) = panel();
        let rows = panel_rows(130, 34, &mut state, &mut data, &views, &np);
        let row = |y: usize| rows[y].trim_matches(['│', ' ']).to_string();
        // 34 rows less the hint line: a 6-row cover, then the track.
        assert_eq!([row(8), row(9), row(10)], ["Song 04", "Artist 4 · from likes", ""], "{}", rows.join("\n"));
        assert!(row(11).starts_with("01:01 ━━━") && row(11).ends_with("──── 06:52"), "{}", rows[11]);
        assert_eq!(row(12), "▶   ⇄ off   ↻ off   vol ▮▮▮▮▯▯▯▯▯▯");
        assert_eq!(rows[14], format!("├ Up next Lyrics Spectrum History {} o page ┤", "─".repeat(3)));
        assert_eq!([row(15), row(16), row(17), row(18)], ["QUEUED BY YOU", "1  Song 10", "2  Song 11", "FROM LIKES"]);
        assert!(rows[32].starts_with('╰'), "{}", rows.join("\n"));
        // The narrowest panel names the key without the word; an unbound key is not named.
        let narrow = panel_rows(110, 34, &mut state, &mut data, &views, &np);
        assert_eq!(narrow[14], "├ Up next Lyrics Spectrum History ─ o ┤");
        state.keymap.apply_toml("[keys]\npanel_page = []\n");
        let unbound = render_text(130, 34, &mut state, &mut data, &views, &np);
        assert!(unbound.contains("History ───────────┤") && !unbound.contains(" page"), "{unbound}");

        // The current page's name is the accented one.
        state.panel_page = PanelPage::History;
        let buf = render_buffer(130, 34, &mut state, &mut data, &views, &np);
        let t = theme::current();
        let x = 130 - panel_width(130);
        assert_eq!((buf[(x + 2, 14)].fg, buf[(x + 27, 14)].fg), (t.muted, t.accent));
        let rows = panel_rows(130, 34, &mut state, &mut data, &views, &np);
        let row = |y: usize| rows[y].trim_matches(['│', ' ']).to_string();
        assert_eq!([row(15), row(16), row(17), row(18)], ["Song 22", "Song 21", "Song 20", ""], "newest first");
        state.playback_history.clear();
        assert!(panel_rows(130, 34, &mut state, &mut data, &views, &np)[15].contains("no history yet"));

        // Lyrics: the visualiser's drawing, in the page's rows, current line centred.
        state.panel_page = PanelPage::Lyrics;
        assert!(panel_rows(130, 34, &mut state, &mut data, &views, &np)[23].contains("Looking for lyrics…"));
        state.lyrics = LyricsStatus::Found(Lyrics {
            synced: vec![(5_000, "Intro line".into()), (60_000, "Current line".into()), (120_000, "Later line".into())],
            plain: vec![],
        });
        let rows = panel_rows(130, 34, &mut state, &mut data, &views, &np);
        let row = |y: usize| rows[y].trim_matches(['│', ' ']).to_string();
        assert_eq!([row(22), row(23), row(24)], ["Intro line", "Current line", "Later line"], "{}", rows.join("\n"));
        assert!(rows[14].starts_with("├ Up next") && rows[32].starts_with('╰'), "the frame is the panel's own");

        // Spectrum: idle until there is sound, then the visualiser's bars.
        state.panel_page = PanelPage::Spectrum;
        let rows = panel_rows(130, 34, &mut state, &mut data, &views, &np);
        assert_eq!(rows[31].trim_matches(['│', ' ']), "▁".repeat(42), "{}", rows.join("\n"));
        let wave = Arc::new(Mutex::new((0..4096).map(|i| (i as f32 * 0.05).sin()).collect::<VecDeque<f32>>()));
        let buf = draw_buffer(130, 34, |f| render(f, &mut state, &mut data, &views, &np, None, &wave));
        let bars = text(&buf);
        assert!(bars.contains('█') && !bars.contains("sctui ─"), "{bars}");

        // Paused, stopped, shuffle and repeat all show on the state row.
        (np.playing, state.shuffle_enabled, state.repeat_enabled) = (false, true, true);
        assert!(panel_rows(130, 34, &mut state, &mut data, &views, &np)[12].contains("⏸   ⇄ on   ↻ on   vol"));
        np.has_track = false;
        let rows = panel_rows(130, 34, &mut state, &mut data, &views, &np);
        assert!(rows[12].contains('■') && rows[9].trim_matches(['│', ' ']) == "Artist 4", "{}", rows.join("\n"));
    }

    #[test]
    fn panel_pages_are_display_only() {
        // `q` over the panel: the popup, with the panel still behind it on its page.
        let (mut state, mut data, views, np) = panel();
        state.panel_page = PanelPage::History;
        state.queue_visible = true;
        let screen = render_text(130, 34, &mut state, &mut data, &views, &np);
        assert!(screen.contains("Queue") && screen.contains("Duration") && screen.contains("Now playing"), "{screen}");
        assert_eq!(state.panel_page, PanelPage::History);
        // Nothing in the panel carries a selection bar.
        state.queue_visible = false;
        let buf = render_buffer(130, 34, &mut state, &mut data, &views, &np);
        let t = theme::current();
        assert!((130 - panel_width(130)..130).all(|x| (0..34).all(|y| buf[(x, y)].bg != t.selection_bg)));
    }
}
