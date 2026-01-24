use ratatui::{
    Frame,
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
};

use super::centered_rect;

pub fn render_quit_confirm(frame: &mut Frame, quit_confirm_selected: usize) {
    let popup_area = centered_rect(25, 10, frame.area());
    frame.render_widget(Clear, popup_area);

    let yes_style = if quit_confirm_selected == 0 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::LightBlue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let no_style = if quit_confirm_selected == 1 {
        Style::default()
            .fg(Color::Black)
            .bg(Color::LightBlue)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let line = Text::from(vec![
        ratatui::text::Line::from("Are you Sure you Want to Quit?"),
        ratatui::text::Line::from(""),
        ratatui::text::Line::from(vec![
            Span::styled("Yes", yes_style),
            Span::raw("   "),
            Span::styled("No", no_style),
        ]),
    ]);

    let box_widget = Paragraph::new(line)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded),
        );
    frame.render_widget(box_widget, popup_area);
}
