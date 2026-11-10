use std::collections::VecDeque;
use crate::theme;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Row, Table},
};

use crate::api::Track;
use crate::tui::logic::state::{AppData, AppState, PlaybackSource, QueuedTrack};
use crate::tui::logic::utils::active_tracks;
use crate::tui::render::utils::{styled_header, truncate_with_ellipsis};
use crate::tui::render::widgets::{fit, put};

use super::utils::centered_rect;

/// The tracks still to play, in order: the manual queue, then the rest of what is
/// playing. `true` marks a track the user queued.
pub fn up_next<'a>(
    queue_tracks: &'a [Track],
    manual_queue: &'a VecDeque<QueuedTrack>,
    auto_queue: &'a VecDeque<usize>,
) -> impl Iterator<Item = (&'a Track, bool)> {
    manual_queue
        .iter()
        .map(|queued| (&queued.track, queued.user_added))
        .chain(auto_queue.iter().filter_map(|idx| queue_tracks.get(*idx)).map(|track| (track, false)))
}

/// What the playing list is, as the queue's `FROM …` heading names it.
pub fn source_name(state: &AppState) -> &'static str {
    match state.playback_source {
        PlaybackSource::Likes | PlaybackSource::FollowingLikes => "LIKES",
        PlaybackSource::Playlist => "PLAYLIST",
        PlaybackSource::Album => "ALBUM",
        PlaybackSource::FollowingPublished => "UPLOADS",
        PlaybackSource::Feed => "FEED",
        PlaybackSource::Radio => "STATION",
    }
}

/// The queue as a plain numbered list filling `area`, for a layout that shows it
/// beside the panes (Sidebar's dock, the Player panel): the user's own tracks under
/// `QUEUED BY YOU`, the rest under what they come from. Display only.
pub fn draw_up_next(frame: &mut Frame, area: Rect, state: &AppState, data: &AppData) {
    let t = theme::current();
    let area = area.intersection(frame.area());
    let mut y = area.y;
    let mut line = |frame: &mut Frame, text: String, style: Style| {
        if y < area.bottom() {
            put(frame, Rect { y, height: 1, ..area }, Line::styled(fit(&text, area.width as usize), style));
            y += 1;
        }
    };
    let heading = Style::new().fg(t.secondary).add_modifier(Modifier::BOLD);
    let source = source_name(state);
    let mut section = None;
    let mut n = 0;
    let upcoming = up_next(active_tracks(state, data), &state.manual_queue, &state.auto_queue);
    for (track, by_user) in upcoming.take(area.height as usize) {
        if section != Some(by_user) {
            section = Some(by_user);
            let shuffled = if state.shuffle_enabled && !by_user { " · SHUFFLED" } else { "" };
            line(frame, if by_user { "QUEUED BY YOU".into() } else { format!("FROM {source}{shuffled}") }, heading);
        }
        n += 1;
        let color = if !track.is_playable() { t.dim } else if by_user { t.fg } else { t.muted };
        line(frame, format!("{n:<3}{}", track.title), Style::new().fg(color));
    }
    if n == 0 {
        line(frame, "queue is empty".into(), Style::new().fg(t.dim));
    }
}

pub fn render_queue(
    frame: &mut Frame,
    queue_tracks: &[Track],
    manual_queue: &VecDeque<QueuedTrack>,
    auto_queue: &VecDeque<usize>,
    current_playing_track: Option<Track>,
    previous_playing_track: Option<Track>,
    scroll: usize,
) {
    let popup_area = centered_rect(70, 60, frame.area());
    crate::tui::render::widgets::clear(frame, popup_area);

    let total_width = popup_area.width as usize;
    let title_width = (total_width * 65) / 100;
    let artist_width = (total_width * 25) / 100;

    let mut rows: Vec<Row> = Vec::new();

    if let Some(track) = previous_playing_track {
        rows.push(
            Row::new(vec![
                truncate_with_ellipsis(&track.title, title_width),
                truncate_with_ellipsis(&track.artists, artist_width),
                track.duration.clone(),
            ])
            .style(Style::default().fg(theme::current().dim)),
        );
    } else {
        rows.push(
            Row::new(vec!["Previous: None", "", ""]).style(Style::default().fg(theme::current().dim)),
        );
    }

    if let Some(track) = current_playing_track {
        rows.push(
            Row::new(vec![
                truncate_with_ellipsis(&track.title, title_width),
                truncate_with_ellipsis(&track.artists, artist_width),
                track.duration.clone(),
            ])
            .style(Style::default().bg(theme::current().selection_bg).fg(theme::current().fg)),
        );
    } else {
        rows.push(Row::new(vec!["Now Playing: None", "", ""]));
    }

    // Upcoming tracks, manual queue first; `scroll` of them are off the top.
    let max_rows = popup_area.height.saturating_sub(3) as usize;
    let upcoming = up_next(queue_tracks, manual_queue, auto_queue).map(|(track, _)| track);
    let scroll = scroll.min((manual_queue.len() + auto_queue.len()).saturating_sub(1));
    for track in upcoming.skip(scroll).take(max_rows.saturating_sub(rows.len())) {
        let mut row = Row::new(vec![
            truncate_with_ellipsis(&track.title, title_width),
            truncate_with_ellipsis(&track.artists, artist_width),
            track.duration.clone(),
        ]);
        if !track.is_playable() {
            row = row.style(Style::default().fg(theme::current().dim));
        }
        rows.push(row);
    }

    if rows.len() <= 2 {
        rows.push(Row::new(vec!["Queue is empty", "", ""]));
    }

    let header = styled_header(&["Title", "Artist", "Duration"]);
    let table = Table::new(
        rows,
        vec![
            Constraint::Percentage(65),
            Constraint::Percentage(25),
            Constraint::Percentage(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title("Queue")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded),
    )
    .column_spacing(1);
    frame.render_widget(table, popup_area);
}
