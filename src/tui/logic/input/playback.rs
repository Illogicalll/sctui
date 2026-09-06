use super::InputOutcome;
use super::helpers::filtered_row;
use crate::api::Track;
use crate::tui::logic::state::{AppData, AppState, PlaybackSource, FollowingTracksFocus};
use crate::player::Player;
use crate::tui::logic::utils::{build_queue, enter_radio, queued_from_current};

use super::queue::selected_queued;

pub(crate) fn handle_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if state.selected_tab == 1 {
        handle_search_enter(state, data, player);
    } else if state.selected_tab == 0 && state.selected_subtab == 0 {
        handle_likes_enter(state, data, player);
    } else if state.selected_tab == 0 && state.selected_subtab == 1 {
        handle_playlist_enter(state, data, player);
    } else if state.selected_tab == 0 && state.selected_subtab == 2 {
        handle_album_enter(state, data, player);
    } else if state.selected_tab == 0 && state.selected_subtab == 3 {
        handle_following_enter(state, data, player);
    } else if state.selected_tab == 2 {
        handle_feed_enter(state, data, player);
    }
    InputOutcome::Continue
}

/// Shift+Enter (or Shift+G): play the selected track as a station — related tracks follow.
pub(crate) fn handle_station(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    let Some(queued) = selected_queued(state, data) else {
        return InputOutcome::Continue;
    };
    if !queued.track.is_playable() {
        return InputOutcome::Continue;
    }
    if let Some(current) = queued_from_current(state, data) {
        state.playback_history.push(current);
    }
    state.manual_queue.clear();
    player.play(queued.track.clone());
    enter_radio(state, data, queued.track);
    InputOutcome::Continue
}

/// Enter on an activity moves focus to its tracks; Enter on a track plays it.
fn handle_feed_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    if !state.info_pane_selected {
        state.info_pane_selected = true;
        return;
    }
    let idx = state.selected_info_row;
    if start_playback(state, data, player, PlaybackSource::Feed, |d| &d.feed_tracks, idx, None, None, None) {
        data.feed_tracks_state.select(Some(idx));
        state.feed_expand_from = Some(state.selected_row);
    }
}

/// Shared Enter core. Returns `false` (without side effects) when `tracks(data)[idx]` is missing or
/// unplayable. `tracks` is a field selector rather than a slice so `data` can still be mutated here.
fn start_playback(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
    source: PlaybackSource,
    tracks: fn(&AppData) -> &Vec<Track>,
    idx: usize,
    playlist_uri: Option<String>,
    album_uri: Option<String>,
    following_user_urn: Option<String>,
) -> bool {
    let Some(track) = tracks(data).get(idx) else {
        return false;
    };
    if !track.is_playable() {
        return false;
    }

    if state.playback_source != source {
        state.playback_history.clear();
        state.manual_queue.clear();
    } else if let Some(queued) = queued_from_current(state, data)
        && !(queued.source == source && queued.index == idx) {
            state.playback_history.push(queued);
        }

    player.play(track.clone());
    state.playback_source = source;
    state.override_playing = None;
    state.current_playing_index = Some(idx);
    if source != PlaybackSource::Likes {
        data.playback_tracks = tracks(data).clone();
    }
    data.playback_playlist_uri = playlist_uri;
    data.playback_album_uri = album_uri;
    data.playback_following_user_urn = following_user_urn;
    state.auto_queue = build_queue(idx, tracks(data), state.shuffle_enabled);
    true
}

fn handle_search_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    match state.selected_searchfilter {
        0 => {
            let idx = state.selected_row;
            start_playback(state, data, player, PlaybackSource::Playlist, |d| &d.search_tracks, idx, None, None, None);
        }
        1 => {
            let idx = state.search_selected_album_track_row;
            let uri = data.search_album_tracks_uri.clone();
            start_playback(state, data, player, PlaybackSource::Album, |d| &d.search_album_tracks, idx, None, uri, None);
        }
        2 => {
            let idx = state.search_selected_playlist_track_row;
            let uri = data.search_playlist_tracks_uri.clone();
            start_playback(state, data, player, PlaybackSource::Playlist, |d| &d.search_playlist_tracks, idx, uri, None, None);
        }
        3 => handle_search_people_enter(state, data, player),
        _ => {}
    }
}

fn handle_search_people_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    if state.search_people_tracks_focus == FollowingTracksFocus::Likes {
        let idx = state.search_selected_person_like_row;
        let urn = data.search_people_likes_user_urn.clone();
        start_playback(state, data, player, PlaybackSource::FollowingLikes, |d| &d.search_people_likes_tracks, idx, None, None, urn);
    } else {
        let idx = state.search_selected_person_track_row;
        let urn = data.search_people_tracks_user_urn.clone();
        start_playback(state, data, player, PlaybackSource::FollowingPublished, |d| &d.search_people_tracks, idx, None, None, urn);
    }
}

fn handle_likes_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
    let Some(selected_idx) = filtered_row(state, state.selected_row) else {
        return;
    };
    if start_playback(state, data, player, PlaybackSource::Likes, |d| &d.likes, selected_idx, None, None, None)
        && !search_active
        && state.selected_tab == 0
        && state.selected_subtab == 0
    {
        state.selected_row = selected_idx;
        data.likes_state.select(Some(state.selected_row));
    }
}

fn handle_playlist_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
    let Some(selected_idx) = filtered_row(state, state.selected_playlist_track_row) else {
        return;
    };
    let uri = data.playlist_tracks_uri.clone();
    if start_playback(state, data, player, PlaybackSource::Playlist, |d| &d.playlist_tracks, selected_idx, uri, None, None)
        && !search_active
    {
        data.playlist_tracks_state.select(Some(state.selected_playlist_track_row));
    }
}

fn handle_album_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let idx = state.selected_album_track_row;
    let uri = data.album_tracks_uri.clone();
    if start_playback(state, data, player, PlaybackSource::Album, |d| &d.album_tracks, idx, None, uri, None) {
        data.album_tracks_state.select(Some(state.selected_album_track_row));
    }
}

fn handle_following_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    if state.following_tracks_focus == FollowingTracksFocus::Likes {
        let idx = state.selected_following_like_row;
        let urn = data.following_likes_user_urn.clone();
        if start_playback(state, data, player, PlaybackSource::FollowingLikes, |d| &d.following_likes_tracks, idx, None, None, urn) {
            data.following_likes_state.select(Some(idx));
        }
    } else {
        let idx = state.selected_following_track_row;
        let urn = data.following_tracks_user_urn.clone();
        if start_playback(state, data, player, PlaybackSource::FollowingPublished, |d| &d.following_tracks, idx, None, None, urn) {
            data.following_tracks_state.select(Some(idx));
        }
    }
}
