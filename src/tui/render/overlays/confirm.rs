use ratatui::{
    Frame,
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use crate::theme;

use super::utils::centered_rect_fixed;

/// Centred Yes / No box. `selected` 0 = Yes, 1 = No. Used for quitting and for
/// every destructive playlist action.
pub fn render_confirm(frame: &mut Frame, message: &str, selected: usize) {
    let width = (message.chars().count() as u16 + 6).max(40).min(frame.area().width.max(1));
    let popup_area = centered_rect_fixed(width, 5, frame.area());
    crate::tui::render::widgets::clear(frame, popup_area);

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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn confirm_popup_does_not_panic_on_narrow_terminal() {
        let message = "x".repeat(60);
        for (w, h) in [(30, 8), (10, 4), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| render_confirm(frame, &message, 0)).unwrap();
        }
    }
}
