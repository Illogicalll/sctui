use ratatui::{
    Frame,
    layout::{Alignment, Constraint},
    widgets::{Block, Borders, Clear, Row, Table},
};

use crate::keymap::{Action, Keymap};
use crate::tui::render::utils::styled_header;

use super::utils::centered_rect;

/// Generated from the live keymap, so it always shows the user's own bindings.
pub fn render_help(frame: &mut Frame, keymap: &Keymap) {
    let popup_area = centered_rect(70, 80, frame.area());
    frame.render_widget(Clear, popup_area);

    let mut rows: Vec<Row> = Action::ALL
        .iter()
        .map(|action| Row::new(vec![keymap.labels(*action), action.description().to_string()]))
        .collect();
    rows.push(Row::new(vec![
        "Esc / Enter (while typing)".to_string(),
        "Leave the search field".to_string(),
    ]));
    rows.push(Row::new(vec![
        "config".to_string(),
        "~/.config/sctui/config.toml  (sctui --dump-config for a template)".to_string(),
    ]));

    let max_rows = popup_area.height.saturating_sub(3) as usize;
    if rows.len() > max_rows {
        rows.truncate(max_rows);
    }

    let table = Table::new(
        rows,
        vec![Constraint::Percentage(32), Constraint::Percentage(68)],
    )
    .header(styled_header(&["Keys", "Action"]))
    .block(
        Block::default()
            .title("Help")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded),
    )
    .column_spacing(1);

    frame.render_widget(table, popup_area);
}
