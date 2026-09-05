use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::api::Track;
use crate::tui::logic::state::VisualizerMode;

mod common;
mod oscilloscope;
mod spectrum;

pub fn render_visualizer(
    frame: &mut Frame,
    area: Rect,
    wave_buffer: &Arc<Mutex<VecDeque<f32>>>,
    mode: VisualizerMode,
    track: &Track,
) {
    let samples: Vec<f32> = {
        let buffer = wave_buffer.lock().unwrap();
        buffer.iter().copied().collect()
    };

    match mode {
        VisualizerMode::Oscilloscope => oscilloscope::render_oscilloscope(frame, area, &samples),
        VisualizerMode::SpectrumBars => spectrum::render_spectrum_bars(frame, area, &samples),
        // ADD_MODE
    }

    render_now_playing_overlay(frame, area, track);
}

/// `artist – title` set into the bottom border, bottom-left. The
/// now-playing bar is hidden while the visualiser is up, so this is the only
/// place the user can see what is playing.
fn render_now_playing_overlay(frame: &mut Frame, area: Rect, track: &Track) {
    if track.track_urn.is_empty() || area.width < 6 || area.height < 3 {
        return;
    }
    let text = format!(" {} – {} ", track.artists, track.title);
    let max = area.width as usize - 4;
    let text: String = if text.chars().count() > max {
        let mut s: String = text.chars().take(max.saturating_sub(2)).collect();
        s.push_str("… ");
        s
    } else {
        text
    };
    let x = area.x + 2;
    let y = area.y + area.height - 1; // bottom border row, like Block::title_bottom
    frame.render_widget(
        Span::styled(
            text,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Rect::new(x, y, (area.width - 4).min(max as u16), 1),
    );
}
