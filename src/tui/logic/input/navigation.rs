use super::InputOutcome;
use crate::player::{Player, TrackChange};
use crate::tui::logic::nav;
use crate::tui::logic::state::{AppData, AppState, visible_tabs};
use crate::tui::logic::utils::{active_tracks, build_queue};

pub(crate) fn handle_tab_switch(state: &mut AppState) -> InputOutcome {
    let tabs = visible_tabs(state).len();
    nav::set_location(state, (state.selected_tab + 1) % tabs, None);
    InputOutcome::Continue
}

pub(crate) fn handle_tab_switch_back(state: &mut AppState) -> InputOutcome {
    let tabs = visible_tabs(state).len();
    nav::set_location(state, (state.selected_tab + tabs - 1) % tabs, None);
    InputOutcome::Continue
}

/// Jump straight to a main tab. A tab that is not showing (the Feed, when hidden in
/// Settings) is ignored.
pub(crate) fn go_to_tab(state: &mut AppState, tab: usize) -> InputOutcome {
    if tab < visible_tabs(state).len() {
        nav::set_location(state, tab, None);
    }
    InputOutcome::Continue
}

/// Step the sub-tab (Library) or search type (Search) by `step`, wrapping; 3 is one to
/// the left. The Feed has none.
pub(crate) fn sub_tab(state: &mut AppState, step: usize) -> InputOutcome {
    let (tab, sub) = nav::location(state);
    if tab < 2 {
        nav::set_location(state, tab, Some((sub + step) % 4));
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
                state.push_history(current);
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
                    state.push_history(current);
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

    #[test]
    fn number_keys_go_to_a_tab_and_the_hidden_feed_is_inert() {
        let mut state = AppState::default();
        sub_tab(&mut state, 1);
        for tab in [2, 1, 0] {
            go_to_tab(&mut state, tab);
            assert_eq!(state.selected_tab, tab);
        }
        assert_eq!(state.selected_subtab, 1, "the tab's own sub-tab is kept");
        state.settings.hide_feed_tab = true;
        go_to_tab(&mut state, 2);
        assert_eq!(state.selected_tab, 0);
        go_to_tab(&mut state, 1);
        assert_eq!(state.selected_tab, 1);
    }

    #[test]
    fn sub_tabs_wrap_both_ways_and_the_feed_has_none() {
        let mut state = AppState::default();
        sub_tab(&mut state, 3);
        assert_eq!(state.selected_subtab, 3);
        sub_tab(&mut state, 1);
        assert_eq!(state.selected_subtab, 0);
        handle_tab_switch(&mut state);
        sub_tab(&mut state, 1);
        assert_eq!((state.selected_searchfilter, state.search_needs_fetch), (1, true));
        handle_tab_switch(&mut state);
        sub_tab(&mut state, 1);
        assert_eq!((state.selected_tab, nav::focus(&state)), (2, 0), "h/l never move pane focus");
    }
}
