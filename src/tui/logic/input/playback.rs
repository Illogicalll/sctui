use super::InputOutcome;
use crate::api::Track;
use crate::tui::logic::state::{AppData, AppState, PlaybackSource, FollowingTracksFocus};
use crate::player::Player;
use crate::tui::logic::utils::{build_queue, queued_from_current};

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
    }
    InputOutcome::Continue
}

fn handle_search_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    match state.selected_searchfilter {
        0 => handle_search_tracks_enter(state, data, player),
        1 => handle_search_album_enter(state, data, player),
        2 => handle_search_playlist_enter(state, data, player),
        3 => handle_search_people_enter(state, data, player),
        _ => {}
    }
}

fn handle_search_tracks_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    let selected_idx = state.selected_row;
    let track = match data.search_tracks.get(selected_idx) {
        Some(track) => track,
        None => return,
    };
    if !track.is_playable() {
        return;
    }

    if state.playback_source != PlaybackSource::Playlist {
        state.playback_history.clear();
        state.manual_queue.clear();
    } else if let Some(queued) = queued_from_current(state, data) {
        if !(queued.source == PlaybackSource::Playlist && queued.index == selected_idx) {
            state.playback_history.push(queued);
        }
    }

    player.play(track.clone());
    state.playback_source = PlaybackSource::Playlist;
    state.override_playing = None;
    state.current_playing_index = Some(selected_idx);
    data.playback_tracks = data.search_tracks.clone();
    data.playback_playlist_uri = None;
    data.playback_album_uri = None;
    data.playback_following_user_urn = None;
    state.auto_queue = build_queue(selected_idx, &data.playback_tracks, state.shuffle_enabled);
}

fn handle_search_playlist_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    let selected_idx = state.search_selected_playlist_track_row;
    let track = match data.search_playlist_tracks.get(selected_idx) {
        Some(track) => track,
        None => return,
    };
    if !track.is_playable() {
        return;
    }

    if state.playback_source != PlaybackSource::Playlist {
        state.playback_history.clear();
        state.manual_queue.clear();
    } else if let Some(queued) = queued_from_current(state, data) {
        if !(queued.source == PlaybackSource::Playlist && queued.index == selected_idx) {
            state.playback_history.push(queued);
        }
    }

    player.play(track.clone());
    state.playback_source = PlaybackSource::Playlist;
    state.override_playing = None;
    state.current_playing_index = Some(selected_idx);
    data.playback_tracks = data.search_playlist_tracks.clone();
    data.playback_playlist_uri = data.search_playlist_tracks_uri.clone();
    data.playback_album_uri = None;
    data.playback_following_user_urn = None;
    state.auto_queue = build_queue(selected_idx, &data.playback_tracks, state.shuffle_enabled);
}

fn handle_search_album_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    let selected_idx = state.search_selected_album_track_row;
    let track = match data.search_album_tracks.get(selected_idx) {
        Some(track) => track,
        None => return,
    };
    if !track.is_playable() {
        return;
    }

    if state.playback_source != PlaybackSource::Album {
        state.playback_history.clear();
        state.manual_queue.clear();
    } else if let Some(queued) = queued_from_current(state, data) {
        if !(queued.source == PlaybackSource::Album && queued.index == selected_idx) {
            state.playback_history.push(queued);
        }
    }

    player.play(track.clone());
    state.playback_source = PlaybackSource::Album;
    state.override_playing = None;
    state.current_playing_index = Some(selected_idx);
    data.playback_tracks = data.search_album_tracks.clone();
    data.playback_playlist_uri = None;
    data.playback_album_uri = data.search_album_tracks_uri.clone();
    data.playback_following_user_urn = None;
    state.auto_queue = build_queue(selected_idx, &data.playback_tracks, state.shuffle_enabled);
}

fn handle_search_people_enter(state: &mut AppState, data: &mut AppData, player: &Player) {
    let (tracks, selected_idx, new_source, user_urn) =
        if state.search_people_tracks_focus == FollowingTracksFocus::Likes {
            (
                &data.search_people_likes_tracks,
                state.search_selected_person_like_row,
                PlaybackSource::FollowingLikes,
                data.search_people_likes_user_urn.clone(),
            )
        } else {
            (
                &data.search_people_tracks,
                state.search_selected_person_track_row,
                PlaybackSource::FollowingPublished,
                data.search_people_tracks_user_urn.clone(),
            )
        };

    let track = match tracks.get(selected_idx) {
        Some(track) => track,
        None => return,
    };
    if !track.is_playable() {
        return;
    }

    if state.playback_source != new_source {
        state.playback_history.clear();
        state.manual_queue.clear();
    } else if let Some(queued) = queued_from_current(state, data) {
        if !(queued.source == new_source && queued.index == selected_idx) {
            state.playback_history.push(queued);
        }
    }

    player.play(track.clone());
    state.playback_source = new_source;
    state.override_playing = None;
    state.current_playing_index = Some(selected_idx);
    data.playback_tracks = tracks.clone();
    data.playback_playlist_uri = None;
    data.playback_album_uri = None;
    data.playback_following_user_urn = user_urn;
    state.auto_queue = build_queue(selected_idx, &data.playback_tracks, state.shuffle_enabled);
}

fn handle_likes_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
    let selected_idx = if search_active {
        state.search_matches.get(state.selected_row).copied()
    } else {
        Some(state.selected_row)
    };
    if let Some(selected_idx) = selected_idx {
        if let Some(track) = data.likes.get(selected_idx) {
            if !track.is_playable() {
                return;
            }
            if state.playback_source != PlaybackSource::Likes {
                state.playback_history.clear();
                state.manual_queue.clear();
            } else if let Some(queued) = queued_from_current(state, data) {
                if !(queued.source == PlaybackSource::Likes && queued.index == selected_idx) {
                    state.playback_history.push(queued);
                }
            }
            player.play(track.clone());
            state.playback_source = PlaybackSource::Likes;
            state.override_playing = None;
            state.current_playing_index = Some(selected_idx);
            data.playback_playlist_uri = None;
            data.playback_album_uri = None;
            data.playback_following_user_urn = None;
            state.auto_queue = build_queue(selected_idx, &data.likes, state.shuffle_enabled);
            if !search_active {
                if state.selected_tab == 0 && state.selected_subtab == 0 {
                    state.selected_row = selected_idx;
                    data.likes_state.select(Some(state.selected_row));
                }
            }
        }
    }
}

fn handle_playlist_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
    let selected_idx = if search_active {
        state
            .search_matches
            .get(state.selected_playlist_track_row)
            .copied()
    } else {
        Some(state.selected_playlist_track_row)
    };
    if let Some(selected_idx) = selected_idx {
        let track = match data.playlist_tracks.get(selected_idx) {
            Some(track) => track,
            None => return,
        };
        if !track.is_playable() {
            return;
        }
        if state.playback_source != PlaybackSource::Playlist {
            state.playback_history.clear();
            state.manual_queue.clear();
        } else if let Some(queued) = queued_from_current(state, data) {
            if !(queued.source == PlaybackSource::Playlist && queued.index == selected_idx) {
                state.playback_history.push(queued);
            }
        }
        player.play(track.clone());
        state.playback_source = PlaybackSource::Playlist;
        state.override_playing = None;
        state.current_playing_index = Some(selected_idx);
        data.playback_tracks = data.playlist_tracks.clone();
        data.playback_playlist_uri = data.playlist_tracks_uri.clone();
        data.playback_album_uri = None;
        data.playback_following_user_urn = None;
        state.auto_queue = build_queue(
            selected_idx,
            &data.playback_tracks,
            state.shuffle_enabled,
        );
        if !search_active {
            data.playlist_tracks_state.select(Some(state.selected_playlist_track_row));
        }
    }
}

fn handle_album_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    if let Some(track) = data.album_tracks.get(state.selected_album_track_row) {
        if !track.is_playable() {
            return;
        }
        if state.playback_source != PlaybackSource::Album {
            state.playback_history.clear();
            state.manual_queue.clear();
        } else if let Some(queued) = queued_from_current(state, data) {
            if !(queued.source == PlaybackSource::Album
                && queued.index == state.selected_album_track_row)
            {
                state.playback_history.push(queued);
            }
        }
        player.play(track.clone());
        state.playback_source = PlaybackSource::Album;
        state.override_playing = None;
        state.current_playing_index = Some(state.selected_album_track_row);
        data.playback_tracks = data.album_tracks.clone();
        data.playback_playlist_uri = None;
        data.playback_album_uri = data.album_tracks_uri.clone();
        data.playback_following_user_urn = None;
        state.auto_queue = build_queue(
            state.selected_album_track_row,
            &data.playback_tracks,
            state.shuffle_enabled,
        );
        data.album_tracks_state.select(Some(state.selected_album_track_row));
    }
}

fn handle_following_enter(
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) {
    let (tracks, selected_idx, new_source, user_urn) =
        if state.following_tracks_focus == FollowingTracksFocus::Likes {
            (
                &data.following_likes_tracks,
                state.selected_following_like_row,
                PlaybackSource::FollowingLikes,
                data.following_likes_user_urn.clone(),
            )
        } else {
            (
                &data.following_tracks,
                state.selected_following_track_row,
                PlaybackSource::FollowingPublished,
                data.following_tracks_user_urn.clone(),
            )
        };
    if let Some(track) = tracks.get(selected_idx) {
        if !track.is_playable() {
            return;
        }
        if state.playback_source != new_source {
            state.playback_history.clear();
            state.manual_queue.clear();
        } else if let Some(queued) = queued_from_current(state, data) {
            if !(queued.source == new_source && queued.index == selected_idx) {
                state.playback_history.push(queued);
            }
        }
        player.play(track.clone());
        state.playback_source = new_source;
        state.override_playing = None;
        state.current_playing_index = Some(selected_idx);
        data.playback_tracks = tracks.clone();
        data.playback_playlist_uri = None;
        data.playback_album_uri = None;
        data.playback_following_user_urn = user_urn;
        state.auto_queue = build_queue(
            selected_idx,
            &data.playback_tracks,
            state.shuffle_enabled,
        );
        if new_source == PlaybackSource::FollowingLikes {
            data.following_likes_state.select(Some(selected_idx));
        } else {
            data.following_tracks_state.select(Some(selected_idx));
        }
    }
}
