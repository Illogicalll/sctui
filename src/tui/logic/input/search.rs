use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::InputOutcome;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::utils::build_search_matches;

pub(crate) fn handle_search_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
) -> Option<InputOutcome> {
    let mut handled = true;
    match key.code {
        KeyCode::Backspace => {
            state.search_query.pop();
            state.search_matches = build_search_matches(
                state.selected_subtab,
                &state.search_query,
                &data.likes,
                &data.playlists,
                &data.albums,
                &data.following,
            );
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                if c == 'f' || c == 'F' {
                    state.search_popup_visible = false;
                    state.search_query.clear();
                    state.search_matches.clear();
                } else {
                    handled = false;
                }
            } else {
                state.search_query.push(c);
                state.search_matches = build_search_matches(
                    state.selected_subtab,
                    &state.search_query,
                    &data.likes,
                    &data.playlists,
                    &data.albums,
                    &data.following,
                );
            }
        }
        _ => {
            handled = false;
        }
    }
    if handled {
        Some(InputOutcome::Continue)
    } else {
        None
    }
}
