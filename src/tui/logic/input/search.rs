use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use super::helpers::edit_text;
use crate::tui::logic::filtering::{clear_filter, filter_edited};
use crate::tui::logic::nav;
use crate::tui::logic::state::{AppData, AppState};

/// Insert mode: the Search query (Search tab) or the Library filter owns every key.
/// Enter keeps the text, Esc cancels a filter, Up/Down step back out to the list;
/// nothing here runs a bound action.
pub(crate) fn handle_insert(key: KeyEvent, state: &mut AppState, data: &AppData) -> InputOutcome {
    let library = state.selected_tab == 0;
    match key.code {
        KeyCode::Esc if library => {
            state.search_typing = false;
            clear_filter(state);
        }
        KeyCode::Esc | KeyCode::Enter | KeyCode::Up | KeyCode::Down => {
            state.search_typing = false;
            if library && state.search_query.trim().is_empty() {
                clear_filter(state);
            }
        }
        _ if library => {
            let was_on = nav::filter_pane(state).and_then(|pane| nav::selected_index(state, pane));
            if edit_text(&key, &mut state.search_query) {
                filter_edited(state, data, was_on);
            }
        }
        _ => {
            if edit_text(&key, &mut state.query) {
                state.search_needs_fetch = true;
                nav::reset_search(state);
            }
        }
    }
    InputOutcome::Continue
}
