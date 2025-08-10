use color_eyre::eyre::{Ok, Result};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Paragraph, Tabs},
};

pub fn run() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = start(terminal);
    ratatui::restore();
    result
}

fn start(mut terminal: DefaultTerminal) -> Result<()> {
    let tab_titles = ["Library", "Search", "Feed"];
    let mut selected_tab = 0;

    loop {
        terminal.draw(|frame| render(frame, selected_tab, &tab_titles))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Tab => {
                    selected_tab = (selected_tab + 1) % tab_titles.len();
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn render(frame: &mut Frame, selected_tab: usize, tab_titles: &[&str]) {
    let chunks = Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(7),
            ]
            .as_ref(),
        )
        .split(frame.area());

    let tabs: Vec<_> = tab_titles.iter().map(|t| Span::raw(*t)).collect();

    let tabs_widget = Tabs::new(tabs)
        .block(
            Block::default()
                .title(Span::styled(
                    "sctui",
                    Style::default().add_modifier(Modifier::BOLD),
                ))
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .select(selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    frame.render_widget(tabs_widget, chunks[0]);

    let content = match selected_tab {
        0 => "Content of Tab 1",
        1 => "Content of Tab 2",
        2 => "Content of Tab 3",
        _ => "",
    };

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Tab content")
                .border_type(BorderType::Rounded),
        )
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, chunks[1]);

    let now_playing = Paragraph::new("Now Playing Area")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .alignment(Alignment::Center);

    frame.render_widget(now_playing, chunks[2]);
}
