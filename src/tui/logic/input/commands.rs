use crate::keymap::Action;

use super::InputOutcome;
use crate::api::{Album, Artist, Playlist, Track};
use crate::tui::logic::filtering::build_filtered_views;
use crate::tui::logic::nav::{self, PaneId};
use crate::tui::logic::state::{AppData, AppState, ConfirmAction, Engagement, HelpPage, PlaybackSource};
use crate::tui::logic::utils::{active_tracks, build_queue};
use crate::tui::logic::utils::queued_from_current;
use crate::tui::logic::utils::{soundcloud_id_from_urn, soundcloud_playlist_id_from_tracks_uri};

use super::queue::{handle_add_to_queue, handle_add_next_to_queue, selected_or_toast};

/// Commands that are plain state changes; movement, playback and tab actions
/// are dispatched in `input::run_action`.
pub(crate) fn run_command(action: Action, state: &mut AppState, data: &mut AppData) -> InputOutcome {
    match action {
        Action::ToggleShuffle => {
            state.shuffle_enabled = !state.shuffle_enabled;
            if let Some(current_idx) = state.current_playing_index {
                state.auto_queue =
                    build_queue(current_idx, active_tracks(state, data), state.shuffle_enabled);
            }
            state.toast(format!("Shuffle {}", if state.shuffle_enabled { "on" } else { "off" }));
        }
        Action::ToggleRepeat => {
            state.repeat_enabled = !state.repeat_enabled;
            state.toast(format!("Repeat {}", if state.repeat_enabled { "on" } else { "off" }));
        }
        Action::AddToQueue => {
            handle_add_to_queue(state, data);
        }
        Action::PlayNext => {
            handle_add_next_to_queue(state, data);
        }
        Action::ToggleLike => {
            toggle_like_selected(state, data);
        }
        Action::ToggleLikePlaying => {
            let playing = queued_from_current(state, data).map(|q| q.track);
            toggle_track_like(playing, state, data);
        }
        Action::Search => {
            // Insert mode: the query on Search, the filter of the location's filterable
            // pane (which takes focus) in Library. An existing filter is edited, not cleared.
            if state.selected_tab == 1 {
                state.search_typing = true;
            } else if let Some(pane) = nav::filter_pane(state) {
                if let Some(at) = nav::panes(state).iter().position(|p| *p == pane) {
                    nav::set_focus(state, at);
                }
                state.search_popup_visible = true;
                state.search_typing = true;
            }
        }
        Action::Help => {
            state.help_visible = !state.help_visible;
            state.help_selected = 0;
            state.help_page = HelpPage::default();
            state.help_settings_selected = 0;
            state.help_eq_selected = 0;
            state.help_capture = None;
            state.help_message = None;
        }
        Action::ToggleVisualizer => {
            state.visualizer_mode = !state.visualizer_mode;
        }
        // Playlist management. Both destructive actions go through the confirm popup.
        Action::AddToPlaylist => {
            if let Some(queued) = selected_or_toast(state, data) {
                super::playlist_picker::open_picker(state, queued.track);
            }
        }
        Action::AddToPlaylistPlaying => {
            if let Some(queued) = queued_from_current(state, data) {
                super::playlist_picker::open_picker(state, queued.track);
            }
        }
        Action::NewPlaylist => {
            super::playlist_picker::open_new_playlist_prompt(state);
        }
        Action::RemoveFromPlaylist => {
            let open_playlist = data
                .playlists
                .iter()
                .find(|p| Some(p.tracks_uri.as_str()) == data.playlist_tracks_uri.as_deref());
            if nav::focused(state) == PaneId::PlaylistTracks
                && let Some(playlist) = open_playlist.filter(|p| p.is_owned)
                && let Some(track) = nav::selected_index(state, PaneId::PlaylistTracks)
                    .and_then(|idx| data.playlist_tracks.get(idx).cloned())
            {
                let tracks_uri = playlist.tracks_uri.clone();
                state.confirm = Some(ConfirmAction::RemoveTrack { tracks_uri, track });
                state.confirm_selected = 1;
            }
        }
        Action::DeletePlaylist => {
            if nav::focused(state) == PaneId::Playlists
                && let Some(playlist) =
                    data.playlists.get(nav::cursor(state, PaneId::Playlists)).filter(|p| p.is_owned)
            {
                state.confirm = Some(ConfirmAction::DeletePlaylist {
                    tracks_uri: playlist.tracks_uri.clone(),
                });
                state.confirm_selected = 1;
            }
        }
        Action::Themes => super::theme_picker::open_theme_picker(state),
        Action::Palette => state.palette = Some(Default::default()),
        Action::ToggleHistory => {
            state.history_visible = !state.history_visible;
            state.history_selected = 0;
        }
        Action::ToggleQueue => {
            state.queue_visible = !state.queue_visible;
            state.queue_scroll = 0;
            if state.queue_visible
                && let Some(current_idx) = state.current_playing_index {
                    // In radio mode an empty queue means related tracks are loading;
                    // rebuilding from the chain would replay what was already heard.
                    if state.auto_queue.is_empty() && state.playback_source != PlaybackSource::Radio {
                        state.auto_queue = build_queue(
                            current_idx,
                            active_tracks(state, data),
                            state.shuffle_enabled,
                        );
                    }
                }
        }
        _ => {}
    }
    InputOutcome::Continue
}

/// Like/unlike or follow/unfollow the focused pane's selected row: the track in a
/// tracks pane, the playlist, album or person in a container pane. Never the parent of
/// a focused track.
fn toggle_like_selected(state: &mut AppState, data: &mut AppData) {
    let id = nav::focused(state);
    let Some(idx) = nav::selected_index(state, id) else { return };
    match id {
        PaneId::Playlists => toggle_playlist_like(data.playlists.get(idx).cloned(), state, data),
        PaneId::SearchPlaylists => toggle_playlist_like(data.search_playlists.get(idx).cloned(), state, data),
        PaneId::Albums => toggle_album_like(data.albums.get(idx).cloned(), state, data),
        PaneId::SearchAlbums => toggle_album_like(data.search_albums.get(idx).cloned(), state, data),
        PaneId::Following => toggle_follow(data.following.get(idx).cloned(), state, data),
        PaneId::SearchPeople => toggle_follow(data.search_people.get(idx).cloned(), state, data),
        // A feed item is not something SoundCloud lets you like; its tracks are.
        PaneId::Feed => {}
        _ => {
            let views = build_filtered_views(state, data);
            toggle_track_like(nav::selected_track(state, data, &views), state, data);
        }
    }
}

fn toggle_playlist_like(playlist: Option<Playlist>, state: &mut AppState, data: &mut AppData) {
    let Some(mut playlist) = playlist else { return };
    let Some(playlist_id) = soundcloud_playlist_id_from_tracks_uri(&playlist.tracks_uri) else { return };
    if data.liked_playlist_uris.remove(&playlist.tracks_uri) {
        state.toast(format!("Unliked {}", playlist.title));
        state.engagement_queue.push_back(Engagement::UnlikePlaylist {
            tracks_uri: playlist.tracks_uri,
            playlist_id,
        });
    } else {
        data.liked_playlist_uris.insert(playlist.tracks_uri.clone());
        state.toast(format!("Liked {}", playlist.title));
        // The Library row this adds is the liked copy, never an owned playlist.
        playlist.is_owned = false;
        state.engagement_queue.push_back(Engagement::LikePlaylist { playlist, playlist_id });
    }
}

fn toggle_album_like(album: Option<Album>, state: &mut AppState, data: &mut AppData) {
    let Some(album) = album else { return };
    let Some(playlist_id) = soundcloud_playlist_id_from_tracks_uri(&album.tracks_uri) else { return };
    if data.liked_album_uris.remove(&album.tracks_uri) {
        state.toast(format!("Unliked {}", album.title));
        state.engagement_queue.push_back(Engagement::UnlikeAlbum {
            tracks_uri: album.tracks_uri,
            playlist_id,
        });
    } else {
        data.liked_album_uris.insert(album.tracks_uri.clone());
        state.toast(format!("Liked {}", album.title));
        state.engagement_queue.push_back(Engagement::LikeAlbum { album, playlist_id });
    }
}

fn toggle_follow(artist: Option<Artist>, state: &mut AppState, data: &mut AppData) {
    let Some(artist) = artist else { return };
    let Some(user_id) = soundcloud_id_from_urn(&artist.urn) else { return };
    if data.followed_user_urns.remove(&artist.urn) {
        state.toast(format!("Unfollowed {}", artist.name));
        state.engagement_queue.push_back(Engagement::UnfollowUser { urn: artist.urn, user_id });
    } else {
        data.followed_user_urns.insert(artist.urn.clone());
        state.toast(format!("Followed {}", artist.name));
        state.engagement_queue.push_back(Engagement::FollowUser { artist, user_id });
    }
}

/// Like/unlike a track, optimistically.
fn toggle_track_like(track: Option<Track>, state: &mut AppState, data: &mut AppData) {
    let Some(track) = track else { return };
    let Some(track_id) = soundcloud_id_from_urn(&track.track_urn) else { return };
    if data.liked_track_urns.remove(&track.track_urn) {
        state.toast(format!("Unliked {}", track.title));
        state.engagement_queue.push_back(Engagement::UnlikeTrack {
            track_urn: track.track_urn,
            track_id,
        });
    } else {
        data.liked_track_urns.insert(track.track_urn.clone());
        state.toast(format!("Liked {}", track.title));
        state
            .engagement_queue
            .push_back(Engagement::LikeTrack { track, track_id });
    }
}
