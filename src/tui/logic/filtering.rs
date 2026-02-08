use crate::api::{Album, Artist, Playlist, Track};

use super::state::{AppData, AppState};

pub struct FilteredViews {
    pub likes: Vec<Track>,
    pub playlists: Vec<Playlist>,
    pub playlist_tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub following: Vec<Artist>,
}

pub fn is_filter_active(state: &AppState) -> bool {
    state.search_popup_visible
        && state.selected_tab == 0
        && !state.search_query.trim().is_empty()
}

pub fn build_filtered_views(state: &AppState, data: &AppData) -> FilteredViews {
    let filter_active = is_filter_active(state);

    let likes = if filter_active && state.selected_subtab == 0 {
        state
            .search_matches
            .iter()
            .filter_map(|&i| data.likes.get(i).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let playlists = Vec::new();

    let playlist_tracks = if filter_active && state.selected_subtab == 1 {
        state
            .search_matches
            .iter()
            .filter_map(|&i| data.playlist_tracks.get(i).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let albums = if filter_active && state.selected_subtab == 2 {
        state
            .search_matches
            .iter()
            .filter_map(|&i| data.albums.get(i).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let following = if filter_active && state.selected_subtab == 3 {
        state
            .search_matches
            .iter()
            .filter_map(|&i| data.following.get(i).cloned())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    FilteredViews {
        likes,
        playlists,
        playlist_tracks,
        albums,
        following,
    }
}

pub fn clamp_selection(
    state: &mut AppState,
    data: &mut AppData,
    filter_active: bool,
    likes_len: usize,
    playlist_tracks_len: usize,
    albums_len: usize,
    following_len: usize,
) {
    if !filter_active {
        return;
    }

    match state.selected_subtab {
        0 => {
            if state.selected_row >= likes_len && likes_len > 0 {
                state.selected_row = likes_len - 1;
            }
            data.likes_state.select(Some(state.selected_row));
        }
        1 => {
            if state.selected_playlist_track_row >= playlist_tracks_len && playlist_tracks_len > 0 {
                state.selected_playlist_track_row = playlist_tracks_len - 1;
            }
            data.playlist_tracks_state
                .select(Some(state.selected_playlist_track_row));
        }
        2 => {
            if state.selected_row >= albums_len && albums_len > 0 {
                state.selected_row = albums_len - 1;
            }
            data.albums_state.select(Some(state.selected_row));
        }
        3 => {
            if state.selected_row >= following_len && following_len > 0 {
                state.selected_row = following_len - 1;
            }
            data.following_state.select(Some(state.selected_row));
        }
        _ => {}
    }
}
