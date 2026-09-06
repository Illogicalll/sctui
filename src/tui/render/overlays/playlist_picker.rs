use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState},
};

use crate::api::{Playlist, Track};
use crate::tui::render::utils::{styled_header, truncate_with_ellipsis};

use super::utils::centered_rect_fixed;

const PROMPT_WIDTH: u16 = 52;
const LIST_WIDTH: u16 = 56;
const MAX_LIST_ROWS: u16 = 15;

/// "Add to playlist" picker. Row 0 is `+ New playlist…`, the rest are the
/// user's own playlists. When `new_title` is `Some`, a compact prompt for the
/// name and visibility is shown instead of the list.
pub fn render_playlist_picker(
    frame: &mut Frame,
    track: Option<&Track>,
    owned: &[&Playlist],
    selected: usize,
    new_title: Option<&str>,
    public: bool,
) {
    let heading = match track {
        Some(track) => format!(" Add \"{}\" to… ", truncate_with_ellipsis(&track.title, 36)),
        None => " New playlist ".to_string(),
    };
    let block = Block::default()
        .title(heading)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded);

    if let Some(title) = new_title {
        let popup_area = centered_rect_fixed(PROMPT_WIDTH, 7, frame.area());
        frame.render_widget(Clear, popup_area);
        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);
        let [name, _, visibility, _, hint] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        let field_width = inner.width.saturating_sub(8) as usize;
        let shown: String = title.chars().rev().take(field_width).collect::<Vec<_>>().into_iter().rev().collect();
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" Name: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{shown}▏"), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ])),
            name,
        );

        let on = Style::default().fg(Color::Black).bg(Color::LightBlue).add_modifier(Modifier::BOLD);
        let off = Style::default().fg(Color::White);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" Visibility: ", Style::default().fg(Color::DarkGray)),
                Span::styled(" Private ", if public { off } else { on }),
                Span::raw("  "),
                Span::styled(" Public ", if public { on } else { off }),
            ])),
            visibility,
        );
        frame.render_widget(
            Paragraph::new("Enter create  ·  ←/→ visibility  ·  Esc cancel")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
            hint,
        );
        return;
    }

    let row_count = owned.len() as u16 + 1;
    let height = (row_count + 3).min(MAX_LIST_ROWS + 3); // header + borders
    let popup_area = centered_rect_fixed(LIST_WIDTH, height, frame.area());
    frame.render_widget(Clear, popup_area);

    let title_width = (popup_area.width as usize * 70) / 100;
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

    let table = Table::new(rows, vec![Constraint::Percentage(70), Constraint::Percentage(30)])
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
