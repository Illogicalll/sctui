use std::time::Instant;

use super::InputOutcome;
use crate::keymap::Action;
use crate::tui::logic::filtering::build_filtered_views;
use crate::tui::logic::nav::{self, Edge};
use crate::tui::logic::state::{AppData, AppState};

/// Every movement action, applied to the focused pane. Holding Up/Down moves further
/// per repeat the longer it is held.
pub(crate) fn handle_move(action: Action, state: &mut AppState, data: &mut AppData) -> InputOutcome {
    let views = build_filtered_views(state, data);
    let page = nav::page_rows(state) as isize;
    let half = (page / 2).max(1);
    let delta = match action {
        Action::Up | Action::Down => {
            let dir = if action == Action::Up { -1 } else { 1 };
            dir * state.move_accel.step_rows(nav::focused(state), dir, Instant::now()) as isize
        }
        Action::PageUp => -page,
        Action::PageDown => page,
        Action::HalfPageUp => -half,
        Action::HalfPageDown => half,
        Action::Top => {
            nav::move_to(state, data, &views, Edge::Top);
            return InputOutcome::Continue;
        }
        _ => {
            nav::move_to(state, data, &views, Edge::Bottom);
            return InputOutcome::Continue;
        }
    };
    nav::move_by(state, data, &views, delta);
    InputOutcome::Continue
}
