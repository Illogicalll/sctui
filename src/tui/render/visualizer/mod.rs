use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui_image::thread::ThreadProtocol;

use crate::api::Track;
use crate::tui::logic::state::VisualizerMode;

mod common;
mod cover;
mod oscilloscope;
mod spectrum;
mod interference;
mod fountain;
mod stacked;
mod seismograph;
mod rings;
mod ridgeline;
mod led;
mod curve;
mod radial;
mod mirror;

pub fn render_visualizer(
    frame: &mut Frame,
    area: Rect,
    wave_buffer: &Arc<Mutex<VecDeque<f32>>>,
    mode: VisualizerMode,
    track: &Track,
    progress_ms: u64,
    cover_art: &mut ThreadProtocol,
) {
    let samples: Vec<f32> = {
        let buffer = wave_buffer.lock().unwrap();
        buffer.iter().copied().collect()
    };

    match mode {
        VisualizerMode::Oscilloscope => oscilloscope::render_oscilloscope(frame, area, &samples),
        VisualizerMode::SpectrumBars => spectrum::render_spectrum_bars(frame, area, &samples),
        VisualizerMode::MirrorSpectrum => mirror::render_mirror_spectrum(frame, area, &samples),
        VisualizerMode::RadialSpectrum => radial::render_radial_spectrum(frame, area, &samples),
        VisualizerMode::FilledCurve => curve::render_filled_curve(frame, area, &samples),
        VisualizerMode::LedMatrix => led::render_led_matrix(frame, area, &samples),
        VisualizerMode::Ridgeline => ridgeline::render_ridgeline(frame, area, &samples),
        VisualizerMode::SpectrumRings => rings::render_spectrum_rings(frame, area, &samples),
        VisualizerMode::Seismograph => seismograph::render_seismograph(frame, area, &samples),
        VisualizerMode::StackedScope => stacked::render_stacked_scope(frame, area, &samples),
        VisualizerMode::ParticleFountain => fountain::render_particle_fountain(frame, area, &samples),
        VisualizerMode::InterferenceField => interference::render_interference_field(frame, area, &samples),
        VisualizerMode::NowPlaying => {
            // Shows title/artist itself; the border overlay would be redundant.
            return cover::render_now_playing(frame, area, track, progress_ms, cover_art);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn synthetic_samples() -> Vec<f32> {
        let sr = 44_100.0_f32;
        (0..4096)
            .flat_map(|i| {
                let t = i as f32 / sr;
                let bass = (2.0 * std::f32::consts::PI * 110.0 * t).sin() * 0.5;
                let mid = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.2;
                let hi = (2.0 * std::f32::consts::PI * 6000.0 * t).sin() * 0.08;
                [bass + mid + hi, bass * 0.8 + mid + hi * 1.5]
            })
            .collect()
    }

    fn empty_protocol() -> ThreadProtocol {
        let (tx, _rx) = std::sync::mpsc::channel();
        ThreadProtocol::new(tx, None)
    }

    fn track() -> Track {
        Track {
            title: "Track Title".into(),
            artists: "Artist".into(),
            duration: "4:10".into(),
            duration_ms: 250_000,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: "soundcloud:tracks:1".into(),
        }
    }

    fn render_all(width: u16, height: u16, frames: usize, show: bool) {
        let wave = Arc::new(Mutex::new(VecDeque::from(synthetic_samples())));
        let track = track();
        for mode in VisualizerMode::ALL {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            for _ in 0..frames {
                if show {
                    // Let the history/simulation modes accumulate real time.
                    std::thread::sleep(std::time::Duration::from_millis(40));
                }
                let mut proto = empty_protocol();
                terminal
                    .draw(|f| {
                        render_visualizer(f, f.area(), &wave, mode, &track, 102_000, &mut proto)
                    })
                    .unwrap();
            }
            if show {
                let buf = terminal.backend().buffer();
                println!("\n== {:?} ({}x{})", mode, width, height);
                for y in 0..height {
                    let row: String =
                        (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect();
                    println!("{row}");
                }
            }
        }
    }

    #[test]
    fn every_mode_renders_at_every_size() {
        render_all(80, 24, 12, std::env::var("SHOW_VIS").is_ok());
        render_all(200, 60, 3, false);
        render_all(21, 7, 3, false);
        render_all(3, 3, 2, false);
        render_all(0, 0, 1, false);
    }

    #[test]
    fn every_mode_renders_silence() {
        let wave = Arc::new(Mutex::new(VecDeque::new()));
        let track = Track {
            track_urn: String::new(),
            ..track()
        };
        for mode in VisualizerMode::ALL {
            let mut terminal = Terminal::new(TestBackend::new(60, 18)).unwrap();
            let mut proto = empty_protocol();
            terminal
                .draw(|f| render_visualizer(f, f.area(), &wave, mode, &track, 0, &mut proto))
                .unwrap();
        }
    }

    #[test]
    fn tab_cycles_through_all_modes_and_wraps() {
        let mut m = VisualizerMode::default();
        for expected in VisualizerMode::ALL.iter().skip(1) {
            m = m.next();
            assert_eq!(m, *expected);
        }
        assert_eq!(m.next(), VisualizerMode::Oscilloscope);
    }
}
