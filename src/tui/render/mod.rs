mod now_playing;
mod overlays;
mod tabs;
mod utils;
mod visualizer;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use crate::theme;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Tabs},
};
use ratatui_image::thread::ThreadProtocol;

use crate::player::Player;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::state::{AppData, AppState, TAB_TITLES};
use crate::tui::render::visualizer::render_visualizer;

pub fn render(
    frame: &mut Frame,
    state: &AppState,
    data: &mut AppData,
    views: &FilteredViews,
    player: &Player,
    cover_art_async: &mut ThreadProtocol,
    wave_buffer: &Arc<Mutex<VecDeque<f32>>>,
) {
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

    if state.visualizer_mode {
        render_visualizer(
            frame,
            frame.area(),
            wave_buffer,
            state.visualizer_view,
            &player.current_track(),
            state.progress,
            cover_art_async,
            &state.lyrics,
        );
        overlays::render_overlays(frame, state, data);
        return;
    }

    render_tabs(frame, chunks[0], &TAB_TITLES, state.selected_tab);

    if state.selected_tab == 0 {
        tabs::render_library(frame, chunks[1], width, state, data, views);
    } else if state.selected_tab == 1 {
        tabs::render_search(frame, chunks[1], width, state, data);
    } else {
        tabs::render_feed(frame, chunks[1], state, data);
    }

    now_playing::render_now_playing(frame, chunks[2], state, player, cover_art_async);

    overlays::render_overlays(frame, state, data);
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
        .style(Style::default().fg(theme::current().fg))
        .highlight_style(
            Style::default()
                .fg(theme::current().accent)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs_widget, area);
}
