use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState},
};

use crate::api::{Playlist, Track};
use crate::tui::render::utils::{styled_header, truncate_with_ellipsis};

use super::utils::centered_rect;

/// "Add to playlist" picker. Row 0 is `+ New playlist…`, the rest are the
/// user's own playlists. When `new_title` is `Some`, the title field is being
/// typed and is shown instead of the list.
pub fn render_playlist_picker(
    frame: &mut Frame,
    track: &Track,
    owned: &[&Playlist],
    selected: usize,
    new_title: Option<&str>,
) {
    let popup_area = centered_rect(60, 60, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(format!(" Add \"{}\" to… ", truncate_with_ellipsis(&track.title, 40)))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded);

    if let Some(title) = new_title {
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        let [_, field, hint] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .areas(inner);
        frame.render_widget(
            Paragraph::new(format!("New playlist name: {title}▏"))
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            field,
        );
        frame.render_widget(
            Paragraph::new("Enter to create (private)  ·  Esc to go back")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
            hint,
        );
        return;
    }

    let total_width = popup_area.width as usize;
    let title_width = (total_width * 75) / 100;

    let mut rows = vec![
        Row::new(vec!["+ New playlist…".to_string(), String::new()])
            .style(Style::default().fg(Color::Cyan)),
    ];
    rows.extend(owned.iter().map(|p| {
        Row::new(vec![
            truncate_with_ellipsis(&p.title, title_width),
            format!("{} tracks", p.track_count),
        ])
    }));

    let table = Table::new(rows, vec![Constraint::Percentage(75), Constraint::Percentage(25)])
        .header(styled_header(&["Playlist", "Size"]))
        .block(block)
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(Color::LightBlue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

    let mut table_state = TableState::default().with_selected(Some(selected.min(owned.len())));
    frame.render_stateful_widget(table, popup_area, &mut table_state);
}
