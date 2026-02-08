use super::helpers::insert_manual_queue;
use crate::api::Track;
use crate::tui::logic::state::{AppData, AppState, PlaybackSource, QueuedTrack, FollowingTracksFocus};

pub(crate) fn handle_add_to_queue(
    state: &mut AppState,
    data: &mut AppData,
) {
    if state.selected_tab == 0 {
        match state.selected_subtab {
            0 => add_likes_to_queue(state, data),
            1 => add_playlist_to_queue(state, data),
            2 => add_album_to_queue(state, data),
            3 => add_following_to_queue(state, data),
            _ => {}
        }
    }
}

pub(crate) fn handle_add_next_to_queue(
    state: &mut AppState,
    data: &mut AppData,
) {
    if state.selected_tab == 0 {
        let mut queued: Option<QueuedTrack> = None;
        match state.selected_subtab {
            0 => queued = get_likes_queued(state, data),
            1 => queued = get_playlist_queued(state, data),
            2 => queued = get_album_queued(state, data),
            3 => queued = get_following_queued(state, data),
            _ => {}
        }

        if let Some(queued) = queued {
            state.manual_queue.push_front(queued);
        }
    }
}

fn add_likes_to_queue(state: &mut AppState, data: &mut AppData) {
    let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
    let selected_idx = if search_active {
        state.search_matches.get(state.selected_row).copied()
    } else {
        Some(state.selected_row)
    };
    if let Some(idx) = selected_idx {
        if let Some(track) = data.likes.get(idx) {
            if track.is_playable() {
                insert_manual_queue(
                    state,
                    QueuedTrack {
                        source: PlaybackSource::Likes,
                        index: idx,
                        track: track.clone(),
                        tracks_snapshot: None,
                        playlist_uri: None,
                        album_uri: None,
                        following_user_urn: None,
                        user_added: true,
                    },
                );
            }
        }
    }
}

fn add_playlist_to_queue(state: &mut AppState, data: &mut AppData) {
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
        if track.is_playable() {
            insert_manual_queue(
                state,
                QueuedTrack {
                    source: PlaybackSource::Playlist,
                    index: selected_idx,
                    track: track.clone(),
                    tracks_snapshot: Some(data.playlist_tracks.clone()),
                    playlist_uri: data.playlist_tracks_uri.clone(),
                    album_uri: None,
                    following_user_urn: None,
                    user_added: true,
                },
            );
        }
    }
}

fn add_album_to_queue(state: &mut AppState, data: &mut AppData) {
    if let Some(track) = data.album_tracks.get(state.selected_album_track_row) {
        if track.is_playable() {
            insert_manual_queue(
                state,
                QueuedTrack {
                    source: PlaybackSource::Album,
                    index: state.selected_album_track_row,
                    track: track.clone(),
                    tracks_snapshot: Some(data.album_tracks.clone()),
                    playlist_uri: None,
                    album_uri: data.album_tracks_uri.clone(),
                    following_user_urn: None,
                    user_added: true,
                },
            );
        }
    }
}

fn add_following_to_queue(state: &mut AppState, data: &mut AppData) {
    if state.following_tracks_focus == FollowingTracksFocus::Likes {
        if let Some(track) = data
            .following_likes_tracks
            .get(state.selected_following_like_row)
        {
            if track.is_playable() {
                insert_manual_queue(
                    state,
                    QueuedTrack {
                        source: PlaybackSource::FollowingLikes,
                        index: state.selected_following_like_row,
                        track: track.clone(),
                        tracks_snapshot: Some(data.following_likes_tracks.clone()),
                        playlist_uri: None,
                        album_uri: None,
                        following_user_urn: data.following_likes_user_urn.clone(),
                        user_added: true,
                    },
                );
            }
        }
    } else if let Some(track) = data
        .following_tracks
        .get(state.selected_following_track_row)
    {
        if track.is_playable() {
            insert_manual_queue(
                state,
                QueuedTrack {
                    source: PlaybackSource::FollowingPublished,
                    index: state.selected_following_track_row,
                    track: track.clone(),
                    tracks_snapshot: Some(data.following_tracks.clone()),
                    playlist_uri: None,
                    album_uri: None,
                    following_user_urn: data.following_tracks_user_urn.clone(),
                    user_added: true,
                },
            );
        }
    }
}

fn get_likes_queued(state: &mut AppState, data: &mut AppData) -> Option<QueuedTrack> {
    let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
    let selected_idx = if search_active {
        state.search_matches.get(state.selected_row).copied()
    } else {
        Some(state.selected_row)
    };
    if let Some(idx) = selected_idx {
        if let Some(track) = data.likes.get(idx) {
            if track.is_playable() {
                return Some(QueuedTrack {
                    source: PlaybackSource::Likes,
                    index: idx,
                    track: track.clone(),
                    tracks_snapshot: None,
                    playlist_uri: None,
                    album_uri: None,
                    following_user_urn: None,
                    user_added: true,
                });
            }
        }
    }
    None
}

fn get_playlist_queued(state: &mut AppState, data: &mut AppData) -> Option<QueuedTrack> {
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
        let track = data.playlist_tracks.get(selected_idx)?;
        if track.is_playable() {
            return Some(QueuedTrack {
                source: PlaybackSource::Playlist,
                index: selected_idx,
                track: track.clone(),
                tracks_snapshot: Some(data.playlist_tracks.clone()),
                playlist_uri: data.playlist_tracks_uri.clone(),
                album_uri: None,
                following_user_urn: None,
                user_added: true,
            });
        }
    }
    None
}

fn get_album_queued(state: &mut AppState, data: &mut AppData) -> Option<QueuedTrack> {
    if let Some(track) = data.album_tracks.get(state.selected_album_track_row) {
        if track.is_playable() {
            return Some(QueuedTrack {
                source: PlaybackSource::Album,
                index: state.selected_album_track_row,
                track: track.clone(),
                tracks_snapshot: Some(data.album_tracks.clone()),
                playlist_uri: None,
                album_uri: data.album_tracks_uri.clone(),
                following_user_urn: None,
                user_added: true,
            });
        }
    }
    None
}

fn get_following_queued(state: &mut AppState, data: &mut AppData) -> Option<QueuedTrack> {
    if state.following_tracks_focus == FollowingTracksFocus::Likes {
        if let Some(track) = data
            .following_likes_tracks
            .get(state.selected_following_like_row)
        {
            if track.is_playable() {
                return Some(QueuedTrack {
                    source: PlaybackSource::FollowingLikes,
                    index: state.selected_following_like_row,
                    track: track.clone(),
                    tracks_snapshot: Some(data.following_likes_tracks.clone()),
                    playlist_uri: None,
                    album_uri: None,
                    following_user_urn: data.following_likes_user_urn.clone(),
                    user_added: true,
                });
            }
        }
    } else if let Some(track) = data
        .following_tracks
        .get(state.selected_following_track_row)
    {
        if track.is_playable() {
            return Some(QueuedTrack {
                source: PlaybackSource::FollowingPublished,
                index: state.selected_following_track_row,
                track: track.clone(),
                tracks_snapshot: Some(data.following_tracks.clone()),
                playlist_uri: None,
                album_uri: None,
                following_user_urn: data.following_tracks_user_urn.clone(),
                user_added: true,
            });
        }
    }
    None
}
