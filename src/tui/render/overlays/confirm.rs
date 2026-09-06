use ratatui::{
    Frame,
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
};
use crate::theme;

use super::utils::centered_rect_fixed;

/// Centred Yes / No box. `selected` 0 = Yes, 1 = No. Used for quitting and for
/// every destructive playlist action.
pub fn render_confirm(frame: &mut Frame, message: &str, selected: usize) {
    let width = (message.chars().count() as u16 + 6).clamp(40, frame.area().width.max(1));
    let popup_area = centered_rect_fixed(width, 5, frame.area());
    frame.render_widget(Clear, popup_area);

    let highlight = Style::default()
        .fg(theme::current().selection_fg)
        .bg(theme::current().selection_bg)
        .add_modifier(Modifier::BOLD);
    let plain = Style::default().fg(theme::current().fg);
    let (yes_style, no_style) = if selected == 0 {
        (highlight, plain)
    } else {
        (plain, highlight)
    };

    let text = Text::from(vec![
        Line::from(message.to_string()),
        Line::from(""),
        Line::from(vec![
            Span::styled("Yes", yes_style),
            Span::raw("   "),
            Span::styled("No", no_style),
        ]),
    ]);

    frame.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded),
        ),
        popup_area,
    );
}
