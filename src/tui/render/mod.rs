mod now_playing;
mod overlays;
mod tabs;
mod utils;
mod visualizer;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, TableState, Tabs},
};
use ratatui_image::thread::ThreadProtocol;

use crate::api::{Album, Artist, Playlist, Track};
use crate::tui::logic::state::QueuedTrack;
use crate::tui::render::visualizer::render_visualizer;

pub fn render(
    frame: &mut Frame,
    likes_view: &Vec<Track>,
    queue_tracks: &Vec<Track>,
    likes_state: &mut TableState,
    playlists: &Vec<Playlist>,
    playlists_state: &mut TableState,
    playlist_tracks: &Vec<Track>,
    playlist_tracks_state: &mut TableState,
    album_tracks: &Vec<Track>,
    album_tracks_state: &mut TableState,
    albums: &Vec<Album>,
    albums_state: &mut TableState,
    following: &Vec<Artist>,
    following_state: &mut TableState,
    following_tracks: &Vec<Track>,
    following_tracks_state: &mut TableState,
    following_likes_tracks: &Vec<Track>,
    following_likes_state: &mut TableState,
    search_tracks: &Vec<Track>,
    search_tracks_state: &mut TableState,
    search_playlists: &Vec<Playlist>,
    search_playlists_state: &mut TableState,
    search_playlist_tracks: &Vec<Track>,
    search_playlist_tracks_state: &mut TableState,
    search_albums: &Vec<Album>,
    search_albums_state: &mut TableState,
    search_album_tracks: &Vec<Track>,
    search_album_tracks_state: &mut TableState,
    search_people: &Vec<Artist>,
    search_people_state: &mut TableState,
    search_people_tracks: &Vec<Track>,
    search_people_tracks_state: &mut TableState,
    search_people_likes_tracks: &Vec<Track>,
    search_people_likes_state: &mut TableState,
    selected_tab: usize,
    tab_titles: &[&str],
    selected_subtab: usize,
    subtab_titles: &[&str],
    selected_row: usize,
    selected_playlist_track_row: usize,
    selected_album_track_row: usize,
    selected_following_track_row: usize,
    selected_following_like_row: usize,
    following_focus_is_likes: bool,
    query: &str,
    searchfilters: &[&str],
    selected_searchfilter: usize,
    search_selected_playlist_track_row: usize,
    search_selected_album_track_row: usize,
    search_selected_person_track_row: usize,
    search_selected_person_like_row: usize,
    search_people_focus_is_likes: bool,
    info_pane_selected: bool,
    selected_info_row: usize,
    data: &mut Vec<(f64, f64)>,
    window: &mut [f64; 2],
    progress: &mut u64,
    selected_track: Track,
    cover_art_async: &mut ThreadProtocol,
    current_volume: f32,
    shuffle_enabled: bool,
    repeat_enabled: bool,
    queue_visible: bool,
    manual_queue: &VecDeque<QueuedTrack>,
    auto_queue: &VecDeque<usize>,
    current_playing_track: Option<Track>,
    previous_playing_track: Option<Track>,
    help_visible: bool,
    quit_confirm_visible: bool,
    quit_confirm_selected: usize,
    search_popup_visible: bool,
    search_query: &str,
    search_match_count: usize,
    visualizer_mode: bool,
    wave_buffer: &Arc<Mutex<VecDeque<f32>>>,
    visualizer_view: crate::tui::logic::state::VisualizerMode,
) {
    let _ = search_match_count;
    let width = frame.area().width as usize;

    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(7),
            ]
            .as_ref(),
        )
        .split(frame.area());

    if visualizer_mode {
        render_visualizer(frame, frame.area(), wave_buffer, visualizer_view);
        overlays::render_overlays(
            frame,
            queue_tracks,
            manual_queue,
            auto_queue,
            current_playing_track,
            previous_playing_track,
            queue_visible,
            help_visible,
            quit_confirm_visible,
            quit_confirm_selected,
        );
        return;
    }

    render_tabs(frame, chunks[0], tab_titles, selected_tab);

    if selected_tab == 0 {
        tabs::render_library(
            frame,
            chunks[1],
            width,
            likes_view,
            likes_state,
            playlists,
            playlists_state,
            playlist_tracks,
            playlist_tracks_state,
            album_tracks,
            album_tracks_state,
            albums,
            albums_state,
            following,
            following_state,
            following_tracks,
            following_tracks_state,
            following_likes_tracks,
            following_likes_state,
            selected_subtab,
            subtab_titles,
            selected_row,
            selected_playlist_track_row,
            selected_album_track_row,
            selected_following_track_row,
            selected_following_like_row,
            following_focus_is_likes,
            search_popup_visible,
            search_query,
        );
    } else if selected_tab == 1 {
        tabs::render_search(
            frame,
            chunks[1],
            width,
            query,
            searchfilters,
            selected_searchfilter,
            selected_row,
            search_tracks,
            search_tracks_state,
            search_playlists,
            search_playlists_state,
            search_playlist_tracks,
            search_playlist_tracks_state,
            search_albums,
            search_albums_state,
            search_album_tracks,
            search_album_tracks_state,
            search_people,
            search_people_state,
            search_people_tracks,
            search_people_tracks_state,
            search_people_likes_tracks,
            search_people_likes_state,
            search_selected_playlist_track_row,
            search_selected_album_track_row,
            search_selected_person_track_row,
            search_selected_person_like_row,
            search_people_focus_is_likes,
        );
    } else {
        tabs::render_feed(
            frame,
            chunks[1],
            width,
            selected_row,
            selected_info_row,
            info_pane_selected,
        );
    }

    now_playing::render_now_playing(
        frame,
        chunks[2],
        data,
        window,
        progress,
        selected_track,
        cover_art_async,
        current_volume,
        shuffle_enabled,
        repeat_enabled,
    );

    overlays::render_overlays(
        frame,
        queue_tracks,
        manual_queue,
        auto_queue,
        current_playing_track,
        previous_playing_track,
        queue_visible,
        help_visible,
        quit_confirm_visible,
        quit_confirm_selected,
    );
}

fn render_tabs(frame: &mut Frame, area: ratatui::layout::Rect, tab_titles: &[&str], selected: usize) {
    let tabs: Vec<_> = tab_titles.iter().map(|t| Span::raw(*t)).collect();
    let tabs_widget = Tabs::new(tabs)
        .block(
            Block::default()
                .title(Span::styled(
                    "sctui",
                    Style::default().add_modifier(Modifier::BOLD),
                ))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs_widget, area);
}
