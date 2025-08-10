use color_eyre::eyre::{Ok, Result};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Alignment, Constraint, Flex, Layout},
    prelude::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub fn run() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = start(terminal);
    ratatui::restore();
    result
}

fn start(mut terminal: DefaultTerminal) -> Result<()> {
    loop {
        terminal.draw(|frame| render(frame))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => break,
                _ => {}
            }
        }
    }
    Ok(())
}

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}

fn render(frame: &mut Frame) {
    let title = Span::styled("sctui", Style::default().add_modifier(Modifier::BOLD));

    let border = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);

    frame.render_widget(border, frame.area());
}
