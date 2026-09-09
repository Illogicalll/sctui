use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState},
};
use crate::theme;

use crate::config::Settings;
use crate::keymap::Action;
use crate::tui::logic::state::AppState;
use crate::tui::render::utils::styled_header;

use super::utils::centered_rect;

/// Key reference and editor, generated from the live keymap. Tab swaps it for the
/// settings page.
pub fn render_help(frame: &mut Frame, state: &AppState) {
    if state.help_settings {
        let mut settings = state.settings;
        let rows: Vec<Row> = Settings::ROWS
            .iter()
            .map(|(label, field)| {
                let on = *field(&mut settings);
                let row = Row::new(vec![label.to_string(), if on { "on" } else { "off" }.to_string()]);
                if on {
                    row
                } else {
                    row.style(Style::default().fg(theme::current().dim))
                }
            })
            .collect();
        render_page(
            frame,
            state,
            " Settings ",
            &["Setting", "State"],
            rows,
            state.help_settings_selected,
            "Enter/Space: toggle   ↑↓: move   Tab: keys   Esc: close   ·  hidden tracks come back after a restart",
        );
        return;
    }

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
    render_page(
        frame,
        state,
        " Keys ",
        &["Action", "Keys"],
        rows,
        state.help_selected,
        "Enter: add key   Backspace: unbind   r: default   ↑↓: move   Tab: settings   Esc: close   ·  saved to ~/.config/sctui/config.toml",
    );
}

/// One page of the popup: a two-column table over the shared status and hint lines.
fn render_page(
    frame: &mut Frame,
    state: &AppState,
    title: &str,
    header: &[&str],
    rows: Vec<Row>,
    selected: usize,
    hint: &str,
) {
    let popup_area = centered_rect(76, 80, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(title)
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

    let table = Table::new(rows, vec![Constraint::Percentage(62), Constraint::Percentage(38)])
        .header(styled_header(header))
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(theme::current().selection_bg)
                .fg(theme::current().fg)
                .add_modifier(Modifier::BOLD),
        );
    let mut table_state = TableState::default().with_selected(Some(selected));
    frame.render_stateful_widget(table, table_area, &mut table_state);

    let (status, status_style) = match (&state.help_capture, &state.help_message) {
        (Some(_), Some(msg)) => (msg.clone(), Style::default().fg(theme::current().warning).add_modifier(Modifier::BOLD)),
        (_, Some(msg)) => (msg.clone(), Style::default().fg(theme::current().accent)),
        _ => (String::new(), Style::default()),
    };
    frame.render_widget(Paragraph::new(status).style(status_style).alignment(Alignment::Center), status_area);
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(theme::current().dim))
            .alignment(Alignment::Center),
        hint_area,
    );
}
