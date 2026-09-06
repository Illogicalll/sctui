use crate::keymap::Action;

use super::InputOutcome;
use crate::api::Track;
use crate::tui::logic::state::{AppData, AppState, ConfirmAction, Engagement, FollowingTracksFocus, PlaybackSource};
use crate::player::Player;
use crate::tui::logic::utils::{active_tracks, build_queue};
use crate::tui::logic::utils::build_search_matches;
use crate::tui::logic::utils::{soundcloud_id_from_urn, soundcloud_playlist_id_from_tracks_uri};

use super::helpers::{filtered_row, reset_search_rows};
use super::queue::{handle_add_to_queue, handle_add_next_to_queue, selected_queued};

pub(crate) fn handle_backspace(state: &mut AppState) -> InputOutcome {
    if state.selected_tab == 1 {
        state.query.pop();
        state.search_needs_fetch = true;
        reset_search_rows(state);
    }
    InputOutcome::Continue
}

/// Commands that are plain state changes; movement, playback and tab actions
/// are dispatched in `input::run_action`.
pub(crate) fn run_command(
    action: Action,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    match action {
        Action::VolumeUp => {
            player.volume_up();
        }
        Action::VolumeDown => {
            player.volume_down();
        }
        Action::ToggleShuffle => {
            state.shuffle_enabled = !state.shuffle_enabled;
            if let Some(current_idx) = state.current_playing_index {
                state.auto_queue =
                    build_queue(current_idx, active_tracks(state, data), state.shuffle_enabled);
            }
        }
        Action::ToggleRepeat => {
            state.repeat_enabled = !state.repeat_enabled;
        }
        Action::AddToQueue => {
            handle_add_to_queue(state, data);
        }
        Action::PlayNext => {
            handle_add_next_to_queue(state, data);
        }
        Action::ToggleLike => {
            enqueue_like_follow_selected(state, data);
        }
        Action::Search => {
            if state.selected_tab == 1 {
                state.search_typing = true;
            } else if state.selected_tab == 0 {
                state.search_popup_visible = true;
                state.search_query.clear();
                state.search_matches = build_search_matches(
                    state.selected_subtab,
                    &state.search_query,
                    &data.likes,
                    &data.playlist_tracks,
                    &data.albums,
                    &data.following,
                );
            }
        }
        Action::Help => {
            state.help_visible = !state.help_visible;
        }
        Action::ToggleVisualizer => {
            state.visualizer_mode = !state.visualizer_mode;
        }
        // Playlist management. Both destructive actions go through the confirm popup.
        Action::AddToPlaylist => {
            if let Some(queued) = selected_queued(state, data) {
                super::playlist_picker::open_picker(state, queued.track);
            }
        }
        Action::NewPlaylist => {
            if state.selected_tab == 0 && state.selected_subtab == 1 {
                super::playlist_picker::open_new_playlist_prompt(state);
            }
        }
        Action::RemoveFromPlaylist => {
            if state.selected_tab == 0 && state.selected_subtab == 1 {
                let open_playlist = data.playlists.iter().position(|p| {
                    Some(p.tracks_uri.as_str()) == data.playlist_tracks_uri.as_deref()
                });
                let track = filtered_row(state, state.selected_playlist_track_row)
                    .and_then(|idx| data.playlist_tracks.get(idx).cloned());
                if let (Some(playlist_idx), Some(track)) = (open_playlist, track)
                    && data.playlists[playlist_idx].is_owned
                {
                    state.confirm = Some(ConfirmAction::RemoveTrack { playlist_idx, track });
                    state.confirm_selected = 1;
                }
            }
        }
        Action::DeletePlaylist => {
            if state.selected_tab == 0
                && state.selected_subtab == 1
                && data.playlists.get(state.selected_row).is_some_and(|p| p.is_owned)
            {
                state.confirm = Some(ConfirmAction::DeletePlaylist {
                    playlist_idx: state.selected_row,
                });
                state.confirm_selected = 1;
            }
        }
        Action::ToggleHistory => {
            state.history_visible = !state.history_visible;
            state.history_selected = 0;
        }
        Action::ToggleQueue => {
            state.queue_visible = !state.queue_visible;
            if state.queue_visible {
                if let Some(current_idx) = state.current_playing_index {
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
        }
        Action::TertiaryDown => {
            if state.selected_tab == 0 && state.selected_subtab == 3 {
                if state.selected_following_like_row + 1 < data.following_likes_tracks.len() {
                    state.selected_following_like_row += 1;
                    state.following_tracks_focus = FollowingTracksFocus::Likes;
                    data.following_likes_state
                        .select(Some(state.selected_following_like_row));
                }
            } else if state.selected_tab == 1 && state.selected_searchfilter == 3 {
                if state.search_selected_person_like_row + 1 < data.search_people_likes_tracks.len() {
                    state.search_selected_person_like_row += 1;
                    state.search_people_tracks_focus = FollowingTracksFocus::Likes;
                    data.search_people_likes_state
                        .select(Some(state.search_selected_person_like_row));
                }
            }
        }
        Action::TertiaryUp => {
            if state.selected_tab == 0 && state.selected_subtab == 3 {
                if state.selected_following_like_row > 0 {
                    state.selected_following_like_row -= 1;
                    state.following_tracks_focus = FollowingTracksFocus::Likes;
                    data.following_likes_state
                        .select(Some(state.selected_following_like_row));
                }
            } else if state.selected_tab == 1 && state.selected_searchfilter == 3 {
                if state.search_selected_person_like_row > 0 {
                    state.search_selected_person_like_row -= 1;
                    state.search_people_tracks_focus = FollowingTracksFocus::Likes;
                    data.search_people_likes_state
                        .select(Some(state.search_selected_person_like_row));
                }
            }
        }
        _ => {}
    }
    InputOutcome::Continue
}

fn enqueue_like_follow_selected(state: &mut AppState, data: &mut AppData) {
    if state.selected_tab == 0 {
        match state.selected_subtab {
            0 => {
                let track = filtered_row(state, state.selected_row).and_then(|idx| data.likes.get(idx));
                if let Some(track) = track {
                    if let Some(track_id) = soundcloud_id_from_urn(&track.track_urn) {
                        data.liked_track_urns.remove(&track.track_urn);
                        state
                            .engagement_queue
                            .push_back(Engagement::UnlikeTrack {
                                track_urn: track.track_urn.clone(),
                                track_id,
                            });
                    }
                }
            }
            1 => {
                if let Some(playlist) = data.playlists.get(state.selected_row) {
                    if let Some(playlist_id) =
                        soundcloud_playlist_id_from_tracks_uri(&playlist.tracks_uri)
                    {
                        let is_liked = data.liked_playlist_uris.contains(&playlist.tracks_uri);
                        if is_liked {
                            data.liked_playlist_uris.remove(&playlist.tracks_uri);
                            state.engagement_queue.push_back(Engagement::UnlikePlaylist {
                                tracks_uri: playlist.tracks_uri.clone(),
                                playlist_id,
                            });
                        } else {
                            data.liked_playlist_uris.insert(playlist.tracks_uri.clone());
                            let mut liked_playlist = playlist.clone();
                            liked_playlist.is_owned = false;
                            state
                                .engagement_queue
                                .push_back(Engagement::LikePlaylist {
                                playlist: liked_playlist,
                                    playlist_id,
                                });
                        }
                    }
                }
            }
            2 => {
                let album = filtered_row(state, state.selected_row).and_then(|idx| data.albums.get(idx));
                if let Some(album) = album {
                    if let Some(playlist_id) =
                        soundcloud_playlist_id_from_tracks_uri(&album.tracks_uri)
                    {
                        data.liked_album_uris.remove(&album.tracks_uri);
                        state.engagement_queue.push_back(Engagement::UnlikeAlbum {
                            tracks_uri: album.tracks_uri.clone(),
                            playlist_id,
                        });
                    }
                }
            }
            3 => {
                let artist = filtered_row(state, state.selected_row).and_then(|idx| data.following.get(idx));
                if let Some(artist) = artist {
                    if let Some(user_id) = soundcloud_id_from_urn(&artist.urn) {
                        data.followed_user_urns.remove(&artist.urn);
                        state.engagement_queue.push_back(Engagement::UnfollowUser {
                            urn: artist.urn.clone(),
                            user_id,
                        });
                    }
                }
            }
            _ => {}
        }
    } else if state.selected_tab == 1 {
        match state.selected_searchfilter {
            0 => toggle_track_like(data.search_tracks.get(state.selected_row).cloned(), state, data),
            1 => {
                if let Some(album) = data.search_albums.get(state.selected_row) {
                    if let Some(playlist_id) =
                        soundcloud_playlist_id_from_tracks_uri(&album.tracks_uri)
                    {
                        let is_liked = data.liked_album_uris.contains(&album.tracks_uri);
                        if is_liked {
                            data.liked_album_uris.remove(&album.tracks_uri);
                            state.engagement_queue.push_back(Engagement::UnlikeAlbum {
                                tracks_uri: album.tracks_uri.clone(),
                                playlist_id,
                            });
                        } else {
                            data.liked_album_uris.insert(album.tracks_uri.clone());
                            state.engagement_queue.push_back(Engagement::LikeAlbum {
                                album: album.clone(),
                                playlist_id,
                            });
                        }
                    }
                }
            }
            2 => {
                if let Some(playlist) = data.search_playlists.get(state.selected_row) {
                    if let Some(playlist_id) =
                        soundcloud_playlist_id_from_tracks_uri(&playlist.tracks_uri)
                    {
                        let is_liked = data.liked_playlist_uris.contains(&playlist.tracks_uri);
                        if is_liked {
                            data.liked_playlist_uris.remove(&playlist.tracks_uri);
                            state.engagement_queue.push_back(Engagement::UnlikePlaylist {
                                tracks_uri: playlist.tracks_uri.clone(),
                                playlist_id,
                            });
                        } else {
                            data.liked_playlist_uris.insert(playlist.tracks_uri.clone());
                            state
                                .engagement_queue
                                .push_back(Engagement::LikePlaylist {
                                    playlist: playlist.clone(),
                                    playlist_id,
                                });
                        }
                    }
                }
            }
            3 => {
                if let Some(artist) = data.search_people.get(state.selected_row) {
                    if let Some(user_id) = soundcloud_id_from_urn(&artist.urn) {
                        let is_followed = data.followed_user_urns.contains(&artist.urn);
                        if is_followed {
                            data.followed_user_urns.remove(&artist.urn);
                            state.engagement_queue.push_back(Engagement::UnfollowUser {
                                urn: artist.urn.clone(),
                                user_id,
                            });
                        } else {
                            data.followed_user_urns.insert(artist.urn.clone());
                            state.engagement_queue.push_back(Engagement::FollowUser {
                                artist: artist.clone(),
                                user_id,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    } else if state.selected_tab == 2 {
        toggle_track_like(data.feed_tracks.get(state.selected_info_row).cloned(), state, data);
    }
}

/// Like/unlike a track from outside the Likes list, optimistically.
fn toggle_track_like(track: Option<Track>, state: &mut AppState, data: &mut AppData) {
    let Some(track) = track else { return };
    let Some(track_id) = soundcloud_id_from_urn(&track.track_urn) else { return };
    if data.liked_track_urns.remove(&track.track_urn) {
        state.engagement_queue.push_back(Engagement::UnlikeTrack {
            track_urn: track.track_urn,
            track_id,
        });
    } else {
        data.liked_track_urns.insert(track.track_urn.clone());
        state
            .engagement_queue
            .push_back(Engagement::LikeTrack { track, track_id });
    }
}

pub(crate) fn handle_search_char(c: char, state: &mut AppState) -> InputOutcome {
    state.query.push(c);
    state.search_needs_fetch = true;
    reset_search_rows(state);
    InputOutcome::Continue
}
