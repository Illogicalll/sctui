use crate::api::Track;
use crate::tui::logic::state::{AppState, FollowingTracksFocus, PlaybackSource, QueuedTrack};

impl QueuedTrack {
    pub(crate) fn new(
        source: PlaybackSource,
        index: usize,
        track: &Track,
        tracks_snapshot: Option<&Vec<Track>>,
        playlist_uri: Option<String>,
        album_uri: Option<String>,
        following_user_urn: Option<String>,
        user_added: bool,
    ) -> Self {
        Self {
            source,
            index,
            track: track.clone(),
            tracks_snapshot: tracks_snapshot.cloned(),
            playlist_uri,
            album_uri,
            following_user_urn,
            user_added,
        }
    }
}

pub(crate) fn insert_manual_queue(state: &mut AppState, queued: QueuedTrack) {
    let mut items: Vec<QueuedTrack> = state.manual_queue.drain(..).collect();
    let insert_idx = match items.iter().rposition(|item| item.user_added) {
        Some(idx) => idx + 1,
        None => 0,
    };
    items.insert(insert_idx, queued);
    state.manual_queue = items.into_iter().collect();
}

/// Maps a visible Library row to its index in the unfiltered list while the search filter is active.
pub(crate) fn filtered_row(state: &AppState, row: usize) -> Option<usize> {
    if state.search_popup_visible && !state.search_query.trim().is_empty() {
        state.search_matches.get(row).copied()
    } else {
        Some(row)
    }
}

pub(crate) fn reset_search_rows(state: &mut AppState) {
    state.selected_row = 0;
    state.search_selected_playlist_track_row = 0;
    state.search_selected_album_track_row = 0;
    state.search_selected_person_track_row = 0;
    state.search_selected_person_like_row = 0;
    state.search_people_tracks_focus = FollowingTracksFocus::Published;
}
