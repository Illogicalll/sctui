use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::player::Player;

use super::state::{
    AppData, AppState, FollowingTracksFocus, PlaybackSource, QueuedTrack, info_table_rows_count,
    table_rows_count,
};
use crate::tui::logic::utils::{
    build_queue, build_search_matches, play_queued_track, queued_from_current,
};

pub enum InputOutcome {
    Continue,
    Quit,
}

fn insert_manual_queue(state: &mut AppState, queued: QueuedTrack) {
    let mut items: Vec<QueuedTrack> = state.manual_queue.drain(..).collect();
    let insert_idx = match items.iter().rposition(|item| item.user_added) {
        Some(idx) => idx + 1,
        None => 0,
    };
    items.insert(insert_idx, queued);
    state.manual_queue = items.into_iter().collect();
}

pub fn handle_key_event(
    key: KeyEvent,
    state: &mut AppState,
    data: &mut AppData,
    player: &Player,
) -> InputOutcome {
    if state.quit_confirm_visible {
        match key.code {
            KeyCode::Esc => {
                state.quit_confirm_visible = false;
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                state.quit_confirm_selected = if state.quit_confirm_selected == 0 { 1 } else { 0 };
            }
            KeyCode::Enter => {
                if state.quit_confirm_selected == 0 {
                    return InputOutcome::Quit;
                } else {
                    state.quit_confirm_visible = false;
                }
            }
            _ => {}
        }
        return InputOutcome::Continue;
    }

    if state.search_popup_visible {
        let mut handled = true;
        match key.code {
            KeyCode::Backspace => {
                state.search_query.pop();
                state.search_matches = build_search_matches(
                    state.selected_subtab,
                    &state.search_query,
                    &data.likes,
                    &data.playlists,
                    &data.albums,
                    &data.following,
                );
            }
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    if c == 'f' || c == 'F' {
                        state.search_popup_visible = false;
                        state.search_query.clear();
                        state.search_matches.clear();
                    } else {
                        handled = false;
                    }
                } else {
                    state.search_query.push(c);
                    state.search_matches = build_search_matches(
                        state.selected_subtab,
                        &state.search_query,
                        &data.likes,
                        &data.playlists,
                        &data.albums,
                        &data.following,
                    );
                }
            }
            _ => {
                handled = false;
            }
        }
        if handled {
            return InputOutcome::Continue;
        }
    }

    if player.is_seeking() {
        return InputOutcome::Continue;
    }

    match key.code {
        KeyCode::Esc => {
            state.quit_confirm_visible = true;
            state.quit_confirm_selected = 1;
        }
        KeyCode::Tab => {
            state.selected_tab = (state.selected_tab + 1) % 3;
            state.selected_row = 0;
        }
        KeyCode::Right => {
            if key.modifiers.contains(KeyModifiers::ALT) {
                if player.is_playing() || state.current_playing_index.is_some() {
                    player.fast_forward();
                }
            } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                if let Some(current_idx) = state.current_playing_index {
                    let active_tracks = match state.playback_source {
                        PlaybackSource::Likes => &data.likes,
                        PlaybackSource::Playlist
                        | PlaybackSource::Album
                        | PlaybackSource::FollowingPublished
                        | PlaybackSource::FollowingLikes => &data.playback_tracks,
                    };
                    if state.manual_queue.is_empty() && state.auto_queue.is_empty() {
                        state.auto_queue =
                            build_queue(current_idx, active_tracks, state.shuffle_enabled);
                    }
                        if let Some(queued) = state.manual_queue.pop_front() {
                            if let Some(current) = queued_from_current(state, data) {
                                state.playback_history.push(current);
                            }
                            play_queued_track(queued, state, data, player, true);
                        } else if let Some(next_idx) = state.auto_queue.pop_front() {
                            if let Some(track) = active_tracks.get(next_idx) {
                                if let Some(current) = queued_from_current(state, data) {
                                    state.playback_history.push(current);
                                }
                                player.play(track.clone());
                                state.override_playing = None;
                                state.current_playing_index = Some(next_idx);
                            if state.playback_source == PlaybackSource::Likes
                                && state.selected_tab == 0
                                && state.selected_subtab == 0
                            {
                                state.selected_row = next_idx;
                                data.likes_state.select(Some(next_idx));
                            } else if state.playback_source == PlaybackSource::Playlist
                                && state.selected_tab == 0
                                && state.selected_subtab == 1
                                && data.playback_playlist_uri.is_some()
                                && data.playback_playlist_uri == data.playlist_tracks_uri
                            {
                                state.selected_playlist_track_row = next_idx;
                                data.playlist_tracks_state.select(Some(next_idx));
                            } else if state.playback_source == PlaybackSource::Album
                                && state.selected_tab == 0
                                && state.selected_subtab == 2
                                && data.playback_album_uri.is_some()
                                && data.playback_album_uri == data.album_tracks_uri
                            {
                                state.selected_album_track_row = next_idx;
                                data.album_tracks_state.select(Some(next_idx));
                            } else if state.playback_source
                                == PlaybackSource::FollowingPublished
                                && state.selected_tab == 0
                                && state.selected_subtab == 3
                                && data.playback_following_user_urn.is_some()
                                && data.playback_following_user_urn
                                    == data.following_tracks_user_urn
                            {
                                state.selected_following_track_row = next_idx;
                                state.following_tracks_focus = FollowingTracksFocus::Published;
                                data.following_tracks_state.select(Some(next_idx));
                            } else if state.playback_source == PlaybackSource::FollowingLikes
                                && state.selected_tab == 0
                                && state.selected_subtab == 3
                                && data.playback_following_user_urn.is_some()
                                && data.playback_following_user_urn
                                    == data.following_likes_user_urn
                            {
                                state.selected_following_like_row = next_idx;
                                state.following_tracks_focus = FollowingTracksFocus::Likes;
                                data.following_likes_state.select(Some(next_idx));
                            }
                        }
                    }
                }
            } else if state.selected_tab == 0 {
                if state.selected_subtab == 1 {
                    state.selected_playlist_row = state.selected_row;
                }
                if state.selected_subtab == 2 {
                    state.selected_album_row = state.selected_row;
                }
                state.selected_subtab = (state.selected_subtab + 1) % 4;
                if state.selected_subtab == 1 {
                    state.selected_row = state.selected_playlist_row;
                    data.playlists_state.select(Some(state.selected_row));
                } else if state.selected_subtab == 2 {
                    state.selected_row = state.selected_album_row;
                    data.albums_state.select(Some(state.selected_row));
                } else {
                    state.selected_row = 0;
                }
            } else if state.selected_tab == 1 {
                state.selected_searchfilter = (state.selected_searchfilter + 1) % 4;
                state.selected_row = 0;
            } else if state.selected_tab == 2 {
                state.info_pane_selected = !state.info_pane_selected;
            }
        }
        KeyCode::Left => {
            if key.modifiers.contains(KeyModifiers::ALT) {
                if player.is_playing() || state.current_playing_index.is_some() {
                    player.rewind();
                }
            } else if key.modifiers.contains(KeyModifiers::SHIFT) {
                if state.current_playing_index.is_some() {
                    if let Some(prev) = state.playback_history.pop() {
                        if let Some(current) = queued_from_current(state, data) {
                            let mut current = current;
                            current.user_added = false;
                            state.manual_queue.push_front(current);
                        }
                        play_queued_track(prev, state, data, player, true);
                    }
                }
            } else if state.selected_tab == 0 {
                if state.selected_subtab == 1 {
                    state.selected_playlist_row = state.selected_row;
                }
                if state.selected_subtab == 2 {
                    state.selected_album_row = state.selected_row;
                }
                state.selected_subtab = if state.selected_subtab == 0 {
                    3
                } else {
                    state.selected_subtab - 1
                };
                if state.selected_subtab == 1 {
                    state.selected_row = state.selected_playlist_row;
                    data.playlists_state.select(Some(state.selected_row));
                } else if state.selected_subtab == 2 {
                    state.selected_row = state.selected_album_row;
                    data.albums_state.select(Some(state.selected_row));
                } else {
                    state.selected_row = 0;
                }
            } else if state.selected_tab == 1 {
                state.selected_searchfilter = if state.selected_searchfilter == 0 {
                    3
                } else {
                    state.selected_searchfilter - 1
                };
                state.selected_row = 0;
            } else if state.selected_tab == 2 {
                state.info_pane_selected = !state.info_pane_selected;
            }
        }
        KeyCode::Down => {
            if state.selected_tab == 0 && state.selected_subtab == 1 {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    if state.selected_row + 1 < data.playlists.len() {
                        state.selected_row += 1;
                        state.selected_playlist_row = state.selected_row;
                        data.playlists_state.select(Some(state.selected_row));
                    }
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    if !data.playlist_tracks.is_empty() {
                        state.selected_playlist_track_row = (state
                            .selected_playlist_track_row
                            + 10)
                            .min(data.playlist_tracks.len() - 1);
                        data.playlist_tracks_state
                            .select(Some(state.selected_playlist_track_row));
                    }
                } else if state.selected_playlist_track_row + 1 < data.playlist_tracks.len() {
                    state.selected_playlist_track_row += 1;
                    data.playlist_tracks_state
                        .select(Some(state.selected_playlist_track_row));
                }
            } else if state.selected_tab == 0 && state.selected_subtab == 2 {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    if state.selected_row + 1 < data.albums.len() {
                        state.selected_row += 1;
                        state.selected_album_row = state.selected_row;
                        data.albums_state.select(Some(state.selected_row));
                    }
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    if !data.album_tracks.is_empty() {
                        state.selected_album_track_row =
                            (state.selected_album_track_row + 10)
                                .min(data.album_tracks.len() - 1);
                        data.album_tracks_state
                            .select(Some(state.selected_album_track_row));
                    }
                } else if state.selected_album_track_row + 1 < data.album_tracks.len() {
                    state.selected_album_track_row += 1;
                    data.album_tracks_state
                        .select(Some(state.selected_album_track_row));
                }
            } else if state.selected_tab == 0 && state.selected_subtab == 3 {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    if state.selected_row + 1 < data.following.len() {
                        state.selected_row += 1;
                        data.following_state.select(Some(state.selected_row));
                    }
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    if !data.following_tracks.is_empty() {
                        state.selected_following_track_row =
                            (state.selected_following_track_row + 10)
                                .min(data.following_tracks.len() - 1);
                        state.following_tracks_focus = FollowingTracksFocus::Published;
                        data.following_tracks_state
                            .select(Some(state.selected_following_track_row));
                    }
                } else if state.selected_following_track_row + 1 < data.following_tracks.len() {
                    state.selected_following_track_row += 1;
                    state.following_tracks_focus = FollowingTracksFocus::Published;
                    data.following_tracks_state
                        .select(Some(state.selected_following_track_row));
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                let max_rows = table_rows_count(state.selected_subtab, data);
                let max_info_rows = info_table_rows_count();
                if state.selected_tab == 2 && state.info_pane_selected {
                    if max_info_rows > 0 {
                        state.selected_info_row =
                            (state.selected_info_row + 10).min(max_info_rows - 1);
                    }
                } else if max_rows > 0 {
                    state.selected_row = (state.selected_row + 10).min(max_rows - 1);
                    match state.selected_subtab {
                        0 => data.likes_state.select(Some(state.selected_row)),
                        1 => data.playlists_state.select(Some(state.selected_row)),
                        2 => data.albums_state.select(Some(state.selected_row)),
                        3 => data.following_state.select(Some(state.selected_row)),
                        _ => {}
                    }
                }
            } else {
                let max_rows = table_rows_count(state.selected_subtab, data);
                let max_info_rows = info_table_rows_count();
                if state.selected_tab == 2
                    && state.info_pane_selected
                    && state.selected_info_row + 1 < max_info_rows
                {
                    state.selected_info_row += 1;
                } else if state.selected_row + 1 < max_rows {
                    state.selected_row += 1;
                    if state.selected_subtab == 1 && state.selected_tab == 0 {
                        state.selected_playlist_row = state.selected_row;
                    }
                    if state.selected_subtab == 2 && state.selected_tab == 0 {
                        state.selected_album_row = state.selected_row;
                    }
                    match state.selected_subtab {
                        0 => data.likes_state.select(Some(state.selected_row)),
                        1 => data.playlists_state.select(Some(state.selected_row)),
                        2 => data.albums_state.select(Some(state.selected_row)),
                        3 => data.following_state.select(Some(state.selected_row)),
                        _ => {}
                    }
                }
            }
        }
        KeyCode::Up => {
            if state.selected_tab == 0 && state.selected_subtab == 1 {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    if state.selected_row > 0 {
                        state.selected_row -= 1;
                        state.selected_playlist_row = state.selected_row;
                        data.playlists_state.select(Some(state.selected_row));
                    }
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    state.selected_playlist_track_row =
                        state.selected_playlist_track_row.saturating_sub(10);
                    data.playlist_tracks_state
                        .select(Some(state.selected_playlist_track_row));
                } else if state.selected_playlist_track_row > 0 {
                    state.selected_playlist_track_row -= 1;
                    data.playlist_tracks_state
                        .select(Some(state.selected_playlist_track_row));
                }
            } else if state.selected_tab == 0 && state.selected_subtab == 2 {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    if state.selected_row > 0 {
                        state.selected_row -= 1;
                        state.selected_album_row = state.selected_row;
                        data.albums_state.select(Some(state.selected_row));
                    }
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    state.selected_album_track_row =
                        state.selected_album_track_row.saturating_sub(10);
                    data.album_tracks_state
                        .select(Some(state.selected_album_track_row));
                } else if state.selected_album_track_row > 0 {
                    state.selected_album_track_row -= 1;
                    data.album_tracks_state
                        .select(Some(state.selected_album_track_row));
                }
            } else if state.selected_tab == 0 && state.selected_subtab == 3 {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    if state.selected_row > 0 {
                        state.selected_row -= 1;
                        data.following_state.select(Some(state.selected_row));
                    }
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    state.selected_following_track_row =
                        state.selected_following_track_row.saturating_sub(10);
                    state.following_tracks_focus = FollowingTracksFocus::Published;
                    data.following_tracks_state
                        .select(Some(state.selected_following_track_row));
                } else if state.selected_following_track_row > 0 {
                    state.selected_following_track_row -= 1;
                    state.following_tracks_focus = FollowingTracksFocus::Published;
                    data.following_tracks_state
                        .select(Some(state.selected_following_track_row));
                }
            } else if key.modifiers.contains(KeyModifiers::ALT) {
                if state.selected_tab == 2 && state.info_pane_selected {
                    state.selected_info_row = state.selected_info_row.saturating_sub(10);
                } else {
                    state.selected_row = state.selected_row.saturating_sub(10);
                    match state.selected_subtab {
                        0 => data.likes_state.select(Some(state.selected_row)),
                        1 => data.playlists_state.select(Some(state.selected_row)),
                        2 => data.albums_state.select(Some(state.selected_row)),
                        3 => data.following_state.select(Some(state.selected_row)),
                        _ => {}
                    }
                }
            } else if state.selected_tab == 2
                && state.info_pane_selected
                && state.selected_info_row > 0
            {
                state.selected_info_row -= 1;
            } else if state.selected_row > 0 {
                state.selected_row -= 1;
                if state.selected_subtab == 1 && state.selected_tab == 0 {
                    state.selected_playlist_row = state.selected_row;
                }
                if state.selected_subtab == 2 && state.selected_tab == 0 {
                    state.selected_album_row = state.selected_row;
                }
                match state.selected_subtab {
                    0 => data.likes_state.select(Some(state.selected_row)),
                    1 => data.playlists_state.select(Some(state.selected_row)),
                    2 => data.albums_state.select(Some(state.selected_row)),
                    3 => data.following_state.select(Some(state.selected_row)),
                    _ => {}
                }
            }
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
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
                        if state.selected_tab == 0 {
                            match state.selected_subtab {
                                0 => {
                                    let search_active = state.search_popup_visible
                                        && !state.search_query.trim().is_empty();
                                    let selected_idx = if search_active {
                                        state.search_matches.get(state.selected_row).copied()
                                    } else {
                                        Some(state.selected_row)
                                    };
                                    if let Some(idx) = selected_idx {
                                        if let Some(track) = data.likes.get(idx) {
                                            if track.is_playable() {
                                                insert_manual_queue(
                                                    state,
                                                    QueuedTrack {
                                                        source: PlaybackSource::Likes,
                                                        index: idx,
                                                        track: track.clone(),
                                                        tracks_snapshot: None,
                                                        playlist_uri: None,
                                                        album_uri: None,
                                                        following_user_urn: None,
                                                        user_added: true,
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                                1 => {
                                    if let Some(track) =
                                        data.playlist_tracks.get(state.selected_playlist_track_row)
                                    {
                                        if track.is_playable() {
                                            insert_manual_queue(
                                                state,
                                                QueuedTrack {
                                                    source: PlaybackSource::Playlist,
                                                    index: state.selected_playlist_track_row,
                                                    track: track.clone(),
                                                    tracks_snapshot: Some(data.playlist_tracks.clone()),
                                                    playlist_uri: data.playlist_tracks_uri.clone(),
                                                    album_uri: None,
                                                    following_user_urn: None,
                                                    user_added: true,
                                                },
                                            );
                                        }
                                    }
                                }
                                2 => {
                                    if let Some(track) =
                                        data.album_tracks.get(state.selected_album_track_row)
                                    {
                                        if track.is_playable() {
                                            insert_manual_queue(
                                                state,
                                                QueuedTrack {
                                                    source: PlaybackSource::Album,
                                                    index: state.selected_album_track_row,
                                                    track: track.clone(),
                                                    tracks_snapshot: Some(data.album_tracks.clone()),
                                                    playlist_uri: None,
                                                    album_uri: data.album_tracks_uri.clone(),
                                                    following_user_urn: None,
                                                    user_added: true,
                                                },
                                            );
                                        }
                                    }
                                }
                                3 => {
                                    if state.following_tracks_focus == FollowingTracksFocus::Likes {
                                        if let Some(track) = data
                                            .following_likes_tracks
                                            .get(state.selected_following_like_row)
                                        {
                                            if track.is_playable() {
                                                insert_manual_queue(
                                                    state,
                                                    QueuedTrack {
                                                        source: PlaybackSource::FollowingLikes,
                                                        index: state.selected_following_like_row,
                                                        track: track.clone(),
                                                        tracks_snapshot: Some(
                                                            data.following_likes_tracks.clone(),
                                                        ),
                                                        playlist_uri: None,
                                                        album_uri: None,
                                                        following_user_urn: data
                                                            .following_likes_user_urn
                                                            .clone(),
                                                        user_added: true,
                                                    },
                                                );
                                            }
                                        }
                                    } else if let Some(track) = data
                                        .following_tracks
                                        .get(state.selected_following_track_row)
                                    {
                                        if track.is_playable() {
                                            insert_manual_queue(
                                                state,
                                                QueuedTrack {
                                                    source: PlaybackSource::FollowingPublished,
                                                    index: state.selected_following_track_row,
                                                    track: track.clone(),
                                                    tracks_snapshot: Some(data.following_tracks.clone()),
                                                    playlist_uri: None,
                                                    album_uri: None,
                                                    following_user_urn: data
                                                        .following_tracks_user_urn
                                                        .clone(),
                                                    user_added: true,
                                                },
                                            );
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    'n' | 'N' => {
                        if state.selected_tab == 0 {
                            let mut queued: Option<QueuedTrack> = None;
                            match state.selected_subtab {
                                0 => {
                                    let search_active = state.search_popup_visible
                                        && !state.search_query.trim().is_empty();
                                    let selected_idx = if search_active {
                                        state.search_matches.get(state.selected_row).copied()
                                    } else {
                                        Some(state.selected_row)
                                    };
                                    if let Some(idx) = selected_idx {
                                        if let Some(track) = data.likes.get(idx) {
                                            if track.is_playable() {
                                                queued = Some(QueuedTrack {
                                                    source: PlaybackSource::Likes,
                                                    index: idx,
                                                    track: track.clone(),
                                                    tracks_snapshot: None,
                                                    playlist_uri: None,
                                                    album_uri: None,
                                                    following_user_urn: None,
                                                    user_added: true,
                                                });
                                            }
                                        }
                                    }
                                }
                                1 => {
                                    if let Some(track) =
                                        data.playlist_tracks.get(state.selected_playlist_track_row)
                                    {
                                        if track.is_playable() {
                                            queued = Some(QueuedTrack {
                                                source: PlaybackSource::Playlist,
                                                index: state.selected_playlist_track_row,
                                                track: track.clone(),
                                                tracks_snapshot: Some(data.playlist_tracks.clone()),
                                                playlist_uri: data.playlist_tracks_uri.clone(),
                                                album_uri: None,
                                                following_user_urn: None,
                                                user_added: true,
                                            });
                                        }
                                    }
                                }
                                2 => {
                                    if let Some(track) =
                                        data.album_tracks.get(state.selected_album_track_row)
                                    {
                                        if track.is_playable() {
                                            queued = Some(QueuedTrack {
                                                source: PlaybackSource::Album,
                                                index: state.selected_album_track_row,
                                                track: track.clone(),
                                                tracks_snapshot: Some(data.album_tracks.clone()),
                                                playlist_uri: None,
                                                album_uri: data.album_tracks_uri.clone(),
                                                following_user_urn: None,
                                                user_added: true,
                                            });
                                        }
                                    }
                                }
                                3 => {
                                    if state.following_tracks_focus == FollowingTracksFocus::Likes {
                                        if let Some(track) = data
                                            .following_likes_tracks
                                            .get(state.selected_following_like_row)
                                        {
                                            if track.is_playable() {
                                                queued = Some(QueuedTrack {
                                                    source: PlaybackSource::FollowingLikes,
                                                    index: state.selected_following_like_row,
                                                    track: track.clone(),
                                                    tracks_snapshot: Some(
                                                        data.following_likes_tracks.clone(),
                                                    ),
                                                    playlist_uri: None,
                                                    album_uri: None,
                                                    following_user_urn: data
                                                        .following_likes_user_urn
                                                        .clone(),
                                                    user_added: true,
                                                });
                                            }
                                        }
                                    } else if let Some(track) = data
                                        .following_tracks
                                        .get(state.selected_following_track_row)
                                    {
                                        if track.is_playable() {
                                            queued = Some(QueuedTrack {
                                                source: PlaybackSource::FollowingPublished,
                                                index: state.selected_following_track_row,
                                                track: track.clone(),
                                                tracks_snapshot: Some(data.following_tracks.clone()),
                                                playlist_uri: None,
                                                album_uri: None,
                                                following_user_urn: data
                                                    .following_tracks_user_urn
                                                    .clone(),
                                                user_added: true,
                                            });
                                        }
                                    }
                                }
                                _ => {}
                            }

                            if let Some(queued) = queued {
                                state.manual_queue.push_front(queued);
                            }
                        }
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
                                &data.albums,
                                &data.following,
                            );
                        }
                    }
                    'h' | 'H' => {
                        state.help_visible = !state.help_visible;
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
                            if state.selected_following_like_row + 1
                                < data.following_likes_tracks.len()
                            {
                                state.selected_following_like_row += 1;
                                state.following_tracks_focus = FollowingTracksFocus::Likes;
                                data.following_likes_state
                                    .select(Some(state.selected_following_like_row));
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
                        }
                    }
                    _ => {}
                }
            } else if state.selected_tab == 0 {
                if c == ' ' {
                    if player.is_playing() {
                        player.pause();
                    } else {
                        player.resume();
                    }
                }
            } else if state.selected_tab == 1 {
                state.query.push(c);
            }
        }
        KeyCode::Backspace => {
            if state.selected_tab == 1 {
                state.query.pop();
            }
        }
        KeyCode::Enter => {
            if state.selected_tab == 0 && state.selected_subtab == 0 {
                let search_active =
                    state.search_popup_visible && !state.search_query.trim().is_empty();
                let selected_idx = if search_active {
                    state.search_matches.get(state.selected_row).copied()
                } else {
                    Some(state.selected_row)
                };
                if let Some(selected_idx) = selected_idx {
                    if let Some(track) = data.likes.get(selected_idx) {
                        if !track.is_playable() {
                            return InputOutcome::Continue;
                        }
                        if state.playback_source != PlaybackSource::Likes {
                            state.playback_history.clear();
                            state.manual_queue.clear();
                        } else if let Some(queued) = queued_from_current(state, data) {
                            if !(queued.source == PlaybackSource::Likes
                                && queued.index == selected_idx)
                            {
                                state.playback_history.push(queued);
                            }
                        }
                        player.play(track.clone());
                        state.playback_source = PlaybackSource::Likes;
                        state.override_playing = None;
                        state.current_playing_index = Some(selected_idx);
                    data.playback_playlist_uri = None;
                    data.playback_album_uri = None;
                        data.playback_following_user_urn = None;
                        state.auto_queue =
                            build_queue(selected_idx, &data.likes, state.shuffle_enabled);
                        if !search_active {
                            if state.selected_tab == 0 && state.selected_subtab == 0 {
                                state.selected_row = selected_idx;
                                data.likes_state.select(Some(state.selected_row));
                            }
                        }
                    }
                }
            } else if state.selected_tab == 0 && state.selected_subtab == 1 {
                if let Some(track) = data
                    .playlist_tracks
                    .get(state.selected_playlist_track_row)
                {
                    if !track.is_playable() {
                        return InputOutcome::Continue;
                    }
                    if state.playback_source != PlaybackSource::Playlist {
                        state.playback_history.clear();
                        state.manual_queue.clear();
                    } else if let Some(queued) = queued_from_current(state, data) {
                        if !(queued.source == PlaybackSource::Playlist
                            && queued.index == state.selected_playlist_track_row)
                        {
                            state.playback_history.push(queued);
                        }
                    }
                    player.play(track.clone());
                    state.playback_source = PlaybackSource::Playlist;
                    state.override_playing = None;
                    state.current_playing_index = Some(state.selected_playlist_track_row);
                    data.playback_tracks = data.playlist_tracks.clone();
                    data.playback_playlist_uri = data.playlist_tracks_uri.clone();
                    data.playback_album_uri = None;
                    data.playback_following_user_urn = None;
                    state.auto_queue = build_queue(
                        state.selected_playlist_track_row,
                        &data.playback_tracks,
                        state.shuffle_enabled,
                    );
                    data.playlist_tracks_state
                        .select(Some(state.selected_playlist_track_row));
                }
            } else if state.selected_tab == 0 && state.selected_subtab == 2 {
                if let Some(track) = data
                    .album_tracks
                    .get(state.selected_album_track_row)
                {
                    if !track.is_playable() {
                        return InputOutcome::Continue;
                    }
                    if state.playback_source != PlaybackSource::Album {
                        state.playback_history.clear();
                        state.manual_queue.clear();
                    } else if let Some(queued) = queued_from_current(state, data) {
                        if !(queued.source == PlaybackSource::Album
                            && queued.index == state.selected_album_track_row)
                        {
                            state.playback_history.push(queued);
                        }
                    }
                    player.play(track.clone());
                    state.playback_source = PlaybackSource::Album;
                    state.override_playing = None;
                    state.current_playing_index = Some(state.selected_album_track_row);
                    data.playback_tracks = data.album_tracks.clone();
                    data.playback_playlist_uri = None;
                    data.playback_album_uri = data.album_tracks_uri.clone();
                    data.playback_following_user_urn = None;
                    state.auto_queue = build_queue(
                        state.selected_album_track_row,
                        &data.playback_tracks,
                        state.shuffle_enabled,
                    );
                    data.album_tracks_state
                        .select(Some(state.selected_album_track_row));
                }
            } else if state.selected_tab == 0 && state.selected_subtab == 3 {
                let (tracks, selected_idx, new_source, user_urn) =
                    if state.following_tracks_focus == FollowingTracksFocus::Likes {
                        (
                            &data.following_likes_tracks,
                            state.selected_following_like_row,
                            PlaybackSource::FollowingLikes,
                            data.following_likes_user_urn.clone(),
                        )
                    } else {
                        (
                            &data.following_tracks,
                            state.selected_following_track_row,
                            PlaybackSource::FollowingPublished,
                            data.following_tracks_user_urn.clone(),
                        )
                    };
                if let Some(track) = tracks.get(selected_idx) {
                    if !track.is_playable() {
                        return InputOutcome::Continue;
                    }
                    if state.playback_source != new_source {
                        state.playback_history.clear();
                        state.manual_queue.clear();
                    } else if let Some(queued) = queued_from_current(state, data) {
                        if !(queued.source == new_source && queued.index == selected_idx) {
                            state.playback_history.push(queued);
                        }
                    }
                    player.play(track.clone());
                    state.playback_source = new_source;
                    state.override_playing = None;
                    state.current_playing_index = Some(selected_idx);
                    data.playback_tracks = tracks.clone();
                    data.playback_playlist_uri = None;
                    data.playback_album_uri = None;
                    data.playback_following_user_urn = user_urn;
                    state.auto_queue = build_queue(
                        selected_idx,
                        &data.playback_tracks,
                        state.shuffle_enabled,
                    );
                    if new_source == PlaybackSource::FollowingLikes {
                        data.following_likes_state.select(Some(selected_idx));
                    } else {
                        data.following_tracks_state.select(Some(selected_idx));
                    }
                }
            }
        }
        _ => {}
    }

    InputOutcome::Continue
}
