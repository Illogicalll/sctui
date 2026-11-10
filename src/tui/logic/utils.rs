use std::collections::VecDeque;

use rand::Rng;
use rand::seq::SliceRandom;

use crate::api::{Album, Artist, Playlist, Track};
use crate::player::{Player, TrackChange};

use super::nav;
use super::state::{AppData, AppState, PlaybackSource, QueuedTrack};

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

/// Adjust a playlist's displayed track count by `delta`, never below zero.
pub(crate) fn bump_track_count(playlist: &mut Playlist, delta: i64) {
    let n = playlist.track_count.trim().parse::<i64>().unwrap_or(0);
    playlist.track_count = (n + delta).max(0).to_string();
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

/// Rebuild the automatic queue around the playing track for the current shuffle setting.
/// A station's list is the chain already heard plus what is still queued, so there the
/// queue is only ever reordered: rebuilding it from the list would replay the chain (and,
/// by refilling the queue, stop more related tracks being fetched).
pub(crate) fn rebuild_auto_queue(state: &mut AppState, data: &AppData) {
    let Some(current_idx) = state.current_playing_index else {
        return;
    };
    if state.playback_source == PlaybackSource::Radio {
        let queued = state.auto_queue.make_contiguous();
        if state.shuffle_enabled {
            queued.shuffle(&mut rand::thread_rng());
        } else {
            queued.sort_unstable();
        }
    } else {
        state.auto_queue = build_queue(current_idx, active_tracks(state, data), state.shuffle_enabled);
    }
}

/// Extend the playing list with `incoming`, queueing each new playable track after what is
/// already queued, or somewhere at random among it when `shuffle` is set. Tracks already
/// present (by urn) or unplayable are dropped.
pub(crate) fn append_feed_tracks(
    playback_tracks: &mut Vec<Track>,
    auto_queue: &mut VecDeque<usize>,
    incoming: impl IntoIterator<Item = Track>,
    shuffle: bool,
) {
    for track in incoming {
        let seen = playback_tracks.iter().any(|t| t.track_urn == track.track_urn);
        if seen || !track.is_playable() {
            continue;
        }
        let at = if shuffle { rand::thread_rng().gen_range(0..=auto_queue.len()) } else { auto_queue.len() };
        auto_queue.insert(at, playback_tracks.len());
        playback_tracks.push(track);
    }
}

/// A page of Likes arrived. Likes load in the background, so a queue started from them
/// early only covers the pages loaded by then: while Likes are what is playing, the new
/// page joins the playing list and its queue.
pub(crate) fn extend_likes_playback(state: &mut AppState, data: &mut AppData, page: &[Track]) {
    if state.playback_source == PlaybackSource::Likes && state.current_playing_index.is_some() {
        append_feed_tracks(
            &mut data.playback_tracks,
            &mut state.auto_queue,
            page.iter().cloned(),
            state.shuffle_enabled,
        );
    }
}

pub fn play_queued_track(
    queued: QueuedTrack,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
    preserve_context: bool,
    change: TrackChange,
) {
    if !queued.track.is_playable() {
        return;
    }

    player.play(queued.track.clone(), change);
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
            data.playback_tracks = data.likes.clone();
            state.auto_queue = build_queue(queued.index, &data.playback_tracks, state.shuffle_enabled);
            if state.selected_tab == 0 && state.selected_subtab == 0 {
                state.selected_row = queued.index;
                data.likes_state.select(Some(queued.index));
            }
        }
        PlaybackSource::Playlist
        | PlaybackSource::Album
        | PlaybackSource::FollowingPublished
        | PlaybackSource::FollowingLikes
        | PlaybackSource::Feed
        | PlaybackSource::Radio => {
            let tracks = queued.tracks_snapshot.unwrap_or_else(|| match queued.source {
                PlaybackSource::Playlist => data.playlist_tracks.clone(),
                PlaybackSource::Album => data.album_tracks.clone(),
                PlaybackSource::FollowingPublished => data.following_tracks.clone(),
                PlaybackSource::FollowingLikes => data.following_likes_tracks.clone(),
                PlaybackSource::Feed => data.feed_tracks.clone(),
                PlaybackSource::Radio => data.playback_tracks.clone(),
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
                nav::set_focus(state, 1);
                data.following_tracks_state.select(Some(queued.index));
            } else if queued.source == PlaybackSource::FollowingLikes
                && state.selected_tab == 0
                && state.selected_subtab == 3
                && data.playback_following_user_urn.is_some()
                && data.playback_following_user_urn == data.following_likes_user_urn
            {
                state.selected_following_like_row = queued.index;
                nav::set_focus(state, 2);
                data.following_likes_state.select(Some(queued.index));
            }
        }
    }
}

/// The list `current_playing_index` and `auto_queue` index: always the copy taken when
/// playback started, never a library list, whose rows move as tracks are liked, unliked,
/// paged in or hidden.
pub(crate) fn active_tracks<'a>(_state: &AppState, data: &'a AppData) -> &'a [Track] {
    &data.playback_tracks
}

/// Hand playback over to related tracks, seeded by `seeds` (index 0 is playing now). A single
/// seed leaves nothing queued, so the main loop's near-empty check fires right away and fetches
/// related tracks for it; a pre-built mix (an artist station) plays through those first.
pub(crate) fn enter_radio(state: &mut AppState, data: &mut AppData, seeds: Vec<Track>) {
    state.playback_source = PlaybackSource::Radio;
    state.override_playing = None;
    state.current_playing_index = Some(0);
    state.radio_fetched_for = None;
    state.radio_waiting = false;
    state.auto_queue = build_queue(0, &seeds, state.shuffle_enabled);
    data.playback_tracks = seeds;
    data.playback_playlist_uri = None;
    data.playback_album_uri = None;
    data.playback_following_user_urn = None;
}

/// Start a station the user asked for, `seeds[0]` playing now. What was playing goes into
/// the history; the hand-built queue is replaced by the station.
pub(crate) fn start_station(state: &mut AppState, data: &mut AppData, seeds: Vec<Track>) {
    if let Some(current) = queued_from_current(state, data) {
        state.push_history(current);
    }
    state.cancel_artist_station();
    state.manual_queue.clear();
    enter_radio(state, data, seeds);
}

pub fn queued_from_current(state: &AppState, data: &AppData) -> Option<QueuedTrack> {
    if let Some(override_track) = state.override_playing.as_ref() {
        return Some(override_track.clone());
    }
    let idx = state.current_playing_index?;
    let source = state.playback_source;
    let track = active_tracks(state, data).get(idx)?;
    let (tracks_snapshot, playlist_uri, album_uri, following_user_urn) = match source {
        PlaybackSource::Playlist => (
            Some(&data.playback_tracks),
            data.playback_playlist_uri.clone(),
            None,
            None,
        ),
        PlaybackSource::Album => (
            Some(&data.playback_tracks),
            None,
            data.playback_album_uri.clone(),
            None,
        ),
        PlaybackSource::FollowingPublished | PlaybackSource::FollowingLikes => (
            Some(&data.playback_tracks),
            None,
            None,
            data.playback_following_user_urn.clone(),
        ),
        PlaybackSource::Likes | PlaybackSource::Feed | PlaybackSource::Radio => (Some(&data.playback_tracks), None, None, None),
    };
    Some(QueuedTrack::new(
        source, idx, track, tracks_snapshot, playlist_uri, album_uri, following_user_urn, false,
    ))
}

/// Move the queue on from the track that is playing and return the one to play instead:
/// the first track queued by hand (it plays outside the list, as `override_playing`),
/// else the next of the list. `None`, with nothing changed, when nothing is queued.
pub(crate) fn advance(state: &mut AppState, data: &AppData) -> Option<Track> {
    let current = queued_from_current(state, data);
    let track = if let Some(queued) = state.manual_queue.pop_front() {
        let track = queued.track.clone();
        state.override_playing = Some(queued);
        track
    } else {
        let next_idx = state.auto_queue.pop_front()?;
        let track = active_tracks(state, data).get(next_idx)?.clone();
        state.override_playing = None;
        state.current_playing_index = Some(next_idx);
        track
    };
    if let Some(current) = current {
        state.push_history(current);
    }
    Some(track)
}

/// The track that is playing: not the list's current row while a queued, previous or
/// history track plays over it.
fn playing_track<'a>(state: &'a AppState, data: &'a AppData) -> Option<&'a Track> {
    match &state.override_playing {
        Some(queued) => Some(&queued.track),
        None => active_tracks(state, data).get(state.current_playing_index?),
    }
}

/// The playing track ran out: what plays now. With repeat on that is the same track again.
pub(crate) fn end_of_track(state: &mut AppState, data: &AppData) -> Option<Track> {
    if state.repeat_enabled {
        return playing_track(state, data).cloned();
    }
    advance(state, data)
}

/// What `end_of_track` would play, without moving anything; for preloading.
pub(crate) fn peek_next(state: &AppState, data: &AppData) -> Option<Track> {
    if state.repeat_enabled {
        playing_track(state, data).cloned()
    } else if let Some(queued) = state.manual_queue.front() {
        Some(queued.track.clone())
    } else {
        state.auto_queue.front().and_then(|&i| active_tracks(state, data).get(i).cloned())
    }
}

/// When to start the next track. Normally that is the moment this one runs out;
/// with a crossfade the next one has to start `crossfade_ms` early so the two
/// overlap. Never earlier than half way through, so a short track (or a long
/// crossfade) cannot start its successor almost as soon as it begins.
pub(crate) fn next_track_trigger_ms(duration_ms: u64, crossfade_ms: u64) -> u64 {
    const END_OF_TRACK_SLACK_MS: u64 = 50;
    let lead = crossfade_ms.max(END_OF_TRACK_SLACK_MS).min(duration_ms / 2);
    duration_ms.saturating_sub(lead)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_track_starts_early_enough_to_crossfade_but_never_half_way_in() {
        // No crossfade: unchanged, the old 50 ms of end-of-track slack.
        assert_eq!(next_track_trigger_ms(200_000, 0), 199_950);
        // 8 s crossfade over a 3 minute track.
        assert_eq!(next_track_trigger_ms(180_000, 8_000), 172_000);
        // A 10 s clip must not hand over 12 s before it ends.
        assert_eq!(next_track_trigger_ms(10_000, 12_000), 5_000);
        assert_eq!(next_track_trigger_ms(0, 12_000), 0);
    }

    fn track(urn: &str, access: &str) -> Track {
        Track {
            title: urn.to_string(),
            artists: String::new(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: access.to_string(),
            track_urn: urn.to_string(),
        }
    }

    #[test]
    fn append_feed_tracks_queues_new_playable_tracks_after_existing_queue() {
        let mut playing = vec![track("a", "playable"), track("b", "playable")];
        let mut queue = VecDeque::from([1]);
        append_feed_tracks(
            &mut playing,
            &mut queue,
            vec![
                track("a", "playable"), // duplicate of what is playing: dropped
                track("c", "blocked"),  // unplayable: dropped
                track("d", "playable"),
                track("e", ""),
                track("d", "playable"), // duplicate within the batch: dropped
            ],
            false,
        );
        let urns: Vec<&str> = playing.iter().map(|t| t.track_urn.as_str()).collect();
        assert_eq!(urns, ["a", "b", "d", "e"]);
        assert_eq!(queue, VecDeque::from([1, 2, 3]));
    }

    fn tracks(urns: &[&str]) -> Vec<Track> {
        urns.iter().map(|urn| track(urn, "playable")).collect()
    }

    /// Likes playing from `playing`, at index `at`, with the rest queued in order.
    fn likes_playing(playing: &[&str], at: usize) -> (AppState, AppData) {
        let data = AppData { likes: tracks(playing), playback_tracks: tracks(playing), ..Default::default() };
        let state = AppState {
            current_playing_index: Some(at),
            auto_queue: build_queue(at, &data.playback_tracks, false),
            ..Default::default()
        };
        (state, data)
    }

    fn queued_urns(state: &AppState, data: &AppData) -> Vec<String> {
        let list = active_tracks(state, data);
        state.auto_queue.iter().map(|&i| list[i].track_urn.clone()).collect()
    }

    /// T2: the Likes queue indexes its own copy, so the library list changing under it
    /// (a like inserted at the top, an unlike, the hide-unplayable filter) moves nothing.
    #[test]
    fn likes_playback_is_not_disturbed_by_the_library_list_changing() {
        use crate::tui::logic::state::Engagement;
        let (mut state, mut data) = likes_playing(&["a", "b", "c", "d"], 1);
        let playing = |state: &AppState, data: &AppData| queued_from_current(state, data).unwrap().track.track_urn;

        // Like a track from somewhere else: it lands at the top of the library.
        data.liked_track_urns.insert("new".into());
        Engagement::LikeTrack { track: track("new", "playable"), track_id: 1 }.apply(&mut state, &mut data);
        assert_eq!(data.likes[0].track_urn, "new");
        assert_eq!(playing(&state, &data), "b");
        assert_eq!(queued_urns(&state, &data), ["c", "d"]);

        // Unlike the playing track: the next one is still next.
        Engagement::UnlikeTrack { track_urn: "b".into(), track_id: 2 }.apply(&mut state, &mut data);
        assert!(!data.likes.iter().any(|t| t.track_urn == "b"));
        assert_eq!(playing(&state, &data), "b");
        assert_eq!(queued_urns(&state, &data), ["c", "d"]);

        // Rows hidden from the library list stay where they were in the queue.
        data.likes[0].access = "blocked".into();
        state.settings.hide_unplayable = true;
        crate::tui::logic::filtering::apply_unplayable_filter(&mut state, &mut data);
        assert_eq!(playing(&state, &data), "b");
        assert_eq!(queued_urns(&state, &data), ["c", "d"]);
    }

    /// T8: repeat replays the track that is playing, not the list row under it.
    #[test]
    fn repeat_replays_a_queued_track_rather_than_the_list_row_under_it() {
        let (mut state, data) = likes_playing(&["a", "b", "c"], 0);
        let extra = track("q", "playable");
        state.manual_queue.push_back(QueuedTrack::new(PlaybackSource::Playlist, 5, &extra, None, None, None, None, true));

        // "a" ends: the queued track plays over the list, which stays on "a".
        assert_eq!(peek_next(&state, &data).unwrap().track_urn, "q");
        assert_eq!(end_of_track(&mut state, &data).unwrap().track_urn, "q");
        assert_eq!(state.current_playing_index, Some(0));

        // Repeat on while it plays: it is preloaded and replayed, again and again.
        state.repeat_enabled = true;
        for _ in 0..2 {
            assert_eq!(peek_next(&state, &data).unwrap().track_urn, "q");
            assert_eq!(end_of_track(&mut state, &data).unwrap().track_urn, "q");
        }
        assert_eq!(state.playback_history.len(), 1, "a repeat is not a new history entry");
        assert_eq!(queued_urns(&state, &data), ["b", "c"], "the list queue is untouched");

        // Repeat off: the list carries on from where it was.
        state.repeat_enabled = false;
        assert_eq!(end_of_track(&mut state, &data).unwrap().track_urn, "b");
        assert!(state.override_playing.is_none());
        assert_eq!(state.playback_history.iter().map(|q| q.track.track_urn.as_str()).collect::<Vec<_>>(), ["a", "q"]);
        // Nothing left to play: nothing changes.
        state.auto_queue.clear();
        assert!(end_of_track(&mut state, &data).is_none() && peek_next(&state, &data).is_none());
        assert_eq!(state.playback_history.len(), 2);
    }

    /// T17: in a station nothing already heard is ever queued again.
    #[test]
    fn a_station_queue_is_reordered_but_never_rebuilt_from_the_heard_chain() {
        // "a".."c" heard, "d" playing, "e" and "f" still to come.
        let (mut state, data) = likes_playing(&["a", "b", "c", "d", "e", "f"], 3);
        state.playback_source = PlaybackSource::Radio;
        state.shuffle_enabled = true;
        rebuild_auto_queue(&mut state, &data);
        let mut queued = queued_urns(&state, &data);
        queued.sort();
        assert_eq!(queued, ["e", "f"]);
        state.shuffle_enabled = false;
        state.auto_queue = VecDeque::from([5, 4]);
        rebuild_auto_queue(&mut state, &data);
        assert_eq!(queued_urns(&state, &data), ["e", "f"], "shuffle off puts what is left back in order");

        // Queue empty while related tracks load: Next has nothing to play and queues nothing.
        state.shuffle_enabled = true;
        state.auto_queue.clear();
        rebuild_auto_queue(&mut state, &data);
        assert!(state.auto_queue.is_empty() && advance(&mut state, &data).is_none());

        // Any other source still reshuffles its whole list.
        state.playback_source = PlaybackSource::Playlist;
        rebuild_auto_queue(&mut state, &data);
        assert_eq!(state.auto_queue.len(), 5);
    }

    /// T10: a track station started while an artist station is still loading wins.
    #[test]
    fn starting_a_track_station_drops_a_pending_artist_station() {
        use crate::api::Artist;
        let (mut state, mut data) = likes_playing(&["a", "b"], 0);
        state.artist_station_request = Some(Artist { name: "Someone".into(), urn: "soundcloud:users:7".into() });
        let in_flight = state.artist_station_fetch.request_id;
        start_station(&mut state, &mut data, tracks(&["s"]));
        assert!(state.artist_station_request.is_none());
        assert_ne!(state.artist_station_fetch.request_id, in_flight, "the late mix is no longer current");
        // The station replaces the queue but not the history.
        assert_eq!(state.playback_history.len(), 1);
        assert_eq!(queued_from_current(&state, &data).unwrap().track.track_urn, "s");
    }

    /// T14: Likes pages that load after playback started join the queue.
    #[test]
    fn likes_pages_arriving_during_likes_playback_extend_the_queue() {
        let (mut state, mut data) = likes_playing(&["a", "b"], 0);
        let page = [track("b", "playable"), track("c", "playable"), track("x", "blocked"), track("d", "playable")];
        extend_likes_playback(&mut state, &mut data, &page);
        assert_eq!(queued_urns(&state, &data), ["b", "c", "d"], "new playable tracks, once each, in order");
        assert_eq!(queued_from_current(&state, &data).unwrap().track.track_urn, "a");

        // Shuffle on: the new tracks are mixed in, each still queued exactly once.
        state.shuffle_enabled = true;
        extend_likes_playback(&mut state, &mut data, &tracks(&["e", "f", "g"]));
        let mut queued = queued_urns(&state, &data);
        queued.sort();
        assert_eq!(queued, ["b", "c", "d", "e", "f", "g"]);

        // Anything else playing, or nothing at all, is left alone.
        let before = data.playback_tracks.len();
        state.playback_source = PlaybackSource::Playlist;
        extend_likes_playback(&mut state, &mut data, &tracks(&["h"]));
        state.playback_source = PlaybackSource::Likes;
        state.current_playing_index = None;
        extend_likes_playback(&mut state, &mut data, &tracks(&["h"]));
        assert_eq!(data.playback_tracks.len(), before);
    }
}
