//! Tighten: the same panes with seven rows of chrome instead of thirteen. Two header
//! rows, the pane row, the 4-row player (one line on a short terminal), the hint line.

use ratatui::Frame;
use ratatui_image::thread::ThreadProtocol;

use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::render::NowPlaying;
use crate::tui::render::widgets::pane::PaneStyle;
use crate::tui::render::widgets::pane_row::draw_pane_row;
use crate::tui::render::widgets::player::{draw_player, player_size};
use crate::tui::render::widgets::{header, hints};

use super::{take_bottom, take_top};

pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
) {
    let mut rest = frame.area();
    let hint_rows = hints::rows(state, rest);
    take_bottom(&mut rest, hint_rows);
    let size = player_size(false, frame.area().height);
    let player = take_bottom(&mut rest, size.height());
    draw_player(frame, player, size, state, data, np, cover);
    header::draw_compact(frame, take_top(&mut rest, header::ROWS), state, data);
    draw_pane_row(frame, rest, state, data, views, PaneStyle { boxed_title: false }, None, np);
}

#[cfg(test)]
mod tests {
    use crate::config::Layout;
    use crate::tui::logic::state::AppState;
    use crate::tui::render::testkit::{LAYOUTS, fixture, go, render_text};

    fn tighten(state: &mut AppState) {
        state.settings.layout = Layout::Tighten;
    }

    #[test]
    fn tighten_chrome_is_at_most_seven_rows_at_120x32() {
        let (mut state, mut data, views, np) = fixture();
        tighten(&mut state);
        for (tab, sub) in [(0, 0), (0, 1), (0, 3), (1, 0), (2, 0)] {
            go(&mut state, &mut data, tab, sub, 0);
            let screen = render_text(120, 32, &mut state, &mut data, &views, &np);
            let lines: Vec<&str> = screen.lines().collect();
            // Two header rows, then panes from row 2 to row 26: 25 of the 32 rows.
            assert!(lines[0].starts_with(" sctui │") && lines[1].starts_with("  "), "{screen}");
            assert!(lines[2].starts_with('╭') && lines[26].starts_with('╰'), "tab {tab} sub {sub}:\n{screen}");
            assert!((3..26).all(|y| lines[y].starts_with('│')), "tab {tab} sub {sub}:\n{screen}");
            // The 4-row player and the hint line under them; never Classic's 7-row bar.
            assert!(lines[27].starts_with('╭') && lines[30].starts_with('╰') && !screen.contains("shf:"), "{screen}");
            assert!(lines[28].contains("Song 04") && lines[31].contains(" keys"), "{screen}");
        }
        // Even on a very tall terminal it keeps the 4-row player; a short one gets the line.
        let tall = render_text(120, 80, &mut state, &mut data, &views, &np);
        assert!(!tall.contains("shf:") && tall.lines().nth(75).unwrap().starts_with('╭'), "{tall}");
        let short = render_text(120, 19, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = short.lines().collect();
        assert!(lines[16].starts_with('╰') && lines[17].contains("Song 04") && lines[18].contains(" keys"), "{short}");
    }

    #[test]
    fn every_layout_is_in_the_everything_tests() {
        for layout in Layout::ALL {
            let mut state = AppState::default();
            let (_, set) = LAYOUTS.iter().find(|(name, _)| *name == layout.label()).expect("in testkit::LAYOUTS");
            set(&mut state);
            assert_eq!(state.settings.layout, layout);
        }
    }
}
