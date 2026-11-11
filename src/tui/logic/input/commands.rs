use ratatui::crossterm::event::KeyCode;

use crate::config::Layout;
use crate::keymap::Action;

use super::InputOutcome;
use crate::api::{Album, Artist, Playlist, Track};
use crate::tui::logic::filtering::build_filtered_views;
use crate::tui::logic::nav::{self, PaneId};
use crate::tui::logic::state::{AppData, AppState, ConfirmAction, Engagement, HelpPage};
use crate::tui::logic::utils::rebuild_auto_queue;
use crate::tui::logic::utils::queued_from_current;
use crate::tui::logic::utils::{soundcloud_id_from_urn, soundcloud_playlist_id_from_tracks_uri};

use super::helpers::list_step;
use super::queue::{handle_add_to_queue, handle_add_next_to_queue, selected_or_toast};

/// Commands that are plain state changes; movement, playback and tab actions
/// are dispatched in `input::run_action`.
pub(crate) fn run_command(action: Action, state: &mut AppState, data: &mut AppData) -> InputOutcome {
    match action {
        Action::ToggleShuffle => {
            state.shuffle_enabled = !state.shuffle_enabled;
            rebuild_auto_queue(state, data);
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
        Action::OpenInBrowser => {
            let url = selected_permalink(state, data);
            open_on_soundcloud(url, state);
        }
        Action::OpenInBrowserPlaying => {
            let url = queued_from_current(state, data).map(|q| q.track.permalink_url);
            open_on_soundcloud(url, state);
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
        Action::PanelPage => state.panel_page = state.panel_page.next(),
        Action::ToggleHistory => {
            state.history_visible = !state.history_visible;
            state.history_selected = 0;
        }
        Action::ToggleQueue => {
            state.queue_visible = !state.queue_visible;
            state.queue_scroll = 0;
            if state.queue_visible && state.auto_queue.is_empty() {
                rebuild_auto_queue(state, data);
            }
        }
        _ => {}
    }
    InputOutcome::Continue
}

/// An action while the navigation column has focus. Movement walks its entries and
/// changes location as it goes; `]` or Enter steps into the panes; delete and like act
/// on the playlist entry under the cursor; the actions that need a track do nothing.
/// Returns false for everything else (tabs, sub-tabs, playback, popups), which works
/// as it does anywhere.
pub(crate) fn run_nav_command(action: Action, state: &mut AppState, data: &mut AppData) -> bool {
    match action {
        Action::PaneLeft
        | Action::StartStation
        | Action::AddToQueue
        | Action::PlayNext
        | Action::AddToPlaylist
        | Action::RemoveFromPlaylist => {}
        Action::PaneRight | Action::PlaySelected => nav::leave_nav(state),
        Action::Up
        | Action::Down
        | Action::PageUp
        | Action::PageDown
        | Action::HalfPageUp
        | Action::HalfPageDown
        | Action::Top
        | Action::Bottom => {
            let items = nav::nav_items(state, data, state.settings.layout == Layout::Sidebar);
            let at = nav::nav_index(state, data, &items);
            // `list_step` only looks at the key when there is no action.
            let key = KeyCode::Null.into();
            if let Some(to) = list_step(&key, Some(action), at, items.len() - 1) {
                nav::nav_go(state, data, &items[to]);
                // Two rows can share a uri (a playlist both owned and liked): the row
                // moved onto is the one selected, whichever `nav_go` found.
                if let Some(first) = items.iter().position(|i| i.playlist_uri.is_some())
                    && items[to].playlist_uri.is_some()
                {
                    *nav::cursor_mut(state, PaneId::Playlists) = to - first;
                }
            }
        }
        Action::DeletePlaylist => {
            if let Some(playlist) = nav::nav_playlist(state, data).filter(|p| p.is_owned) {
                state.confirm = Some(ConfirmAction::DeletePlaylist { tracks_uri: playlist.tracks_uri.clone() });
                state.confirm_selected = 1;
            }
        }
        Action::ToggleLike => toggle_playlist_like(nav::nav_playlist(state, data).cloned(), state, data),
        Action::OpenInBrowser => {
            let url = nav::nav_playlist(state, data).map(|p| p.permalink_url.clone());
            open_on_soundcloud(url, state);
        }
        // The filter belongs to a pane, which takes the focus.
        Action::Search => {
            state.nav_focused = false;
            return false;
        }
        _ => return false,
    }
    true
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

/// The soundcloud.com page of the focused pane's selected row: the track in a tracks
/// pane, the playlist, album, person or feed item in a container pane. `None` when
/// the pane is empty; an empty string when the row came without a permalink.
fn selected_permalink(state: &AppState, data: &AppData) -> Option<String> {
    let id = nav::focused(state);
    let idx = nav::selected_index(state, id)?;
    match id {
        PaneId::Playlists => data.playlists.get(idx).map(|p| p.permalink_url.clone()),
        PaneId::SearchPlaylists => data.search_playlists.get(idx).map(|p| p.permalink_url.clone()),
        PaneId::Albums => data.albums.get(idx).map(|a| a.permalink_url.clone()),
        PaneId::SearchAlbums => data.search_albums.get(idx).map(|a| a.permalink_url.clone()),
        PaneId::Following => data.following.get(idx).map(|a| a.permalink_url.clone()),
        PaneId::SearchPeople => data.search_people.get(idx).map(|a| a.permalink_url.clone()),
        PaneId::Feed => data.feed.get(idx).map(|a| a.permalink_url.clone()),
        _ => {
            let views = build_filtered_views(state, data);
            nav::selected_track(state, data, &views).map(|t| t.permalink_url)
        }
    }
}

/// Open a row's soundcloud.com page in the browser and say so, or say why not.
fn open_on_soundcloud(url: Option<String>, state: &mut AppState) {
    let Some(url) = url else { return state.toast_err("Nothing to open") };
    // The link is whatever the API sent: only a soundcloud.com page goes to the OS.
    if !url.starts_with("https://soundcloud.com/") {
        return state.toast_err("No SoundCloud link for this item");
    }
    // Tests drive the real handlers; none of them may open a browser.
    match if cfg!(test) { Ok(()) } else { webbrowser::open(&url) } {
        Ok(()) => state.toast(format!("Opened in browser · {url}")),
        Err(e) => state.toast_err(format!("Could not open the browser: {e}")),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::logic::state::PlaybackSource;
    use crate::tui::logic::toast::ToastKind;
    use crate::tui::render::testkit::{fixture, fixture_empty, go};

    /// #54: `b` opens the focused pane's selected row on soundcloud.com, whatever kind
    /// of row it is, and `B` the playing track. Under test nothing is launched; the
    /// toast names the link that would be.
    #[test]
    fn open_in_browser_opens_the_selection_or_the_playing_track() {
        let (mut state, mut data, ..) = fixture();
        let said = |state: &mut AppState, data: &mut AppData, action| {
            state.toasts.clear();
            run_command(action, state, data);
            let toast = state.toasts.last().expect("a toast either way");
            (toast.msg.clone(), toast.kind)
        };
        let opened = |url: &str| (format!("Opened in browser · {url}"), ToastKind::Ok);
        // (tab, sub-tab, focused pane) and the page its selected row links to.
        for (tab, sub, focus, url) in [
            (0, 0, 0, "https://soundcloud.com/artist-0/track-0"),
            (0, 1, 0, "https://soundcloud.com/me/sets/playlist-0"),
            (0, 1, 1, "https://soundcloud.com/artist-2/track-100"),
            (0, 2, 0, "https://soundcloud.com/artist-0/sets/album-0"),
            (0, 3, 0, "https://soundcloud.com/person-0"),
            (1, 3, 0, "https://soundcloud.com/person-0"),
            (2, 0, 0, "https://soundcloud.com/poster-0/post-0"),
        ] {
            go(&mut state, &mut data, tab, sub, focus);
            assert_eq!(said(&mut state, &mut data, Action::OpenInBrowser), opened(url), "{tab} {sub} {focus}");
        }
        // The playing track (Song 04), wherever the cursor is.
        assert_eq!(said(&mut state, &mut data, Action::OpenInBrowserPlaying), opened("https://soundcloud.com/artist-4/track-4"));

        // A row that came without a permalink, or with something that is not a
        // soundcloud.com page, is never handed to the browser.
        go(&mut state, &mut data, 0, 0, 0);
        for bad in ["", "file:///etc/passwd", "https://soundcloud.com.evil.example/x"] {
            data.likes[0].permalink_url = bad.to_string();
            assert_eq!(said(&mut state, &mut data, Action::OpenInBrowser), ("No SoundCloud link for this item".to_string(), ToastKind::Err));
        }
        // The navigation column: the playlist under its cursor, or nothing.
        state.settings.layout = Layout::Sidebar;
        go(&mut state, &mut data, 0, 1, 0);
        state.nav_focused = true;
        *nav::cursor_mut(&mut state, PaneId::Playlists) = 2;
        state.toasts.clear();
        assert!(run_nav_command(Action::OpenInBrowser, &mut state, &mut data));
        assert_eq!(state.toasts[0].msg, "Opened in browser · https://soundcloud.com/me/sets/playlist-2");
        state.nav_on_header = true;
        assert!(run_nav_command(Action::OpenInBrowser, &mut state, &mut data));
        assert_eq!(state.toasts[1].msg, "Nothing to open");

        // Nothing selected and nothing playing.
        let (mut state, mut data, ..) = fixture_empty();
        for action in [Action::OpenInBrowser, Action::OpenInBrowserPlaying] {
            assert_eq!(said(&mut state, &mut data, action), ("Nothing to open".to_string(), ToastKind::Err));
        }
    }

    /// T17: toggling shuffle in a station must not queue the tracks already heard.
    #[test]
    fn toggling_shuffle_in_a_station_does_not_requeue_heard_tracks() {
        let track = |i: usize| Track {
            title: String::new(),
            artists: String::new(),
            duration: String::new(),
            duration_ms: 0,
            playback_count: String::new(),
            artwork_url: String::new(),
            access: String::new(),
            track_urn: i.to_string(),
            permalink_url: String::new(),
        };
        let mut data = AppData { playback_tracks: (0..6).map(track).collect(), ..Default::default() };
        let mut state = AppState {
            playback_source: PlaybackSource::Radio,
            current_playing_index: Some(4),
            auto_queue: [5].into(),
            ..Default::default()
        };
        run_command(Action::ToggleShuffle, &mut state, &mut data);
        assert!(state.shuffle_enabled);
        assert_eq!(state.auto_queue, [5], "only the track not yet heard");
        run_command(Action::ToggleShuffle, &mut state, &mut data);
        assert_eq!(state.auto_queue, [5]);
    }
}
