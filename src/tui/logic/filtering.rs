use crate::api::{Album, Artist, Playlist, Track};

use super::state::{AppData, AppState};

pub struct FilteredViews {
    pub likes: Vec<Track>,
    pub playlists: Vec<Playlist>,
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
    let query = state.search_query.trim().to_lowercase();

    let likes = if filter_active && state.selected_subtab == 0 {
        data.likes
            .iter()
            .filter(|track| {
                !query.is_empty()
                    && (track.title.to_lowercase().contains(&query)
                        || track.artists.to_lowercase().contains(&query))
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let playlists = if filter_active && state.selected_subtab == 1 {
        data.playlists
            .iter()
            .filter(|playlist| {
                !query.is_empty() && playlist.title.to_lowercase().contains(&query)
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let albums = if filter_active && state.selected_subtab == 2 {
        data.albums
            .iter()
            .filter(|album| {
                !query.is_empty()
                    && (album.title.to_lowercase().contains(&query)
                        || album.artists.to_lowercase().contains(&query))
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let following = if filter_active && state.selected_subtab == 3 {
        data.following
            .iter()
            .filter(|artist| {
                !query.is_empty() && artist.name.to_lowercase().contains(&query)
            })
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    FilteredViews {
        likes,
        playlists,
        albums,
        following,
    }
}

pub fn clamp_selection(
    state: &mut AppState,
    data: &mut AppData,
    filter_active: bool,
    likes_len: usize,
    playlists_len: usize,
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
            if state.selected_row >= playlists_len && playlists_len > 0 {
                state.selected_row = playlists_len - 1;
            }
            data.playlists_state.select(Some(state.selected_row));
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
