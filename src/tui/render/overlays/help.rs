use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState},
};
use crate::theme;

use crate::keymap::Action;
use crate::tui::logic::state::AppState;
use crate::tui::render::utils::styled_header;

use super::utils::centered_rect;

/// Key reference and editor, generated from the live keymap.
pub fn render_help(frame: &mut Frame, state: &AppState) {
    let popup_area = centered_rect(76, 80, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Keys ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded);
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    let [table_area, status_area, hint_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let rows: Vec<Row> = Action::ALL
        .iter()
        .map(|action| {
            let keys = state.keymap.labels(*action);
            let mut row = Row::new(vec![action.description().to_string(), keys.clone()]);
            if keys == "unbound" {
                row = row.style(Style::default().fg(theme::current().dim));
            }
            row
        })
        .collect();

    let table = Table::new(rows, vec![Constraint::Percentage(62), Constraint::Percentage(38)])
        .header(styled_header(&["Action", "Keys"]))
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(theme::current().selection_bg)
                .fg(theme::current().fg)
                .add_modifier(Modifier::BOLD),
        );
    let mut table_state = TableState::default().with_selected(Some(state.help_selected));
    frame.render_stateful_widget(table, table_area, &mut table_state);

    let (status, status_style) = match (&state.help_capture, &state.help_message) {
        (Some(_), Some(msg)) => (msg.clone(), Style::default().fg(theme::current().warning).add_modifier(Modifier::BOLD)),
        (_, Some(msg)) => (msg.clone(), Style::default().fg(theme::current().accent)),
        _ => (String::new(), Style::default()),
    };
    frame.render_widget(Paragraph::new(status).style(status_style).alignment(Alignment::Center), status_area);
    frame.render_widget(
        Paragraph::new("Enter: add key   Backspace: unbind   r: default   ↑↓: move   Esc: close   ·  saved to ~/.config/sctui/config.toml")
            .style(Style::default().fg(theme::current().dim))
            .alignment(Alignment::Center),
        hint_area,
    );
}
