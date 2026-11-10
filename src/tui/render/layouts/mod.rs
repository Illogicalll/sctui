//! One pure function per layout, each built from the shared widgets.

pub mod classic;

use ratatui::Frame;
use ratatui_image::thread::ThreadProtocol;

use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::render::NowPlaying;

/// Draw the whole screen behind the overlays in the user's layout.
pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
) {
    classic::draw(frame, state, data, views, np, cover);
}
