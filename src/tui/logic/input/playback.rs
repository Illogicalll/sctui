use super::InputOutcome;
use crate::api::{Artist, Track};
use crate::tui::logic::nav::{self, PaneId};
use crate::tui::logic::state::{AppData, AppState, PlaybackSource, FollowingTracksFocus};
use crate::player::{Player, TrackChange};
use crate::tui::logic::utils::{build_queue, enter_radio, queued_from_current};

use super::queue::selected_queued;

/// The artist row itself, when it (not either track pane) has focus.
pub(crate) fn focused_artist(state: &AppState, data: &AppData) -> Option<Artist> {
    let id = nav::focused(state);
    let people = match id {
        PaneId::Following => &data.following,
        PaneId::SearchPeople => &data.search_people,
        _ => return None,
    };
    people.get(nav::selected_index(state, id)?).cloned()
}

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

/// Play the selected track as a station — related tracks follow.
/// On a focused artist row, starts an artist station instead; the loop fetches it in the
/// background (there is no synchronous path from here to the network).
pub(crate) fn handle_station(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if let Some(artist) = focused_artist(state, data) {
        state.toast(format!("Starting station from {}", artist.name));
        state.artist_station_request = Some(artist);
        return InputOutcome::Continue;
    }
    let Some(queued) = selected_queued(state, data) else {
        state.toast_err("Select a track or an artist first");
        return InputOutcome::Continue;
    };
    if !queued.track.is_playable() {
        return InputOutcome::Continue;
    }
    state.toast(format!("Station started from {}", queued.track.title));
    if let Some(current) = queued_from_current(state, data) {
        state.push_history(current);
    }
    state.manual_queue.clear();
    player.play(queued.track.clone(), TrackChange::UserSkip);
    // Choosing something to play means play it, paused or not.
    player.resume();
    enter_radio(state, data, vec![queued.track]);
    InputOutcome::Continue
}

fn handle_feed_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    let idx = nav::cursor(state, PaneId::FeedTracks);
    if start_playback(state, data, player, PlaybackSource::Feed, |d| &d.feed_tracks, idx, None, None, None) {
        data.feed_tracks_state.select(Some(idx));
        state.feed_expand_from = Some(nav::cursor(state, PaneId::Feed));
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
            state.push_history(queued);
        }

    player.play(track.clone(), TrackChange::UserSkip);
    // Choosing something to play means play it, paused or not. Next/previous keep the pause.
    player.resume();
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
            let idx = nav::cursor(state, PaneId::SearchTracks);
            start_playback(state, data, player, PlaybackSource::Playlist, |d| &d.search_tracks, idx, None, None, None);
        }
        1 => {
            let idx = nav::cursor(state, PaneId::SearchAlbumTracks);
            let uri = data.search_album_tracks_uri.clone();
            start_playback(state, data, player, PlaybackSource::Album, |d| &d.search_album_tracks, idx, None, uri, None);
        }
        2 => {
            let idx = nav::cursor(state, PaneId::SearchPlaylistTracks);
            let uri = data.search_playlist_tracks_uri.clone();
            start_playback(state, data, player, PlaybackSource::Playlist, |d| &d.search_playlist_tracks, idx, uri, None, None);
        }
        3 => handle_search_people_enter(state, data, player),
        _ => {}
    }
}

fn handle_search_people_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    if state.search_people_tracks_focus == FollowingTracksFocus::Likes {
        let idx = nav::cursor(state, PaneId::SearchPeopleLikes);
        let urn = data.search_people_likes_user_urn.clone();
        start_playback(state, data, player, PlaybackSource::FollowingLikes, |d| &d.search_people_likes_tracks, idx, None, None, urn);
    } else {
        let idx = nav::cursor(state, PaneId::SearchPeopleTracks);
        let urn = data.search_people_tracks_user_urn.clone();
        start_playback(state, data, player, PlaybackSource::FollowingPublished, |d| &d.search_people_tracks, idx, None, None, urn);
    }
}

fn handle_likes_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let Some(selected_idx) = nav::selected_index(state, PaneId::Likes) else {
        return;
    };
    start_playback(state, data, player, PlaybackSource::Likes, |d| &d.likes, selected_idx, None, None, None);
}

fn handle_playlist_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let Some(selected_idx) = nav::selected_index(state, PaneId::PlaylistTracks) else {
        return;
    };
    let uri = data.playlist_tracks_uri.clone();
    start_playback(state, data, player, PlaybackSource::Playlist, |d| &d.playlist_tracks, selected_idx, uri, None, None);
}

fn handle_album_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let idx = nav::cursor(state, PaneId::AlbumTracks);
    let uri = data.album_tracks_uri.clone();
    if start_playback(state, data, player, PlaybackSource::Album, |d| &d.album_tracks, idx, None, uri, None) {
        data.album_tracks_state.select(Some(idx));
    }
}

fn handle_following_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    if state.following_tracks_focus == FollowingTracksFocus::Likes {
        let idx = nav::cursor(state, PaneId::FollowingLikes);
        let urn = data.following_likes_user_urn.clone();
        if start_playback(state, data, player, PlaybackSource::FollowingLikes, |d| &d.following_likes_tracks, idx, None, None, urn) {
            data.following_likes_state.select(Some(idx));
        }
    } else {
        let idx = nav::cursor(state, PaneId::FollowingTracks);
        let urn = data.following_tracks_user_urn.clone();
        if start_playback(state, data, player, PlaybackSource::FollowingPublished, |d| &d.following_tracks, idx, None, None, urn) {
            data.following_tracks_state.select(Some(idx));
        }
    }
}
