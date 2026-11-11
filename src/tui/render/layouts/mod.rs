//! One pure function per layout, each built from the shared widgets.

pub mod classic;
pub mod columns;
pub mod panel;
pub mod tighten;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ratatui::{Frame, layout::Rect};
use ratatui_image::thread::ThreadProtocol;

use crate::config::Layout;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::render::NowPlaying;

/// Draw the whole screen behind the overlays in the user's layout. Every layout leaves
/// `widgets::hints::rows` rows free at the bottom; `render` draws the hint line there.
pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
    wave: &Arc<Mutex<VecDeque<f32>>>,
) {
    match state.settings.layout {
        Layout::Classic => classic::draw(frame, state, data, views, np, cover),
        Layout::Tighten => tighten::draw(frame, state, data, views, np, cover),
        Layout::Columns => columns::draw(frame, state, data, views, np, cover),
        Layout::PlayerPanel => panel::draw(frame, state, data, views, np, cover, wave),
    }
}

/// Carve `rows` off the top of `rest` (fewer when it has fewer).
fn take_top(rest: &mut Rect, rows: u16) -> Rect {
    let rows = rows.min(rest.height);
    let taken = Rect { height: rows, ..*rest };
    rest.y += rows;
    rest.height -= rows;
    taken
}

/// Carve `rows` off the bottom of `rest` (fewer when it has fewer).
fn take_bottom(rest: &mut Rect, rows: u16) -> Rect {
    let rows = rows.min(rest.height);
    rest.height -= rows;
    Rect { y: rest.bottom(), height: rows, ..*rest }
}
