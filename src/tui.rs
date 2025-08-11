use color_eyre::eyre::{Ok, Result};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Tabs},
};

// prep terminal and color_eyre error reporting
pub fn run() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = start(terminal);
    ratatui::restore();
    result
}

// state management and render loop
fn start(mut terminal: DefaultTerminal) -> Result<()> {
    let tab_titles = ["Library", "Search", "Feed"];
    let mut selected_tab = 0;
    let mut selected_subtab = 0;
    let mut selected_row = 0;
    let subtab_titles = [
        "Likes",
        "Playlists",
        "Albums",
        "Stations",
        "Following",
        "History",
    ];

    loop {
        terminal.draw(|frame| {
            render(
                frame,
                selected_tab,
                &tab_titles,
                selected_subtab,
                &subtab_titles,
                selected_row,
            )
        })?;

        // support for both arrow keys and hjkl
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Tab | KeyCode::Char('L') => {
                    selected_tab = (selected_tab + 1) % tab_titles.len();
                    selected_row = 0;
                }
                KeyCode::Char('H') => {
                    if selected_tab == 0 {
                        selected_tab = tab_titles.len() - 1;
                        selected_row = 0;
                    } else {
                        selected_tab -= 1;
                        selected_row = 0;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if selected_tab == 0 {
                        selected_subtab = (selected_subtab + 1) % subtab_titles.len();
                        selected_row = 0;
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if selected_tab == 0 {
                        if selected_subtab == 0 {
                            selected_subtab = subtab_titles.len() - 1;
                        } else {
                            selected_subtab -= 1;
                        }
                        selected_row = 0;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected_tab == 0 {
                        let max_rows = get_table_rows_count(selected_subtab);
                        if selected_row + 1 < max_rows {
                            selected_row += 1;
                        }
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if selected_tab == 0 && selected_row > 0 {
                        selected_row -= 1;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

// TODO: make this dynamic, as real data won't be of fixed length
fn get_table_rows_count(selected_subtab: usize) -> usize {
    match selected_subtab {
        0 => 3,
        1 => 2,
        2 => 2,
        3 => 2,
        4 => 2,
        5 => 2,
        _ => 0,
    }
}

// receives the state and renders the application
fn render(
    frame: &mut Frame,
    selected_tab: usize,
    tab_titles: &[&str],
    selected_subtab: usize,
    subtab_titles: &[&str],
    selected_row: usize,
) {
    // divide the terminal into 3 'chunks' (tabs, content area, now playing)
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

    // main 'library, search, feed' tabs
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

    // change content to be rendered based on the active tab
    if selected_tab == 0 {
        // divide the content area into 2 'chunks' (subtabs and table)
        let subchunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
            .split(chunks[1]);

        // 'likes, playlists, albums, ...' subtabs
        let subtabs: Vec<_> = subtab_titles.iter().map(|t| Span::raw(*t)).collect();
        let subtabs_widget = Tabs::new(subtabs)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .select(selected_subtab)
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(subtabs_widget, subchunks[0]);

        // define headers for tables for each subtab
        fn styled_header(cells: &[&str]) -> Row<'static> {
            let style = Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD);
            let cells: Vec<Cell> = cells
                .iter()
                .map(|&text| Cell::from(text.to_string()).style(style))
                .collect();
            Row::new(cells)
        }
        let (header, num_columns) = match selected_subtab {
            0 | 5 => (
                styled_header(&["Name", "Artist(s)", "Album", "Duration"]),
                4,
            ),
            1 => (styled_header(&["Name", "No. Songs", "Duration"]), 3),
            2 => (styled_header(&["Name", "Artist(s)", "Year", "Duration"]), 4),
            3 | 4 => (styled_header(&["Name"]), 1),
            _ => (Row::new(vec![] as Vec<Cell>), 0),
        };

        // define column widths for each subtab
        let column_widths: Vec<Constraint> = if num_columns > 0 {
            if num_columns > 2 {
                let other_width = 90 / (num_columns as u16 - 1);
                let mut widths = vec![Constraint::Percentage(other_width); num_columns - 1];
                widths.push(Constraint::Percentage(10));
                widths
            } else {
                let width = 100 / num_columns as u16;
                (0..num_columns)
                    .map(|_| Constraint::Percentage(width))
                    .collect()
            }
        } else {
            vec![]
        };

        // truncation to avoid cut-off text in columns
        let total_width = frame.area().width as usize;
        let col_max_widths: Vec<usize> = column_widths
            .iter()
            .map(|c| match c {
                Constraint::Percentage(p) => (total_width * (*p as usize)) / 100,
                _ => 10,
            })
            .collect();
        fn truncate_with_ellipsis(s: &str, max_width: usize) -> String {
            if s.chars().count() > max_width && max_width > 3 {
                let truncated: String = s.chars().take(max_width - 3).collect();
                format!("{}...", truncated)
            } else {
                s.to_string()
            }
        }

        // define rows for each subtab
        let rows = match selected_subtab {
            0 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Short song name", col_max_widths[0]),
                    truncate_with_ellipsis("Short artist name", col_max_widths[1]),
                    truncate_with_ellipsis("Short album name", col_max_widths[2]),
                    truncate_with_ellipsis("0:57", col_max_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Medium length song name", col_max_widths[0]),
                    truncate_with_ellipsis("Medium length artist name", col_max_widths[1]),
                    truncate_with_ellipsis("Medium length album name", col_max_widths[2]),
                    truncate_with_ellipsis("12:54", col_max_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis(
                        "Really really really long song name",
                        col_max_widths[0],
                    ),
                    truncate_with_ellipsis(
                        "Really really really long artist name",
                        col_max_widths[1],
                    ),
                    truncate_with_ellipsis(
                        "Really really really long album name",
                        col_max_widths[2],
                    ),
                    truncate_with_ellipsis("12:59:30", col_max_widths[3]),
                ]),
            ],
            1 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Playlist 1", col_max_widths[0]),
                    truncate_with_ellipsis("15", col_max_widths[1]),
                    truncate_with_ellipsis("30:00", col_max_widths[2]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Playlist 2", col_max_widths[0]),
                    truncate_with_ellipsis("1", col_max_widths[1]),
                    truncate_with_ellipsis("2:30", col_max_widths[2]),
                ]),
            ],
            2 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Album One", col_max_widths[0]),
                    truncate_with_ellipsis("Artist X", col_max_widths[1]),
                    truncate_with_ellipsis("1997", col_max_widths[2]),
                    truncate_with_ellipsis("45:02", col_max_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Album Two", col_max_widths[0]),
                    truncate_with_ellipsis("Artist Y", col_max_widths[1]),
                    truncate_with_ellipsis("2009", col_max_widths[2]),
                    truncate_with_ellipsis("16:03", col_max_widths[3]),
                ]),
            ],
            3 => vec![
                Row::new(vec![truncate_with_ellipsis(
                    "Station Jazz",
                    col_max_widths[0],
                )]),
                Row::new(vec![truncate_with_ellipsis(
                    "Station Rock",
                    col_max_widths[0],
                )]),
            ],
            4 => vec![
                Row::new(vec![truncate_with_ellipsis(
                    "Following Artist A",
                    col_max_widths[0],
                )]),
                Row::new(vec![truncate_with_ellipsis(
                    "Following Artist B",
                    col_max_widths[0],
                )]),
            ],
            5 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("History Song A", col_max_widths[0]),
                    truncate_with_ellipsis("Artist X", col_max_widths[1]),
                    truncate_with_ellipsis("Album X", col_max_widths[2]),
                    truncate_with_ellipsis("3:10", col_max_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("History Song B", col_max_widths[0]),
                    truncate_with_ellipsis("Artist Y", col_max_widths[1]),
                    truncate_with_ellipsis("Album Y", col_max_widths[2]),
                    truncate_with_ellipsis("2:54", col_max_widths[3]),
                ]),
            ],
            _ => vec![],
        };

        // highlight selected row
        let rows: Vec<_> = rows
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                if i == selected_row {
                    row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
                } else {
                    row
                }
            })
            .collect();

        // render table
        let table = Table::new(rows, column_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);

        frame.render_widget(table, subchunks[1]);
    } else {
        let content = match selected_tab {
            1 => "Content of Tab 2",
            2 => "Content of Tab 3",
            _ => "",
        };

        let paragraph = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, chunks[1]);
    }

    let now_playing = Paragraph::new("Now Playing Area")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        )
        .alignment(Alignment::Center);

    frame.render_widget(now_playing, chunks[2]);
}
