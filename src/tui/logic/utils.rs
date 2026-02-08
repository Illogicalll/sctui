use std::collections::VecDeque;

use rand::seq::SliceRandom;

use crate::api::{Album, Artist, Playlist, Track};
use crate::player::Player;

use super::state::{AppData, AppState, FollowingTracksFocus, PlaybackSource, QueuedTrack};

fn fuzzy_score_subsequence(query: &str, candidate: &str) -> Option<i64> {
    if query.is_empty() || candidate.is_empty() {
        return None;
    }

    let q_chars: Vec<char> = query.chars().collect();
    let c_chars: Vec<char> = candidate.chars().collect();

    if q_chars.len() > c_chars.len() {
        return None;
    }

    let mut positions: Vec<usize> = Vec::with_capacity(q_chars.len());
    let mut qi = 0usize;
    for (ci, &cc) in c_chars.iter().enumerate() {
        if cc == q_chars[qi] {
            positions.push(ci);
            qi += 1;
            if qi == q_chars.len() {
                break;
            }
        }
    }

    if qi != q_chars.len() {
        return None;
    }

    let mut score: i64 = 0;
    let mut prev_pos: Option<usize> = None;
    for &pos in &positions {
        score += 10;

        if pos == 0 {
            score += 15;
        } else if !c_chars[pos - 1].is_alphanumeric() {
            score += 10;
        }

        if let Some(prev) = prev_pos {
            if pos == prev + 1 {
                score += 8;
            } else {
                score -= (pos - prev - 1) as i64;
            }
        }
        prev_pos = Some(pos);
    }

    score -= positions[0] as i64;

    if candidate.starts_with(query) {
        score += 50;
    } else if candidate.contains(query) {
        score += 30;
    }

    score -= (c_chars.len() as i64) / 10;

    Some(score)
}

fn fuzzy_score_track_tokens(tokens: &[&str], title: &str, artists: &str) -> Option<i64> {
    let title_lc = title.to_lowercase();
    let artists_lc = artists.to_lowercase();

    let mut total: i64 = 0;
    for token in tokens {
        let title_score = fuzzy_score_subsequence(token, &title_lc).map(|s| s + 25);
        let artist_score = fuzzy_score_subsequence(token, &artists_lc);

        let best = match (title_score, artist_score) {
            (Some(a), Some(b)) => a.max(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => return None,
        };
        total += best;
    }

    Some(total)
}

fn fuzzy_score_single_field_tokens(tokens: &[&str], field: &str) -> Option<i64> {
    let field_lc = field.to_lowercase();
    let mut total: i64 = 0;
    for token in tokens {
        total += fuzzy_score_subsequence(token, &field_lc)?;
    }
    Some(total)
}

pub fn build_search_matches(
    selected_subtab: usize,
    query: &str,
    likes: &Vec<Track>,
    playlists: &Vec<Playlist>,
    playlist_tracks: &Vec<Track>,
    albums: &Vec<Album>,
    following: &Vec<Artist>,
) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    let tokens: Vec<&str> = q.split_whitespace().filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return Vec::new();
    }

    match selected_subtab {
        0 => {
            let mut scored: Vec<(i64, usize)> = likes
                .iter()
                .enumerate()
                .filter_map(|(i, track)| {
                    fuzzy_score_track_tokens(&tokens, &track.title, &track.artists)
                        .map(|s| (s, i))
                })
                .collect();
            scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            scored.into_iter().map(|(_, i)| i).collect()
        }
        1 => {
            let mut scored: Vec<(i64, usize)> = playlist_tracks
                .iter()
                .enumerate()
                .filter_map(|(i, track)| {
                    fuzzy_score_track_tokens(&tokens, &track.title, &track.artists)
                        .map(|s| (s, i))
                })
                .collect();
            scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            scored.into_iter().map(|(_, i)| i).collect()
        }
        2 => {
            let mut scored: Vec<(i64, usize)> = albums
                .iter()
                .enumerate()
                .filter_map(|(i, album)| {
                    fuzzy_score_track_tokens(&tokens, &album.title, &album.artists)
                        .map(|s| (s, i))
                })
                .collect();
            scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            scored.into_iter().map(|(_, i)| i).collect()
        }
        3 => {
            let mut scored: Vec<(i64, usize)> = following
                .iter()
                .enumerate()
                .filter_map(|(i, artist)| {
                    fuzzy_score_single_field_tokens(&tokens, &artist.name)
                        .map(|s| (s, i))
                })
                .collect();
            scored.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
            scored.into_iter().map(|(_, i)| i).collect()
        }
        _ => Vec::new(),
    }
}

pub fn soundcloud_id_from_urn(urn: &str) -> Option<u64> {
    let last = urn.rsplit(':').next()?;
    last.parse::<u64>().ok()
}

pub fn soundcloud_playlist_id_from_tracks_uri(tracks_uri: &str) -> Option<u64> {
    let s = tracks_uri.trim();
    if s.is_empty() {
        return None;
    }

    let s = s
        .strip_prefix("https://api.soundcloud.com")
        .or_else(|| s.strip_prefix("http://api.soundcloud.com"))
        .unwrap_or(s);

    let s = s.split('?').next().unwrap_or(s);

    for marker in ["playlists/", "sets/"] {
        if let Some(idx) = s.find(marker) {
            let after = &s[idx + marker.len()..];
            let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return digits.parse::<u64>().ok();
            }
        }
    }

    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "playlists" || *part == "sets" {
            let raw = parts.get(i + 1)?;
            let digits: String = raw.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                return digits.parse::<u64>().ok();
            }
        }
    }

    let mut best: String = String::new();
    let mut current: String = String::new();
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else {
            if current.len() > best.len() {
                best = current.clone();
            }
            current.clear();
        }
    }
    if current.len() > best.len() {
        best = current;
    }
    if best.is_empty() {
        return None;
    }
    best.parse::<u64>().ok()
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
