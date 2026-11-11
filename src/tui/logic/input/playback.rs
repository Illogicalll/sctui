use super::InputOutcome;
use crate::api::{Artist, Track};
use crate::tui::logic::nav::{self, PaneId};
use crate::tui::logic::state::{AppData, AppState, PlaybackSource, FollowingTracksFocus};
use crate::player::{Player, TrackChange};
use crate::tui::logic::utils::{build_queue, queued_from_current, start_station};

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
    player.play(queued.track.clone(), TrackChange::UserSkip);
    // Choosing something to play means play it, paused or not.
    player.resume();
    start_station(state, data, vec![queued.track]);
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
#[allow(clippy::too_many_arguments)]
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
    let Some(track) = begin_playback(state, data, source, tracks, idx, playlist_uri, album_uri, following_user_urn) else {
        return false;
    };
    player.play(track, TrackChange::UserSkip);
    // Choosing something to play means play it, paused or not. Next/previous keep the pause.
    player.resume();
    true
}

/// Everything `start_playback` does short of touching the player: the queue state for
/// playing `tracks(data)[idx]`, and that track to play.
#[allow(clippy::too_many_arguments)]
fn begin_playback(
    state: &mut AppState,
    data: &mut AppData,
    source: PlaybackSource,
    tracks: fn(&AppData) -> &Vec<Track>,
    idx: usize,
    playlist_uri: Option<String>,
    album_uri: Option<String>,
    following_user_urn: Option<String>,
) -> Option<Track> {
    let track = tracks(data).get(idx).filter(|t| t.is_playable())?.clone();

    // Tracks queued by hand and the listening history outlive a change of list. What
    // was playing joins the history unless it is the very track being started again.
    if let Some(queued) = queued_from_current(state, data)
        && queued.track.track_urn != track.track_urn {
            state.push_history(queued);
        }

    state.cancel_artist_station();
    state.playback_source = source;
    state.override_playing = None;
    state.current_playing_index = Some(idx);
    data.playback_tracks = tracks(data).clone();
    data.playback_playlist_uri = playlist_uri;
    data.playback_album_uri = album_uri;
    data.playback_following_user_urn = following_user_urn;
    state.auto_queue = build_queue(idx, &data.playback_tracks, state.shuffle_enabled);
    Some(track)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::logic::state::QueuedTrack;
    use crate::tui::logic::utils::active_tracks;

    fn tracks(prefix: &str, n: usize) -> Vec<Track> {
        (0..n)
            .map(|i| Track {
                title: format!("{prefix}{i}"),
                artists: String::new(),
                duration: String::new(),
                duration_ms: 0,
                playback_count: String::new(),
                artwork_url: String::new(),
                access: String::new(),
                track_urn: format!("{prefix}{i}"),
            })
            .collect()
    }

    fn library() -> AppData {
        AppData {
            likes: tracks("like", 4),
            playlist_tracks: tracks("listed", 4),
            search_tracks: tracks("found", 4),
            ..Default::default()
        }
    }

    fn enter(state: &mut AppState, data: &mut AppData, source: PlaybackSource, tracks: fn(&AppData) -> &Vec<Track>, idx: usize) -> Option<String> {
        begin_playback(state, data, source, tracks, idx, None, None, None).map(|t| t.track_urn)
    }

    fn history(state: &AppState) -> Vec<&str> {
        state.playback_history.iter().map(|q| q.track.track_urn.as_str()).collect()
    }

    /// T2: Enter on a like plays from a copy of the list, as every other source does.
    #[test]
    fn enter_on_a_like_queues_from_a_copy_of_the_likes() {
        let (mut state, mut data) = (AppState::default(), library());
        assert_eq!(enter(&mut state, &mut data, PlaybackSource::Likes, |d| &d.likes, 1).as_deref(), Some("like1"));
        data.likes.insert(0, tracks("new", 1).remove(0));
        let list = active_tracks(&state, &data);
        assert_eq!(list[state.current_playing_index.unwrap()].track_urn, "like1");
        assert_eq!(state.auto_queue.iter().map(|&i| list[i].track_urn.as_str()).collect::<Vec<_>>(), ["like2", "like3"]);
    }

    /// T10: an artist station still being fetched must not replace what the user then
    /// chose to play.
    #[test]
    fn enter_drops_a_pending_artist_station() {
        let (mut state, mut data) = (AppState::default(), library());
        // Requested and not yet picked up by the loop, or already being fetched.
        state.artist_station_request = Some(Artist { name: "Someone".into(), urn: "soundcloud:users:7".into() });
        let in_flight = state.artist_station_fetch.request_id;
        enter(&mut state, &mut data, PlaybackSource::Likes, |d| &d.likes, 0);
        assert!(state.artist_station_request.is_none());
        assert_ne!(state.artist_station_fetch.request_id, in_flight, "the late mix is no longer current");
    }

    /// T13: choosing something else to play keeps the hand-built queue and the history,
    /// whatever kind of list it comes from.
    #[test]
    fn enter_keeps_the_manual_queue_and_history_across_sources() {
        let (mut state, mut data) = (AppState::default(), library());
        enter(&mut state, &mut data, PlaybackSource::Playlist, |d| &d.playlist_tracks, 0);
        let queued = data.playlist_tracks[3].clone();
        state.manual_queue.push_back(QueuedTrack::new(PlaybackSource::Playlist, 3, &queued, None, None, None, None, true));

        // A different kind of source.
        enter(&mut state, &mut data, PlaybackSource::Likes, |d| &d.likes, 2);
        assert_eq!(state.manual_queue.len(), 1, "the queued track survives");
        assert_eq!(history(&state), ["listed0"]);

        // Same source kind and same row, but another list (search results also play as
        // `Playlist`): the outgoing track still goes into the history.
        enter(&mut state, &mut data, PlaybackSource::Playlist, |d| &d.playlist_tracks, 2);
        enter(&mut state, &mut data, PlaybackSource::Playlist, |d| &d.search_tracks, 2);
        assert_eq!(history(&state), ["listed0", "like2", "listed2"]);

        // Starting the playing track again is not a new history entry.
        enter(&mut state, &mut data, PlaybackSource::Playlist, |d| &d.search_tracks, 2);
        assert_eq!(history(&state).len(), 3);
        assert_eq!(state.manual_queue.len(), 1);

        // A row that cannot be played changes nothing.
        data.likes[0].access = "blocked".into();
        assert_eq!(enter(&mut state, &mut data, PlaybackSource::Likes, |d| &d.likes, 0), None);
        assert_eq!((history(&state).len(), state.playback_source == PlaybackSource::Playlist), (3, true));
    }
}
