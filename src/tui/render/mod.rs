mod layouts;
mod overlays;
#[cfg(test)]
pub(crate) mod testkit;
mod utils;
mod visualizer;
mod widgets;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use crate::theme;

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::Line,
};
use ratatui_image::thread::ThreadProtocol;

use crate::api::Track;
use crate::player::Player;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::render::visualizer::render_visualizer;

/// What the player is doing, read once per frame so drawing needs no audio device.
pub struct NowPlaying {
    /// The playing track, or the player's "nothing playing" placeholder.
    pub track: Track,
    /// 0.0 to 2.0.
    pub volume: f32,
    /// False while paused or stopped.
    pub playing: bool,
    /// False when nothing has been played yet (`track` is the placeholder).
    pub has_track: bool,
}

impl NowPlaying {
    pub fn from_player(player: &Player) -> Self {
        let track = player.current_track();
        Self {
            has_track: !track.track_urn.is_empty(),
            volume: player.get_volume(),
            playing: player.is_playing(),
            track,
        }
    }
}

/// Smallest terminal anything but the "too small" notice is drawn on.
pub const MIN_COLS: u16 = 36;
pub const MIN_ROWS: u16 = 8;

pub fn render(
    frame: &mut Frame,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
    wave_buffer: &Arc<Mutex<VecDeque<f32>>>,
) {
    let area = frame.area();
    if area.width < MIN_COLS || area.height < MIN_ROWS {
        // Nothing else, overlays included: they stay open and come back with the room.
        let t = theme::current();
        let y = area.y + area.height.saturating_sub(2) / 2;
        let row = |dy: u16| Rect { y: y + dy, height: 1, ..area };
        widgets::put(frame, row(0), Line::styled("terminal too small", Style::new().fg(t.warning)).centered());
        widgets::put(
            frame,
            row(1),
            Line::styled(format!("{}×{}", area.width, area.height), Style::new().fg(t.dim)).centered(),
        );
        return;
    }

    if state.visualizer_mode {
        render_visualizer(
            frame,
            area,
            wave_buffer,
            state.visualizer_view,
            &np.track,
            state.progress,
            cover,
            &state.lyrics,
        );
    } else {
        layouts::draw(frame, state, data, views, np, cover);
    }
    overlays::render_overlays(frame, state, data);
}
