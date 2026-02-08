use ratatui::crossterm::event::{KeyEvent, KeyModifiers};

use super::InputOutcome;
use crate::tui::logic::state::{AppData, AppState, EngagementAction, FollowingTracksFocus, PlaybackSource};
use crate::player::Player;
use crate::tui::logic::utils::build_queue;
use crate::tui::logic::utils::build_search_matches;
use crate::tui::logic::utils::{soundcloud_id_from_urn, soundcloud_playlist_id_from_tracks_uri};

use super::queue::{handle_add_to_queue, handle_add_next_to_queue};

pub(crate) fn handle_char(
    key: KeyEvent,
    c: char,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        handle_shift_char(c, state, data, player)
    } else if state.selected_tab == 0 {
        handle_space(c, player)
    } else if state.selected_tab == 1 {
        handle_search_char(c, state)
    } else {
        InputOutcome::Continue
    }
}

pub(crate) fn handle_backspace(state: &mut AppState) -> InputOutcome {
    if state.selected_tab == 1 {
        state.query.pop();
        state.search_needs_fetch = true;
        state.selected_row = 0;
        state.search_selected_playlist_track_row = 0;
        state.search_selected_album_track_row = 0;
        state.search_selected_person_track_row = 0;
        state.search_selected_person_like_row = 0;
        state.search_people_tracks_focus = FollowingTracksFocus::Published;
    }
    InputOutcome::Continue
}

fn handle_shift_char(
    c: char,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    match c {
        'u' | 'U' => {
            player.volume_up();
        }
        'd' | 'D' => {
            player.volume_down();
        }
        's' | 'S' => {
            state.shuffle_enabled = !state.shuffle_enabled;
            if let Some(current_idx) = state.current_playing_index {
                let active_tracks = match state.playback_source {
                    PlaybackSource::Likes => &data.likes,
                    PlaybackSource::Playlist
                    | PlaybackSource::Album
                    | PlaybackSource::FollowingPublished
                    | PlaybackSource::FollowingLikes => &data.playback_tracks,
                };
                state.auto_queue =
                    build_queue(current_idx, active_tracks, state.shuffle_enabled);
            }
        }
        'r' | 'R' => {
            state.repeat_enabled = !state.repeat_enabled;
        }
        'a' | 'A' => {
            handle_add_to_queue(state, data);
        }
        'n' | 'N' => {
            handle_add_next_to_queue(state, data);
        }
        'l' | 'L' => {
            enqueue_like_follow_selected(state, data);
        }
        'f' | 'F' => {
            if state.selected_tab == 0 {
                state.search_popup_visible = true;
                state.search_query.clear();
                state.search_matches = build_search_matches(
                    state.selected_subtab,
                    &state.search_query,
                    &data.likes,
                    &data.playlists,
                    &data.playlist_tracks,
                    &data.albums,
                    &data.following,
                );
            }
        }
        'h' | 'H' => {
            state.help_visible = !state.help_visible;
        }
        'v' | 'V' => {
            state.visualizer_mode = !state.visualizer_mode;
        }
        'q' | 'Q' => {
            state.queue_visible = !state.queue_visible;
            if state.queue_visible {
                if let Some(current_idx) = state.current_playing_index {
                    if state.auto_queue.is_empty() {
                        let active_tracks = match state.playback_source {
                            PlaybackSource::Likes => &data.likes,
                            PlaybackSource::Playlist
                            | PlaybackSource::Album
                            | PlaybackSource::FollowingPublished
                            | PlaybackSource::FollowingLikes => &data.playback_tracks,
                        };
                        state.auto_queue = build_queue(
                            current_idx,
                            active_tracks,
                            state.shuffle_enabled,
                        );
                    }
                }
            }
        }
        'j' | 'J' => {
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
        'k' | 'K' => {
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
                let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
                let selected_idx = if search_active {
                    state.search_matches.get(state.selected_row).copied()
                } else {
                    Some(state.selected_row)
                };
                let track = selected_idx.and_then(|idx| data.likes.get(idx));
                if let Some(track) = track {
                    if let Some(track_id) = soundcloud_id_from_urn(&track.track_urn) {
                        data.liked_track_urns.remove(&track.track_urn);
                        state
                            .engagement_queue
                            .push_back(EngagementAction::UnlikeTrack {
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
                            state.engagement_queue.push_back(EngagementAction::UnlikePlaylist {
                                tracks_uri: playlist.tracks_uri.clone(),
                                playlist_id,
                            });
                        } else {
                            data.liked_playlist_uris.insert(playlist.tracks_uri.clone());
                            let mut liked_playlist = playlist.clone();
                            liked_playlist.is_owned = false;
                            state
                                .engagement_queue
                                .push_back(EngagementAction::LikePlaylist {
                                playlist: liked_playlist,
                                    playlist_id,
                                });
                        }
                    }
                }
            }
            2 => {
                let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
                let selected_idx = if search_active {
                    state.search_matches.get(state.selected_row).copied()
                } else {
                    Some(state.selected_row)
                };
                let album = selected_idx.and_then(|idx| data.albums.get(idx));
                if let Some(album) = album {
                    if let Some(playlist_id) =
                        soundcloud_playlist_id_from_tracks_uri(&album.tracks_uri)
                    {
                        data.liked_album_uris.remove(&album.tracks_uri);
                        state.engagement_queue.push_back(EngagementAction::UnlikeAlbum {
                            tracks_uri: album.tracks_uri.clone(),
                            playlist_id,
                        });
                    }
                }
            }
            3 => {
                let search_active = state.search_popup_visible && !state.search_query.trim().is_empty();
                let selected_idx = if search_active {
                    state.search_matches.get(state.selected_row).copied()
                } else {
                    Some(state.selected_row)
                };
                let artist = selected_idx.and_then(|idx| data.following.get(idx));
                if let Some(artist) = artist {
                    if let Some(user_id) = soundcloud_id_from_urn(&artist.urn) {
                        data.followed_user_urns.remove(&artist.urn);
                        state.engagement_queue.push_back(EngagementAction::UnfollowUser {
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
            0 => {
                if let Some(track) = data.search_tracks.get(state.selected_row) {
                    if let Some(track_id) = soundcloud_id_from_urn(&track.track_urn) {
                        let is_liked = data.liked_track_urns.contains(&track.track_urn);
                        if is_liked {
                            data.liked_track_urns.remove(&track.track_urn);
                            state.engagement_queue.push_back(EngagementAction::UnlikeTrack {
                                track_urn: track.track_urn.clone(),
                                track_id,
                            });
                        } else {
                            data.liked_track_urns.insert(track.track_urn.clone());
                            state.engagement_queue.push_back(EngagementAction::LikeTrack {
                                track: track.clone(),
                                track_id,
                            });
                        }
                    }
                }
            }
            1 => {
                if let Some(album) = data.search_albums.get(state.selected_row) {
                    if let Some(playlist_id) =
                        soundcloud_playlist_id_from_tracks_uri(&album.tracks_uri)
                    {
                        let is_liked = data.liked_album_uris.contains(&album.tracks_uri);
                        if is_liked {
                            data.liked_album_uris.remove(&album.tracks_uri);
                            state.engagement_queue.push_back(EngagementAction::UnlikeAlbum {
                                tracks_uri: album.tracks_uri.clone(),
                                playlist_id,
                            });
                        } else {
                            data.liked_album_uris.insert(album.tracks_uri.clone());
                            state.engagement_queue.push_back(EngagementAction::LikeAlbum {
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
                            state.engagement_queue.push_back(EngagementAction::UnlikePlaylist {
                                tracks_uri: playlist.tracks_uri.clone(),
                                playlist_id,
                            });
                        } else {
                            data.liked_playlist_uris.insert(playlist.tracks_uri.clone());
                            state
                                .engagement_queue
                                .push_back(EngagementAction::LikePlaylist {
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
                            state.engagement_queue.push_back(EngagementAction::UnfollowUser {
                                urn: artist.urn.clone(),
                                user_id,
                            });
                        } else {
                            data.followed_user_urns.insert(artist.urn.clone());
                            state.engagement_queue.push_back(EngagementAction::FollowUser {
                                artist: artist.clone(),
                                user_id,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn handle_space(c: char, player: &Player) -> InputOutcome {
    if c == ' ' {
        if player.is_playing() {
            player.pause();
        } else {
            player.resume();
        }
    }
    InputOutcome::Continue
}

fn handle_search_char(c: char, state: &mut AppState) -> InputOutcome {
    state.query.push(c);
    state.search_needs_fetch = true;
    state.selected_row = 0;
    state.search_selected_playlist_track_row = 0;
    state.search_selected_album_track_row = 0;
    state.search_selected_person_track_row = 0;
    state.search_selected_person_like_row = 0;
    state.search_people_tracks_focus = FollowingTracksFocus::Published;
    InputOutcome::Continue
}
