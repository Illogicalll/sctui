use ratatui::{
    style::{Modifier, Style},
    widgets::{Cell, Row},
};
use crate::theme;

pub fn styled_header(cells: &[&str]) -> Row<'static> {
    let style = Style::default()
        .fg(theme::current().secondary)
        .add_modifier(Modifier::BOLD);
    let cells: Vec<Cell> = cells
        .iter()
        .map(|&text| Cell::from(text.to_string()).style(style))
        .collect();
    Row::new(cells)
}

pub fn truncate_with_ellipsis(s: &str, min_width: usize) -> String {
    if s.chars().count() > min_width && min_width > 3 {
        let truncated: String = s.chars().take(min_width - 3).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}
