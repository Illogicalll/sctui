use std::collections::VecDeque;

use rand::seq::SliceRandom;

use crate::api::{Album, Artist, Playlist, Track};
use crate::player::Player;

use super::state::{AppData, AppState, FollowingTracksFocus, PlaybackSource, QueuedTrack};
pub fn build_search_matches(
    selected_subtab: usize,
    query: &str,
    likes: &Vec<Track>,
    playlists: &Vec<Playlist>,
    albums: &Vec<Album>,
    following: &Vec<Artist>,
) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    match selected_subtab {
        0 => likes
            .iter()
            .enumerate()
            .filter(|(_, track)| {
                track.title.to_lowercase().contains(&q)
                    || track.artists.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect(),
        1 => playlists
            .iter()
            .enumerate()
            .filter(|(_, playlist)| playlist.title.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect(),
        2 => albums
            .iter()
            .enumerate()
            .filter(|(_, album)| {
                album.title.to_lowercase().contains(&q)
                    || album.artists.to_lowercase().contains(&q)
            })
            .map(|(i, _)| i)
            .collect(),
        3 => following
            .iter()
            .enumerate()
            .filter(|(_, artist)| artist.name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect(),
        _ => Vec::new(),
    }
}

pub fn build_queue(
    current_idx: usize,
    tracks: &[Track],
    shuffle_enabled: bool,
) -> VecDeque<usize> {
    if tracks.is_empty() {
        return VecDeque::new();
    }

    if shuffle_enabled {
        let mut indices: Vec<usize> = tracks
            .iter()
            .enumerate()
            .filter(|(i, track)| *i != current_idx && track.is_playable())
            .map(|(i, _)| i)
            .collect();
        indices.shuffle(&mut rand::thread_rng());
        VecDeque::from(indices)
    } else {
        let indices: Vec<usize> = (current_idx + 1..tracks.len())
            .filter(|&i| tracks[i].is_playable())
            .collect();
        VecDeque::from(indices)
    }
}

pub fn play_queued_track(
    queued: QueuedTrack,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
    preserve_context: bool,
) {
    if !queued.track.is_playable() {
        return;
    }

    player.play(queued.track.clone());
    state.override_playing = Some(queued.clone());
    if preserve_context {
        return;
    }
    state.playback_source = queued.source;
    state.current_playing_index = Some(queued.index);

    match queued.source {
        PlaybackSource::Likes => {
            data.playback_playlist_uri = None;
            data.playback_album_uri = None;
            data.playback_following_user_urn = None;
            state.auto_queue = build_queue(queued.index, &data.likes, state.shuffle_enabled);
            if state.selected_tab == 0 && state.selected_subtab == 0 {
                state.selected_row = queued.index;
                data.likes_state.select(Some(queued.index));
            }
        }
        PlaybackSource::Playlist
        | PlaybackSource::Album
        | PlaybackSource::FollowingPublished
        | PlaybackSource::FollowingLikes => {
            let tracks = queued.tracks_snapshot.unwrap_or_else(|| match queued.source {
                PlaybackSource::Playlist => data.playlist_tracks.clone(),
                PlaybackSource::Album => data.album_tracks.clone(),
                PlaybackSource::FollowingPublished => data.following_tracks.clone(),
                PlaybackSource::FollowingLikes => data.following_likes_tracks.clone(),
                PlaybackSource::Likes => Vec::new(),
            });
            if tracks.is_empty() || queued.index >= tracks.len() {
                return;
            }
            data.playback_tracks = tracks;
            data.playback_playlist_uri = queued.playlist_uri;
            data.playback_album_uri = queued.album_uri;
            data.playback_following_user_urn = queued.following_user_urn;
            state.auto_queue =
                build_queue(queued.index, &data.playback_tracks, state.shuffle_enabled);

            if queued.source == PlaybackSource::Playlist
                && state.selected_tab == 0
                && state.selected_subtab == 1
                && data.playback_playlist_uri.is_some()
                && data.playback_playlist_uri == data.playlist_tracks_uri
            {
                state.selected_playlist_track_row = queued.index;
                data.playlist_tracks_state.select(Some(queued.index));
            } else if queued.source == PlaybackSource::Album
                && state.selected_tab == 0
                && state.selected_subtab == 2
                && data.playback_album_uri.is_some()
                && data.playback_album_uri == data.album_tracks_uri
            {
                state.selected_album_track_row = queued.index;
                data.album_tracks_state.select(Some(queued.index));
            } else if queued.source == PlaybackSource::FollowingPublished
                && state.selected_tab == 0
                && state.selected_subtab == 3
                && data.playback_following_user_urn.is_some()
                && data.playback_following_user_urn == data.following_tracks_user_urn
            {
                state.selected_following_track_row = queued.index;
                state.following_tracks_focus = FollowingTracksFocus::Published;
                data.following_tracks_state.select(Some(queued.index));
            } else if queued.source == PlaybackSource::FollowingLikes
                && state.selected_tab == 0
                && state.selected_subtab == 3
                && data.playback_following_user_urn.is_some()
                && data.playback_following_user_urn == data.following_likes_user_urn
            {
                state.selected_following_like_row = queued.index;
                state.following_tracks_focus = FollowingTracksFocus::Likes;
                data.following_likes_state.select(Some(queued.index));
            }
        }
    }
}

pub fn queued_from_current(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    if let Some(override_track) = state.override_playing.as_ref() {
        return Some(override_track.clone());
    }
    let idx = state.current_playing_index?;
    match state.playback_source {
        PlaybackSource::Likes => {
            let track = data.likes.get(idx)?.clone();
            Some(QueuedTrack {
                source: PlaybackSource::Likes,
                index: idx,
                track,
                tracks_snapshot: None,
                playlist_uri: None,
                album_uri: None,
                following_user_urn: None,
                user_added: false,
            })
        }
        PlaybackSource::Playlist => {
            let track = data.playback_tracks.get(idx)?.clone();
            Some(QueuedTrack {
                source: PlaybackSource::Playlist,
                index: idx,
                track,
                tracks_snapshot: Some(data.playback_tracks.clone()),
                playlist_uri: data.playback_playlist_uri.clone(),
                album_uri: None,
                following_user_urn: None,
                user_added: false,
            })
        }
        PlaybackSource::Album => {
            let track = data.playback_tracks.get(idx)?.clone();
            Some(QueuedTrack {
                source: PlaybackSource::Album,
                index: idx,
                track,
                tracks_snapshot: Some(data.playback_tracks.clone()),
                playlist_uri: None,
                album_uri: data.playback_album_uri.clone(),
                following_user_urn: None,
                user_added: false,
            })
        }
        PlaybackSource::FollowingPublished => {
            let track = data.playback_tracks.get(idx)?.clone();
            Some(QueuedTrack {
                source: PlaybackSource::FollowingPublished,
                index: idx,
                track,
                tracks_snapshot: Some(data.playback_tracks.clone()),
                playlist_uri: None,
                album_uri: None,
                following_user_urn: data.playback_following_user_urn.clone(),
                user_added: false,
            })
        }
        PlaybackSource::FollowingLikes => {
            let track = data.playback_tracks.get(idx)?.clone();
            Some(QueuedTrack {
                source: PlaybackSource::FollowingLikes,
                index: idx,
                track,
                tracks_snapshot: Some(data.playback_tracks.clone()),
                playlist_uri: None,
                album_uri: None,
                following_user_urn: data.playback_following_user_urn.clone(),
                user_added: false,
            })
        }
    }
}
