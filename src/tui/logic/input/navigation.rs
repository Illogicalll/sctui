use super::InputOutcome;
use super::helpers::reset_search_rows;
use crate::player::{Player, TrackChange};
use crate::tui::logic::state::{AppData, AppState, visible_tabs};
use crate::tui::logic::utils::{active_tracks, build_queue, build_search_matches};

pub(crate) fn handle_tab_switch(state: &mut AppState) -> InputOutcome {
    let tabs = visible_tabs(state).len();
    state.selected_tab = (state.selected_tab + 1) % tabs;
    after_tab_switch(state);
    InputOutcome::Continue
}

pub(crate) fn handle_tab_switch_back(state: &mut AppState) -> InputOutcome {
    let tabs = visible_tabs(state).len();
    state.selected_tab = (state.selected_tab + tabs - 1) % tabs;
    after_tab_switch(state);
    InputOutcome::Continue
}

/// Landing on an empty Search tab drops straight into typing.
fn after_tab_switch(state: &mut AppState) {
    state.selected_row = 0;
    state.search_typing = state.selected_tab == 1 && state.query.is_empty();
}

pub(crate) fn sub_tab_right(state: &mut AppState, data: &mut AppData) -> InputOutcome {
    if state.selected_tab == 0 {
        if state.selected_subtab == 1 {
            state.selected_playlist_row = state.selected_row;
        }
        if state.selected_subtab == 2 {
            state.selected_album_row = state.selected_row;
        }
        state.selected_subtab = (state.selected_subtab + 1) % 4;
        if state.selected_subtab == 1 {
            state.selected_row = state.selected_playlist_row;
            data.playlists_state.select(Some(state.selected_row));
        } else if state.selected_subtab == 2 {
            state.selected_row = state.selected_album_row;
            data.albums_state.select(Some(state.selected_row));
        } else {
            state.selected_row = 0;
        }

        if state.search_popup_visible && !state.search_query.trim().is_empty() {
            state.search_matches = build_search_matches(
                state.selected_subtab,
                &state.search_query,
                &data.likes,
                &data.playlist_tracks,
                &data.albums,
                &data.following,
            );
            state.selected_row = 0;
            match state.selected_subtab {
                0 => data.likes_state.select(Some(0)),
                1 => {
                    state.selected_playlist_track_row = 0;
                    data.playlist_tracks_state.select(Some(0));
                }
                2 => {
                    state.selected_album_row = 0;
                    data.albums_state.select(Some(0));
                }
                3 => data.following_state.select(Some(0)),
                _ => {}
            }
        }
    } else if state.selected_tab == 1 {
        state.selected_searchfilter = (state.selected_searchfilter + 1) % 4;
        reset_search_rows(state);
        state.search_needs_fetch = true;
        data.search_tracks_state.select(Some(0));
        data.search_albums_state.select(Some(0));
        data.search_playlists_state.select(Some(0));
        data.search_people_state.select(Some(0));
    } else if state.selected_tab == 2 {
        state.info_pane_selected = !state.info_pane_selected;
    }

    InputOutcome::Continue
}

pub(crate) fn sub_tab_left(state: &mut AppState, data: &mut AppData) -> InputOutcome {
    if state.selected_tab == 0 {
        if state.selected_subtab == 1 {
            state.selected_playlist_row = state.selected_row;
        }
        if state.selected_subtab == 2 {
            state.selected_album_row = state.selected_row;
        }
        state.selected_subtab = if state.selected_subtab == 0 {
            3
        } else {
            state.selected_subtab - 1
        };
        if state.selected_subtab == 1 {
            state.selected_row = state.selected_playlist_row;
            data.playlists_state.select(Some(state.selected_row));
        } else if state.selected_subtab == 2 {
            state.selected_row = state.selected_album_row;
            data.albums_state.select(Some(state.selected_row));
        } else {
            state.selected_row = 0;
        }

        if state.search_popup_visible && !state.search_query.trim().is_empty() {
            state.search_matches = build_search_matches(
                state.selected_subtab,
                &state.search_query,
                &data.likes,
                &data.playlist_tracks,
                &data.albums,
                &data.following,
            );
            state.selected_row = 0;
            match state.selected_subtab {
                0 => data.likes_state.select(Some(0)),
                1 => {
                    state.selected_playlist_track_row = 0;
                    data.playlist_tracks_state.select(Some(0));
                }
                2 => {
                    state.selected_album_row = 0;
                    data.albums_state.select(Some(0));
                }
                3 => data.following_state.select(Some(0)),
                _ => {}
            }
        }
    } else if state.selected_tab == 1 {
        state.selected_searchfilter = if state.selected_searchfilter == 0 {
            3
        } else {
            state.selected_searchfilter - 1
        };
        reset_search_rows(state);
        state.search_needs_fetch = true;
        data.search_tracks_state.select(Some(0));
        data.search_albums_state.select(Some(0));
        data.search_playlists_state.select(Some(0));
        data.search_people_state.select(Some(0));
    } else if state.selected_tab == 2 {
        state.info_pane_selected = !state.info_pane_selected;
    }

    InputOutcome::Continue
}

pub(crate) fn handle_next_track(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if let Some(current_idx) = state.current_playing_index {
        let active_tracks = active_tracks(state, data);
        if state.manual_queue.is_empty() && state.auto_queue.is_empty() {
            state.auto_queue =
                build_queue(current_idx, active_tracks, state.shuffle_enabled);
        }
        if let Some(queued) = state.manual_queue.pop_front() {
            if let Some(current) = crate::tui::logic::utils::queued_from_current(state, data) {
                state.playback_history.push(current);
            }
            crate::tui::logic::utils::play_queued_track(
                queued,
                state,
                data,
                player,
                true,
                TrackChange::UserSkip,
            );
        } else if let Some(next_idx) = state.auto_queue.pop_front()
            && let Some(track) = active_tracks.get(next_idx) {
                if let Some(current) = crate::tui::logic::utils::queued_from_current(state, data) {
                    state.playback_history.push(current);
                }
                player.play(track.clone(), TrackChange::UserSkip);
                state.override_playing = None;
                state.current_playing_index = Some(next_idx);
            }
    }
    InputOutcome::Continue
}

pub(crate) fn handle_prev_track(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if state.current_playing_index.is_some()
        && let Some(prev) = state.playback_history.pop() {
            if let Some(current) = crate::tui::logic::utils::queued_from_current(state, data) {
                let mut current = current;
                current.user_added = false;
                state.manual_queue.push_front(current);
            }
            crate::tui::logic::utils::play_queued_track(
                prev,
                state,
                data,
                player,
                true,
                TrackChange::UserSkip,
            );
        }
    InputOutcome::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabs_in_order(hide_feed_tab: bool) -> Vec<usize> {
        let mut state = AppState::default();
        state.settings.hide_feed_tab = hide_feed_tab;
        let mut seen = vec![state.selected_tab];
        for _ in 0..3 {
            handle_tab_switch(&mut state);
            seen.push(state.selected_tab);
        }
        seen
    }

    #[test]
    fn tab_cycle_skips_the_feed_when_it_is_hidden() {
        assert_eq!(tabs_in_order(false), [0, 1, 2, 0]);
        assert_eq!(tabs_in_order(true), [0, 1, 0, 1]);
    }

    #[test]
    fn tab_cycle_back_wraps_to_the_last_visible_tab() {
        let mut state = AppState::default();
        handle_tab_switch_back(&mut state);
        assert_eq!(state.selected_tab, 2);

        let mut state = AppState::default();
        state.settings.hide_feed_tab = true;
        handle_tab_switch_back(&mut state);
        assert_eq!(state.selected_tab, 1);
    }
}
