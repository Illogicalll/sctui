use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::tui::logic::state::VisualizerMode;

mod common;
mod oscilloscope;
mod spectrum;

pub fn render_visualizer(
    frame: &mut Frame,
    area: Rect,
    wave_buffer: &Arc<Mutex<VecDeque<f32>>>,
    mode: VisualizerMode,
) {
    let samples: Vec<f32> = {
        let buffer = wave_buffer.lock().unwrap();
        buffer.iter().copied().collect()
    };

    match mode {
        VisualizerMode::Oscilloscope => oscilloscope::render_oscilloscope(frame, area, &samples, mode),
        VisualizerMode::SpectrumBars => spectrum::render_spectrum_bars(frame, area, &samples, mode),
    }
}
