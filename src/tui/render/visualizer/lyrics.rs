use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
};

use crate::tui::logic::state::LyricsStatus;

use super::common::frame_block;

const CURRENT: Color = Color::Cyan;
const NEAR: Color = Color::Rgb(150, 150, 165);
const FAR: Color = Color::Rgb(85, 85, 100);

/// Lyrics for the playing track: synced lines keep the current one centred,
/// plain lyrics scroll with playback, otherwise a one-line notice.
pub fn render_lyrics(
    frame: &mut Frame,
    area: Rect,
    status: &LyricsStatus,
    progress_ms: u64,
    duration_ms: u64,
) {
    let block = frame_block();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Clear, inner);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let notice = |text: &str| {
        let pad = inner.height / 2;
        let mut lines = vec![Line::raw(""); pad as usize];
        lines.push(Line::styled(text.to_string(), Style::default().fg(FAR)));
        Paragraph::new(lines).alignment(Alignment::Center)
    };

    match status {
        LyricsStatus::Idle | LyricsStatus::Loading => {
            frame.render_widget(notice("Looking for lyrics…"), inner);
        }
        LyricsStatus::NotFound => {
            frame.render_widget(notice("No lyrics found for this track"), inner);
        }
        LyricsStatus::Found(lyrics) if lyrics.is_synced() => {
            let lines = &lyrics.synced;
            let current = lines.iter().rposition(|(t, _)| *t <= progress_ms);
            let height = inner.height as usize;
            let centre = height / 2;
            // Before the first line, park the "current" slot just above line 0.
            let anchor = current.map(|i| i as isize).unwrap_or(-1);
            let mut out = Vec::with_capacity(height);
            for row in 0..height {
                let li = anchor + row as isize - centre as isize;
                if li < 0 || li as usize >= lines.len() {
                    out.push(Line::raw(""));
                    continue;
                }
                let text = lines[li as usize].1.as_str();
                let text = if text.is_empty() { "♪" } else { text };
                let style = if Some(li as usize) == current {
                    Style::default().fg(CURRENT).add_modifier(Modifier::BOLD)
                } else if row.abs_diff(centre) <= 2 {
                    Style::default().fg(NEAR)
                } else {
                    Style::default().fg(FAR)
                };
                out.push(Line::styled(text.to_string(), style));
            }
            frame.render_widget(Paragraph::new(out).alignment(Alignment::Center), inner);
        }
        LyricsStatus::Found(lyrics) => {
            let mut out = vec![Line::styled("(unsynced lyrics)", Style::default().fg(FAR)), Line::raw("")];
            out.extend(lyrics.plain.iter().map(|l| Line::styled(l.clone(), Style::default().fg(NEAR))));
            let visible = inner.height as usize;
            let overflow = out.len().saturating_sub(visible);
            let fraction = if duration_ms == 0 { 0.0 } else { (progress_ms as f64 / duration_ms as f64).clamp(0.0, 1.0) };
            let scroll = (fraction * overflow as f64).round() as u16;
            frame.render_widget(
                Paragraph::new(out)
                    .alignment(Alignment::Center)
                    .wrap(Wrap { trim: true })
                    .scroll((scroll, 0)),
                inner,
            );
        }
    }
}
