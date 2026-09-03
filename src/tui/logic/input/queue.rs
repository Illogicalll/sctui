use super::helpers::{filtered_row, insert_manual_queue};
use crate::tui::logic::state::{AppData, AppState, PlaybackSource, QueuedTrack, FollowingTracksFocus};

pub(crate) fn handle_add_to_queue(
    state: &mut AppState,
    data: &mut AppData,
) {
    if let Some(queued) = selected_queued(state, data) {
        insert_manual_queue(state, queued);
    }
}

pub(crate) fn handle_add_next_to_queue(
    state: &mut AppState,
    data: &mut AppData,
) {
    if let Some(queued) = selected_queued(state, data) {
        state.manual_queue.push_front(queued);
    }
}

fn selected_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    if state.selected_tab == 0 {
        match state.selected_subtab {
            0 => get_likes_queued(state, data),
            1 => get_playlist_queued(state, data),
            2 => get_album_queued(state, data),
            3 => get_following_queued(state, data),
            _ => None,
        }
    } else if state.selected_tab == 1 {
        match state.selected_searchfilter {
            0 => get_search_tracks_queued(state, data),
            1 => get_search_album_queued(state, data),
            2 => get_search_playlist_queued(state, data),
            3 => get_search_people_queued(state, data),
            _ => None,
        }
    } else {
        None
    }
}

fn get_search_tracks_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = state.selected_row;
    let track = data.search_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Playlist, idx, track, Some(&data.search_tracks), None, None, None, true,
    ))
}

fn get_search_playlist_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = state.search_selected_playlist_track_row;
    let track = data.search_playlist_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Playlist, idx, track, Some(&data.search_playlist_tracks),
        data.search_playlist_tracks_uri.clone(), None, None, true,
    ))
}

fn get_search_album_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = state.search_selected_album_track_row;
    let track = data.search_album_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Album, idx, track, Some(&data.search_album_tracks),
        None, data.search_album_tracks_uri.clone(), None, true,
    ))
}

fn get_search_people_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    if state.search_people_tracks_focus == FollowingTracksFocus::Likes {
        let idx = state.search_selected_person_like_row;
        let track = data.search_people_likes_tracks.get(idx).filter(|t| t.is_playable())?;
        return Some(QueuedTrack::new(
            PlaybackSource::FollowingLikes, idx, track, Some(&data.search_people_likes_tracks),
            None, None, data.search_people_likes_user_urn.clone(), true,
        ));
    }
    let idx = state.search_selected_person_track_row;
    let track = data.search_people_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::FollowingPublished, idx, track, Some(&data.search_people_tracks),
        None, None, data.search_people_tracks_user_urn.clone(), true,
    ))
}

fn get_likes_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = filtered_row(state, state.selected_row)?;
    let track = data.likes.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(PlaybackSource::Likes, idx, track, None, None, None, None, true))
}

fn get_playlist_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = filtered_row(state, state.selected_playlist_track_row)?;
    let track = data.playlist_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Playlist, idx, track, Some(&data.playlist_tracks),
        data.playlist_tracks_uri.clone(), None, None, true,
    ))
}

fn get_album_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = state.selected_album_track_row;
    let track = data.album_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Album, idx, track, Some(&data.album_tracks),
        None, data.album_tracks_uri.clone(), None, true,
    ))
}

fn get_following_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    if state.following_tracks_focus == FollowingTracksFocus::Likes {
        let idx = state.selected_following_like_row;
        let track = data.following_likes_tracks.get(idx).filter(|t| t.is_playable())?;
        return Some(QueuedTrack::new(
            PlaybackSource::FollowingLikes, idx, track, Some(&data.following_likes_tracks),
            None, None, data.following_likes_user_urn.clone(), true,
        ));
    }
    let idx = state.selected_following_track_row;
    let track = data.following_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::FollowingPublished, idx, track, Some(&data.following_tracks),
        None, None, data.following_tracks_user_urn.clone(), true,
    ))
}
