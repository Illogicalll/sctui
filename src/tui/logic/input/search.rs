use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::InputOutcome;
use crate::tui::logic::state::{AppData, AppState};
use crate::tui::logic::utils::build_search_matches;

fn set_primary_selection(state: &mut AppState, data: &mut AppData, row: usize) {
    state.selected_row = row;
    match state.selected_subtab {
        0 => data.likes_state.select(Some(row)),
        1 => {
            state.selected_playlist_row = row;
            data.playlists_state.select(Some(row));
        }
        2 => {
            state.selected_album_row = row;
            data.albums_state.select(Some(row));
        }
        3 => data.following_state.select(Some(row)),
        _ => {}
    }
}

fn selected_original_index(state: &AppState) -> Option<usize> {
    if state.search_query.trim().is_empty() {
        return None;
    }
    state.search_matches.get(state.selected_row).copied()
}

pub(crate) fn handle_search_input(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
) -> Option<InputOutcome> {
    let mut handled = true;
    match key.code {
        KeyCode::Backspace => {
            let previously_selected = selected_original_index(state);
            state.search_query.pop();
            state.search_matches = build_search_matches(
                state.selected_subtab,
                &state.search_query,
                &data.likes,
                &data.playlists,
                &data.albums,
                &data.following,
            );

            if state.search_query.trim().is_empty() {
                if let Some(idx) = previously_selected {
                    set_primary_selection(state, data, idx);
                }
            } else {
                set_primary_selection(state, data, 0);
            }
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::SHIFT) && (c == 'f' || c == 'F') {
                let previously_selected = selected_original_index(state);
                if let Some(idx) = previously_selected {
                    set_primary_selection(state, data, idx);
                }
                state.search_popup_visible = false;
                state.search_query.clear();
                state.search_matches.clear();
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
                if !state.search_query.trim().is_empty() {
                    set_primary_selection(state, data, 0);
                }
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
