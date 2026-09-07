use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use ratatui_image::{FilterType, Resize, StatefulImage, thread::ThreadProtocol};
use crate::theme;

use crate::api::{Track, format_duration};

use super::common::frame_block;

const PAD: u16 = 2;
const MIN_WIDTH_FOR_ART: u16 = 30;
const ART_MAX_WIDTH_FRACTION: f32 = 0.9; // of the left half
const ART_HEIGHT_FRACTION: f32 = 0.5;
const INFO_WIDTH_FRACTION: u16 = 3; // info block is 3/4 of the text column
#[allow(non_snake_case)]
fn DIM() -> Color {
    theme::current().dim
}
#[allow(non_snake_case)]
fn GREY() -> Color {
    theme::current().muted
}

/// Big cover art on the left, title / artist / progress on the right.
pub fn render_now_playing(
    frame: &mut Frame,
    area: Rect,
    track: &Track,
    progress_ms: u64,
    cover_art: &mut ThreadProtocol,
) {
    let block = frame_block();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width < 8 || inner.height < 5 {
        return;
    }

    let body = Rect {
        x: inner.x + PAD,
        y: inner.y + 1,
        width: inner.width.saturating_sub(2 * PAD),
        height: inner.height.saturating_sub(2),
    };

    // Two halves: art centred in the left one, info centred in the right one.
    // Below MIN_WIDTH_FOR_ART the art is dropped and the info takes the body.
    let (art_half, text) = if inner.width >= MIN_WIDTH_FOR_ART {
        let [l, r] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(body);
        (Some(l), r)
    } else {
        (None, body)
    };

    if let Some(half) = art_half {
        // Art is a square: a cell is ~2:1, so width = 2 × height. Takes 50% of
        // the height so it sits inset rather than touching the frame.
        let art_w = ((half.height as f32 * ART_HEIGHT_FRACTION) as u16 * 2)
            .min((half.width as f32 * ART_MAX_WIDTH_FRACTION) as u16)
            .max(2);
        let art_h = (art_w / 2).clamp(1, half.height);
        let art_rect = Rect {
            x: half.x + (half.width - art_w) / 2,
            y: half.y + (half.height - art_h) / 2,
            width: art_w,
            height: art_h,
        };
        frame.render_stateful_widget(
            StatefulImage::new().resize(Resize::Scale(Some(FilterType::Triangle))),
            art_rect,
            cover_art,
        );
    }

    // Info block centred in the text column, both ways. Five rows: title,
    // artist, blank, bar, times.
    let info_w = (text.width * INFO_WIDTH_FRACTION / 4).max(text.width.min(16));
    let info_x = text.x + (text.width - info_w) / 2;
    let rows = 5u16.min(text.height);
    let top = text.y + (text.height - rows) / 2;
    let row = |i: u16| Rect { x: info_x, y: top + i, width: info_w, height: 1 };

    if rows >= 1 {
        frame.render_widget(
            Paragraph::new(track.title.as_str())
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(theme::current().fg)
                        .add_modifier(Modifier::BOLD),
                ),
            row(0),
        );
    }
    if rows >= 2 {
        frame.render_widget(
            Paragraph::new(track.artists.as_str())
                .alignment(Alignment::Center)
                .style(Style::default().fg(GREY())),
            row(1),
        );
    }
    if rows >= 4 {
        let ratio = if track.duration_ms == 0 {
            0.0
        } else {
            (progress_ms as f64 / track.duration_ms as f64).clamp(0.0, 1.0)
        };
        frame.render_widget(Paragraph::new(progress_line(info_w, ratio)), row(3));
    }
    if rows >= 5 {
        let elapsed = format_duration(progress_ms);
        let total = track.duration.as_str();
        let space = (info_w as usize).saturating_sub(elapsed.chars().count() + total.chars().count());
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(elapsed),
                Span::raw(" ".repeat(space)),
                Span::raw(total),
            ]))
            .style(Style::default().fg(GREY())),
            row(4),
        );
    }
}

/// `━━━━╸────`: heavy line for played, a head glyph, faint line for remaining.
fn progress_line(width: u16, ratio: f64) -> Line<'static> {
    let width = width as usize;
    let played = (ratio * width as f64).round() as usize;
    let played = played.min(width);
    let mut spans = Vec::with_capacity(3);
    if played > 0 {
        spans.push(Span::styled(
            "━".repeat(played - 1) + "╸",
            Style::default().fg(theme::current().accent),
        ));
    }
    spans.push(Span::styled(
        "─".repeat(width - played),
        Style::default().fg(DIM()),
    ));
    Line::from(spans)
}
