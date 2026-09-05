use ratatui::{
    Frame,
    layout::{Alignment, Constraint},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Row, Table, TableState},
};

use crate::tui::logic::state::QueuedTrack;
use crate::tui::render::utils::{styled_header, truncate_with_ellipsis};

use super::utils::centered_rect;

/// Listening history for this session, newest first. `selected` indexes the
/// displayed (newest-first) order.
pub fn render_history(frame: &mut Frame, history: &[QueuedTrack], selected: usize) {
    let popup_area = centered_rect(70, 60, frame.area());
    frame.render_widget(Clear, popup_area);

    let total_width = popup_area.width as usize;
    let title_width = (total_width * 65) / 100;
    let artist_width = (total_width * 25) / 100;

    let rows: Vec<Row> = if history.is_empty() {
        vec![Row::new(vec!["No history yet", "", ""]).style(Style::default().fg(Color::DarkGray))]
    } else {
        history
            .iter()
            .rev()
            .map(|queued| {
                let track = &queued.track;
                let mut row = Row::new(vec![
                    truncate_with_ellipsis(&track.title, title_width),
                    truncate_with_ellipsis(&track.artists, artist_width),
                    track.duration.clone(),
                ]);
                if !track.is_playable() {
                    row = row.style(Style::default().fg(Color::DarkGray));
                }
                row
            })
            .collect()
    };

    let table = Table::new(
        rows,
        vec![
            Constraint::Percentage(65),
            Constraint::Percentage(25),
            Constraint::Percentage(10),
        ],
    )
    .header(styled_header(&["Title", "Artist", "Duration"]))
    .block(
        Block::default()
            .title("History")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded),
    )
    .column_spacing(1)
    .row_highlight_style(
        Style::default()
            .bg(Color::LightBlue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

    let mut table_state = TableState::default().with_selected(if history.is_empty() {
        None
    } else {
        Some(selected.min(history.len() - 1))
    });
    frame.render_stateful_widget(table, popup_area, &mut table_state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Track;
    use crate::tui::logic::state::PlaybackSource;
    use ratatui::{Terminal, backend::TestBackend};

    fn entry(title: &str) -> QueuedTrack {
        QueuedTrack::new(
            PlaybackSource::Likes,
            0,
            &Track {
                title: title.to_string(),
                artists: "Artist".into(),
                duration: "3:00".into(),
                duration_ms: 180_000,
                playback_count: String::new(),
                artwork_url: String::new(),
                access: "playable".into(),
                track_urn: format!("urn:{title}"),
            },
            None,
            None,
            None,
            None,
            false,
        )
    }

    fn screen(history: &[QueuedTrack], selected: usize) -> String {
        let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
        terminal
            .draw(|f| render_history(f, history, selected))
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..16)
            .map(|y| (0..60).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn newest_entry_is_listed_first() {
        let history = vec![entry("older"), entry("newer")];
        let s = screen(&history, 0);
        assert!(s.find("newer").unwrap() < s.find("older").unwrap());
    }

    #[test]
    fn empty_history_has_placeholder_and_no_panic_on_selection() {
        let s = screen(&[], 5);
        assert!(s.contains("No history yet"));
    }
}
