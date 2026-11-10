use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::InputOutcome;
use super::helpers::insert_manual_queue;
use crate::keymap::Action;
use crate::tui::logic::nav::{self, PaneId};
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

/// The queue popup is modal: movement scrolls the upcoming tracks, Esc or its own key
/// closes it, and every other key is dropped.
pub(crate) fn handle_queue_input(key: KeyEvent, state: &mut AppState) -> InputOutcome {
    let last = (state.manual_queue.len() + state.auto_queue.len()).saturating_sub(1);
    let scroll = &mut state.queue_scroll;
    match (key.code, state.keymap.action(&key)) {
        (KeyCode::Esc, _) | (_, Some(Action::ToggleQueue)) => state.queue_visible = false,
        (_, Some(Action::Up)) => *scroll = scroll.saturating_sub(1),
        (_, Some(Action::Down)) => *scroll = (*scroll + 1).min(last),
        (_, Some(Action::PageUp)) => *scroll = scroll.saturating_sub(10),
        (_, Some(Action::PageDown)) => *scroll = (*scroll + 10).min(last),
        (_, Some(Action::Top)) => *scroll = 0,
        (_, Some(Action::Bottom)) => *scroll = last,
        _ => {}
    }
    InputOutcome::Continue
}

/// The focused pane's selected track, ready to queue. `None` unless a tracks pane has
/// focus and its selected row is playable.
pub(crate) fn selected_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    if nav::focused(state).is_container() {
        return None;
    }
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
        get_feed_queued(state, data)
    }
}

fn get_feed_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = nav::cursor(state, PaneId::FeedTracks);
    let track = data.feed_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Feed, idx, track, Some(&data.feed_tracks), None, None, None, true,
    ))
}

fn get_search_tracks_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = nav::cursor(state, PaneId::SearchTracks);
    let track = data.search_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Playlist, idx, track, Some(&data.search_tracks), None, None, None, true,
    ))
}

fn get_search_playlist_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = nav::cursor(state, PaneId::SearchPlaylistTracks);
    let track = data.search_playlist_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Playlist, idx, track, Some(&data.search_playlist_tracks),
        data.search_playlist_tracks_uri.clone(), None, None, true,
    ))
}

fn get_search_album_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = nav::cursor(state, PaneId::SearchAlbumTracks);
    let track = data.search_album_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Album, idx, track, Some(&data.search_album_tracks),
        None, data.search_album_tracks_uri.clone(), None, true,
    ))
}

fn get_search_people_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    if state.search_people_tracks_focus == FollowingTracksFocus::Likes {
        let idx = nav::cursor(state, PaneId::SearchPeopleLikes);
        let track = data.search_people_likes_tracks.get(idx).filter(|t| t.is_playable())?;
        return Some(QueuedTrack::new(
            PlaybackSource::FollowingLikes, idx, track, Some(&data.search_people_likes_tracks),
            None, None, data.search_people_likes_user_urn.clone(), true,
        ));
    }
    let idx = nav::cursor(state, PaneId::SearchPeopleTracks);
    let track = data.search_people_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::FollowingPublished, idx, track, Some(&data.search_people_tracks),
        None, None, data.search_people_tracks_user_urn.clone(), true,
    ))
}

fn get_likes_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = nav::selected_index(state, PaneId::Likes)?;
    let track = data.likes.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(PlaybackSource::Likes, idx, track, None, None, None, None, true))
}

fn get_playlist_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = nav::selected_index(state, PaneId::PlaylistTracks)?;
    let track = data.playlist_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Playlist, idx, track, Some(&data.playlist_tracks),
        data.playlist_tracks_uri.clone(), None, None, true,
    ))
}

fn get_album_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    let idx = nav::cursor(state, PaneId::AlbumTracks);
    let track = data.album_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::Album, idx, track, Some(&data.album_tracks),
        None, data.album_tracks_uri.clone(), None, true,
    ))
}

fn get_following_queued(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    if state.following_tracks_focus == FollowingTracksFocus::Likes {
        let idx = nav::cursor(state, PaneId::FollowingLikes);
        let track = data.following_likes_tracks.get(idx).filter(|t| t.is_playable())?;
        return Some(QueuedTrack::new(
            PlaybackSource::FollowingLikes, idx, track, Some(&data.following_likes_tracks),
            None, None, data.following_likes_user_urn.clone(), true,
        ));
    }
    let idx = nav::cursor(state, PaneId::FollowingTracks);
    let track = data.following_tracks.get(idx).filter(|t| t.is_playable())?;
    Some(QueuedTrack::new(
        PlaybackSource::FollowingPublished, idx, track, Some(&data.following_tracks),
        None, None, data.following_tracks_user_urn.clone(), true,
    ))
}
