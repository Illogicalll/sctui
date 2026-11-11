//! The player readout in its three sizes: Classic's 7-row bar, a 4-row bar, one line.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, Gauge},
};
use ratatui_image::{Resize, StatefulImage, thread::ThreadProtocol};

use crate::api::{Track, format_duration};
use crate::theme;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::utils::active_tracks;
use crate::tui::render::NowPlaying;

use super::{fit, put, text_width};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayerSize {
    Classic7,
    Bar4,
    Line1,
}

impl PlayerSize {
    /// Rows the readout occupies.
    pub fn height(self) -> u16 {
        match self {
            PlayerSize::Classic7 => 7,
            PlayerSize::Bar4 => 4,
            PlayerSize::Line1 => 1,
        }
    }
}

/// Terminal rows from which the 7-row bar (Classic only) and the 4-row bar are used.
pub const ROWS_CLASSIC7: u16 = 30;
pub const ROWS_BAR4: u16 = 20;
/// Columns from which Classic's 7-row bar has the sine wave either side.
pub const COLS_SINE: u16 = 90;

/// The readout a terminal `rows` tall gets.
pub fn player_size(layout_is_classic: bool, rows: u16) -> PlayerSize {
    if layout_is_classic && rows >= ROWS_CLASSIC7 {
        PlayerSize::Classic7
    } else if rows >= ROWS_BAR4 {
        PlayerSize::Bar4
    } else {
        PlayerSize::Line1
    }
}

/// Playing, paused or stopped.
pub fn glyph(np: &NowPlaying) -> &'static str {
    match (np.has_track, np.playing) {
        (false, _) => "■",
        (true, true) => "▶",
        (true, false) => "⏸",
    }
}

pub fn ratio(state: &AppState, np: &NowPlaying) -> f64 {
    (state.progress as f64 / np.track.duration_ms.max(1) as f64).clamp(0.0, 1.0)
}

/// `width` cells of progress rule, the played part in the accent colour.
pub fn rule(width: u16, ratio: f64) -> Line<'static> {
    let t = theme::current();
    let done = ((width as f64 * ratio).round() as usize).min(width as usize);
    Line::from(vec![
        Span::styled("━".repeat(done), Style::new().fg(t.accent)),
        Span::styled("─".repeat(width as usize - done), Style::new().fg(t.dim)),
    ])
}

/// The track that plays after this one: the manual queue first, then the list.
fn next_up<'a>(state: &'a AppState, data: &'a AppData) -> Option<&'a Track> {
    state
        .manual_queue
        .front()
        .map(|queued| &queued.track)
        .or_else(|| state.auto_queue.front().and_then(|&i| active_tracks(state, data).get(i)))
}

/// Draw the player readout of `size` in `area`. `cover` is the playing track's art;
/// without it (tests, or art still loading) its space is left empty.
pub fn draw_player(
    frame: &mut Frame,
    area: Rect,
    size: PlayerSize,
    state: &AppState,
    data: &AppData,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
) {
    let area = area.intersection(frame.area());
    if area.is_empty() {
        return;
    }
    match size {
        PlayerSize::Classic7 => classic7(frame, area, state, np, cover),
        PlayerSize::Bar4 => bar4(frame, area, state, data, np, cover),
        PlayerSize::Line1 => line1(frame, area, state, np),
    }
}

/// Draw the cover image in `rect`; nothing without one.
pub fn image(frame: &mut Frame, rect: Rect, cover: Option<&mut ThreadProtocol>) {
    let rect = rect.intersection(frame.area());
    if let Some(cover) = cover
        && !rect.is_empty()
    {
        frame.render_stateful_widget(StatefulImage::new().resize(Resize::Scale(None)), rect, cover);
    }
}

/// Volume runs 0.0 to 2.0 in steps of 0.1.
fn volume_percent(np: &NowPlaying) -> u32 {
    (np.volume * 100.0).round().max(0.0) as u32
}

/// Shuffle, repeat and a ten-cell volume meter: `⇄ on   ↻ off   vol ▮▮▮▮▯▯▯▯▯▯`.
pub fn readout(state: &AppState, np: &NowPlaying) -> String {
    let on_off = |on: bool| if on { "on" } else { "off" };
    let filled = ((np.volume * 5.0).round().max(0.0) as usize).min(10);
    format!(
        "⇄ {}   ↻ {}   vol {}{}",
        on_off(state.shuffle_enabled),
        on_off(state.repeat_enabled),
        "▮".repeat(filled),
        "▯".repeat(10 - filled)
    )
}

fn classic7(frame: &mut Frame, area: Rect, state: &AppState, np: &NowPlaying, cover: Option<&mut ThreadProtocol>) {
    let t = theme::current();
    let side = if area.width >= COLS_SINE { area.width * 15 / 100 } else { 0 };
    if side > 0 {
        let points: Vec<(f64, f64)> = (0..200)
            .map(|i| {
                let x = state.tick + i as f64 * 0.1;
                (x, (x / 2.0).sin() * 10.0)
            })
            .collect();
        let chart = Chart::new(vec![
            Dataset::default().marker(symbols::Marker::Braille).style(Style::new().fg(t.accent)).data(&points),
        ])
        .x_axis(Axis::default().bounds([state.tick, state.tick + 20.0]))
        .y_axis(Axis::default().bounds([-10.0, 10.0]));
        frame.render_widget(&chart, Rect { width: side, ..area });
        frame.render_widget(&chart, Rect { x: area.right() - side, width: side, ..area });
    }

    let mid = Rect { x: area.x + side, width: area.width - 2 * side, ..area };
    let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded);
    let inner = block.inner(mid);
    frame.render_widget(block, mid);
    if inner.width < 4 || inner.height < 5 {
        return;
    }

    let mut left = inner.x + 1;
    if mid.width >= 60 {
        image(frame, Rect::new(left, inner.y, 12, inner.height), cover);
        left += 12 + 3;
    }
    // `shf / vol / rep`, bottom-right.
    const READOUT: u16 = 9;
    let mut right = inner.right().saturating_sub(1);
    if inner.width >= READOUT + 14 {
        let tick = |on: bool| if on { "✔" } else { "×" };
        let lines = [
            format!("shf:   {}", tick(state.shuffle_enabled)),
            format!("vol: {:.1}", np.volume),
            format!("rep:   {}", tick(state.repeat_enabled)),
        ];
        let x = right - READOUT;
        for (i, text) in lines.into_iter().enumerate() {
            let line = Line::styled(text, Style::new().add_modifier(Modifier::BOLD)).right_aligned();
            put(frame, Rect::new(x, inner.y + 2 + i as u16, READOUT, 1), line);
        }
        right = x.saturating_sub(2);
    }
    let width = right.saturating_sub(left);
    if width == 0 {
        return;
    }
    let column = |row: u16| Rect::new(left, inner.y + row, width, 1);
    let title = fit(&np.track.title, width as usize);
    put(frame, column(0), Line::styled(title, Style::new().add_modifier(Modifier::BOLD)).centered());
    put(frame, column(2), Line::raw(fit(&np.track.artists, width as usize)).centered());
    let label = Span::styled(
        format!("{} / {}", format_duration(state.progress), np.track.duration),
        Style::new().fg(t.fg),
    );
    frame.render_widget(
        Gauge::default()
            .style(Style::new().bg(t.selection_bg))
            .gauge_style(t.accent)
            .ratio(ratio(state, np))
            .label(label),
        column(4),
    );
}

fn bar4(
    frame: &mut Frame,
    area: Rect,
    state: &AppState,
    data: &AppData,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
) {
    let t = theme::current();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.dim));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 6 || inner.height == 0 {
        return;
    }
    let mut left = inner.x + 1;
    if area.width >= 70 {
        image(frame, Rect::new(inner.x, inner.y, 5, inner.height.min(2)), cover);
        left = inner.x + 7;
    }
    let right = inner.right() - 1;
    let readout = if area.width >= 90 {
        readout(state, np)
    } else {
        format!(
            "{}{}vol {}%",
            if state.shuffle_enabled { "⇄ " } else { "" },
            if state.repeat_enabled { "↻ " } else { "" },
            volume_percent(np)
        )
    };
    let readout_w = text_width(&readout) as u16;
    let room = right.saturating_sub(left + readout_w + 2);

    // Row 1: state, title · artist, and the shuffle / repeat / volume readout.
    let mut spans = vec![Span::styled(glyph(np), Style::new().fg(t.accent).add_modifier(Modifier::BOLD)), Span::raw("  ")];
    let title = fit(&np.track.title, room.saturating_sub(3) as usize);
    let used = 3 + text_width(&title);
    spans.push(Span::styled(title, Style::new().fg(t.fg).add_modifier(Modifier::BOLD)));
    let artist = format!(" · {}", np.track.artists);
    if used + text_width(&artist) <= room as usize {
        spans.push(Span::styled(artist, Style::new().fg(t.muted)));
    }
    put(frame, Rect::new(left, inner.y, room, 1), Line::from(spans));
    put(frame, Rect::new(left, inner.y, right.saturating_sub(left), 1), Line::styled(readout, Style::new().fg(t.fg)).right_aligned());

    // Row 2: position, progress rule, duration, and what plays next.
    if inner.height < 2 {
        return;
    }
    let y = inner.y + 1;
    let next = next_up(state, data)
        .filter(|_| area.width >= 80)
        .map(|track| format!("next › {}", fit(&track.title, 26)))
        .unwrap_or_default();
    let next_w = text_width(&next) as u16;
    let position = format_duration(state.progress);
    let position_w = text_width(&position) as u16;
    let duration_w = text_width(&np.track.duration) as u16;
    let taken = position_w + duration_w + 2 + if next_w > 0 { next_w + 3 } else { 0 };
    let rule_w = right.saturating_sub(left).saturating_sub(taken);
    let muted = Style::new().fg(t.muted);
    put(frame, Rect::new(left, y, position_w, 1), Line::styled(position, muted));
    put(frame, Rect::new(left + position_w + 1, y, rule_w, 1), rule(rule_w, ratio(state, np)));
    put(frame, Rect::new(left + position_w + rule_w + 2, y, duration_w, 1), Line::styled(np.track.duration.clone(), muted));
    put(frame, Rect::new(left, y, right.saturating_sub(left), 1), Line::styled(next, Style::new().fg(t.dim)).right_aligned());
}

fn line1(frame: &mut Frame, area: Rect, state: &AppState, np: &NowPlaying) {
    let t = theme::current();
    let mut tail = format!(" {}/{}", format_duration(state.progress), np.track.duration);
    if area.width >= 60 {
        // Shuffle and repeat only when on, as in the narrow 4-row bar.
        let on = |on: bool, glyph: &'static str| if on { glyph } else { "" };
        tail.push_str(&format!(
            "  {}{}vol {}%",
            on(state.shuffle_enabled, "⇄ "),
            on(state.repeat_enabled, "↻ "),
            volume_percent(np)
        ));
    }
    let tail_w = text_width(&tail) as u16;
    let rule_w = if area.width >= 70 { area.width / 5 } else { 0 };
    let right = area.right().saturating_sub(1);
    let text_w = area.width.saturating_sub(4 + tail_w + rule_w + 2);
    let text = fit(&format!("{} · {}", np.track.title, np.track.artists), text_w as usize);
    put(
        frame,
        Rect::new(area.x + 1, area.y, 2 + text_w, 1),
        Line::from(vec![
            Span::styled(glyph(np), Style::new().fg(t.accent).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(text, Style::new().fg(t.fg).add_modifier(Modifier::BOLD)),
        ]),
    );
    if rule_w > 0 {
        put(
            frame,
            Rect::new(right.saturating_sub(tail_w + rule_w), area.y, rule_w, 1),
            rule(rule_w, ratio(state, np)),
        );
    }
    put(
        frame,
        Rect::new(right.saturating_sub(tail_w), area.y, tail_w, 1),
        Line::styled(tail, Style::new().fg(t.muted)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::render::testkit::{draw_buffer, fixture, text};

    #[test]
    fn player_size_thresholds() {
        assert_eq!(player_size(true, 30), PlayerSize::Classic7);
        assert_eq!(player_size(true, 80), PlayerSize::Classic7);
        assert_eq!(player_size(true, 29), PlayerSize::Bar4);
        assert_eq!(player_size(false, 80), PlayerSize::Bar4);
        assert_eq!(player_size(false, 20), PlayerSize::Bar4);
        assert_eq!(player_size(true, 20), PlayerSize::Bar4);
        assert_eq!(player_size(true, 19), PlayerSize::Line1);
        assert_eq!(player_size(false, 6), PlayerSize::Line1);
        assert_eq!(
            [PlayerSize::Classic7, PlayerSize::Bar4, PlayerSize::Line1].map(PlayerSize::height),
            [7, 4, 1]
        );
    }

    #[test]
    fn every_size_shows_state_title_artist_times_and_volume() {
        let (mut state, data, _, mut np) = fixture();
        for size in [PlayerSize::Classic7, PlayerSize::Bar4, PlayerSize::Line1] {
            let screen = text(&draw_buffer(120, size.height(), |f| {
                draw_player(f, f.area(), size, &state, &data, &np, None)
            }));
            for want in [np.track.title.as_str(), np.track.artists.as_str(), np.track.duration.as_str(), "vol", "01:01"] {
                assert!(screen.contains(want), "{size:?} is missing {want:?}:\n{screen}");
            }
        }
        // Shuffle and repeat: named in the two bars, a glyph each while on in the one-line bar.
        (state.shuffle_enabled, state.repeat_enabled) = (true, true);
        for (size, width, want) in [(PlayerSize::Bar4, 120, "⇄ on   ↻ on"), (PlayerSize::Bar4, 80, "⇄ ↻ vol"), (PlayerSize::Line1, 80, "⇄ ↻ vol")] {
            let screen = text(&draw_buffer(width, size.height(), |f| draw_player(f, f.area(), size, &state, &data, &np, None)));
            assert!(screen.contains(want), "{size:?} at {width} is missing {want:?}:\n{screen}");
        }
        (state.shuffle_enabled, state.repeat_enabled) = (false, false);
        // The compact sizes name the state; the 4-row bar also has room for what is next.
        let bar = |np: &NowPlaying| text(&draw_buffer(120, 4, |f| draw_player(f, f.area(), PlayerSize::Bar4, &state, &data, np, None)));
        assert!(bar(&np).contains('▶') && bar(&np).contains("next › "), "{}", bar(&np));
        np.playing = false;
        assert!(bar(&np).contains('⏸'));
        np.has_track = false;
        assert!(bar(&np).contains('■'));
        // The sine wave either side is only drawn from 90 columns.
        let classic = |w: u16| text(&draw_buffer(w, 7, |f| draw_player(f, f.area(), PlayerSize::Classic7, &state, &data, &np, None)));
        assert!(classic(89).starts_with('╭'), "{}", classic(89));
        assert!(!classic(90).starts_with('╭'), "{}", classic(90));
    }
}
