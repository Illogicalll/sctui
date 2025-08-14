use color_eyre::eyre::{Ok, Result};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Tabs},
};

static NUM_TABS: usize = 3;
static NUM_SUBTABS: usize = 6;
static NUM_SEARCHFILTERS: usize = 4;

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
    let mut query = String::new();
    let searchfilters = ["Tracks", "Albums", "Playlists", "People"];
    let mut selected_searchfilter = 0;

    loop {
        terminal.draw(|frame| {
            render(
                frame,
                selected_tab,
                &tab_titles,
                selected_subtab,
                &subtab_titles,
                selected_row,
                &query,
                &searchfilters,
                selected_searchfilter,
            )
        })?;

        // support for both arrow keys and hjkl
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Tab => {
                    selected_tab = (selected_tab + 1) % NUM_TABS;
                    selected_row = 0;
                }
                KeyCode::Right => {
                    if selected_tab == 0 {
                        selected_subtab = (selected_subtab + 1) % NUM_SUBTABS;
                        selected_row = 0;
                    } else if selected_tab == 1 {
                        selected_searchfilter = (selected_searchfilter + 1) % NUM_SEARCHFILTERS;
                        selected_row = 0
                    }
                }
                KeyCode::Left => {
                    if selected_tab == 0 {
                        if selected_subtab == 0 {
                            selected_subtab = NUM_SUBTABS - 1;
                        } else {
                            selected_subtab -= 1;
                        }
                        selected_row = 0;
                    } else if selected_tab == 1 {
                        if selected_searchfilter == 0 {
                            selected_searchfilter = NUM_SEARCHFILTERS - 1;
                        } else {
                            selected_searchfilter -= 1;
                        }
                        selected_row = 0;
                    }
                }
                KeyCode::Down => {
                    if selected_tab == 0 {
                        let max_rows = get_table_rows_count(selected_subtab);
                        if selected_row + 1 < max_rows {
                            selected_row += 1;
                        }
                    }
                }
                KeyCode::Up => {
                    if selected_tab == 0 && selected_row > 0 {
                        selected_row -= 1;
                    }
                }
                KeyCode::Char(c) => {
                    if selected_tab == 1 {
                        query.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if selected_tab == 1 {
                        query.pop();
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

// styling for table headers
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

// column widths for tables based on the number of columns
fn calculate_column_widths(num_columns: usize) -> Vec<Constraint> {
    if num_columns == 0 {
        return vec![];
    }

    if num_columns > 2 {
        let other_width = 90 / (num_columns as u16 - 1);
        let mut widths = vec![Constraint::Percentage(other_width); num_columns - 1];
        widths.push(Constraint::Percentage(10)); // last column fixed at 10%
        widths
    } else {
        let width = 100 / num_columns as u16;
        (0..num_columns)
            .map(|_| Constraint::Percentage(width))
            .collect()
    }
}

// truncation with ellipsis for text overflow management
fn calculate_min_widths(column_widths: &[Constraint], total_width: usize) -> Vec<usize> {
    column_widths
        .iter()
        .map(|c| match c {
            Constraint::Percentage(p) => (total_width * (*p as usize)) / 100,
            _ => 10,
        })
        .collect()
}

fn truncate_with_ellipsis(s: &str, min_width: usize) -> String {
    if s.chars().count() > min_width && min_width > 3 {
        let truncated: String = s.chars().take(min_width - 3).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
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
    query: &str,
    searchfilters: &[&str],
    selected_searchfilter: usize,
) {
    let width = frame.area().width as usize;

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
    //
    // ===========
    // LIBRARY TAB
    // ===========
    //
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
        let (header, num_columns) = match selected_subtab {
            0 | 5 => (
                styled_header(&["Title", "Artist(s)", "Album", "Duration"]),
                4,
            ),
            1 => (styled_header(&["Name", "No. Songs", "Duration"]), 3),
            2 => (
                styled_header(&["Title", "Artist(s)", "Year", "Duration"]),
                4,
            ),
            3 | 4 => (styled_header(&["Name"]), 1),
            _ => (Row::new(vec![] as Vec<Cell>), 0),
        };

        // truncation to avoid cut-off text in columns
        let col_widths = calculate_column_widths(num_columns);
        let col_min_widths = calculate_min_widths(&col_widths, width);

        // define rows for each subtab
        let rows = match selected_subtab {
            0 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Short song name", col_min_widths[0]),
                    truncate_with_ellipsis("Short artist name", col_min_widths[1]),
                    truncate_with_ellipsis("Short album name", col_min_widths[2]),
                    truncate_with_ellipsis("0:57", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Medium length song name", col_min_widths[0]),
                    truncate_with_ellipsis("Medium length artist name", col_min_widths[1]),
                    truncate_with_ellipsis("Medium length album name", col_min_widths[2]),
                    truncate_with_ellipsis("12:54", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis(
                        "Really really really long song name",
                        col_min_widths[0],
                    ),
                    truncate_with_ellipsis(
                        "Really really really long artist name",
                        col_min_widths[1],
                    ),
                    truncate_with_ellipsis(
                        "Really really really long album name",
                        col_min_widths[2],
                    ),
                    truncate_with_ellipsis("12:59:30", col_min_widths[3]),
                ]),
            ],
            1 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Playlist 1", col_min_widths[0]),
                    truncate_with_ellipsis("15", col_min_widths[1]),
                    truncate_with_ellipsis("30:00", col_min_widths[2]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Playlist 2", col_min_widths[0]),
                    truncate_with_ellipsis("1", col_min_widths[1]),
                    truncate_with_ellipsis("2:30", col_min_widths[2]),
                ]),
            ],
            2 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Album One", col_min_widths[0]),
                    truncate_with_ellipsis("Artist X", col_min_widths[1]),
                    truncate_with_ellipsis("1997", col_min_widths[2]),
                    truncate_with_ellipsis("45:02", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Album Two", col_min_widths[0]),
                    truncate_with_ellipsis("Artist Y", col_min_widths[1]),
                    truncate_with_ellipsis("2009", col_min_widths[2]),
                    truncate_with_ellipsis("16:03", col_min_widths[3]),
                ]),
            ],
            3 => vec![
                Row::new(vec![truncate_with_ellipsis(
                    "Station Jazz",
                    col_min_widths[0],
                )]),
                Row::new(vec![truncate_with_ellipsis(
                    "Station Rock",
                    col_min_widths[0],
                )]),
            ],
            4 => vec![
                Row::new(vec![truncate_with_ellipsis(
                    "Following Artist A",
                    col_min_widths[0],
                )]),
                Row::new(vec![truncate_with_ellipsis(
                    "Following Artist B",
                    col_min_widths[0],
                )]),
            ],
            5 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("History Song A", col_min_widths[0]),
                    truncate_with_ellipsis("Artist X", col_min_widths[1]),
                    truncate_with_ellipsis("Album X", col_min_widths[2]),
                    truncate_with_ellipsis("3:10", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("History Song B", col_min_widths[0]),
                    truncate_with_ellipsis("Artist Y", col_min_widths[1]),
                    truncate_with_ellipsis("Album Y", col_min_widths[2]),
                    truncate_with_ellipsis("2:54", col_min_widths[3]),
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
        let table = Table::new(rows, col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);

        frame.render_widget(table, subchunks[1]);
    }
    //
    // ==========
    // SEARCH TAB
    // ==========
    //
    else if selected_tab == 1 {
        // divide the content area into 2 'chunks' (search bar and table)
        let subchunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(chunks[1]);

        // search bar
        let input = Paragraph::new(query.to_string())
            .block(
                Block::default()
                    .title("search")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .alignment(Alignment::Center);
        frame.render_widget(input, subchunks[0]);

        // define headers for different search filters
        let (header, num_columns) = match selected_searchfilter {
            0 => (
                styled_header(&["Title", "Artist(s)", "Album", "Duration"]),
                4,
            ),
            1 => (
                styled_header(&["Title", "Artist(s)", "Year", "Duration"]),
                4,
            ),
            2 => (styled_header(&["Name", "No. Songs", "Duration"]), 3),
            3 => (styled_header(&["Name"]), 1),
            _ => (Row::new(vec![] as Vec<Cell>), 0),
        };

        // truncation to avoid cut-off text in columns
        let col_widths = calculate_column_widths(num_columns);
        let col_min_widths = calculate_min_widths(&col_widths, width);

        // define rows based on search filter
        let rows = match selected_searchfilter {
            0 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Short song name", col_min_widths[0]),
                    truncate_with_ellipsis("Short artist name", col_min_widths[1]),
                    truncate_with_ellipsis("Short album name", col_min_widths[2]),
                    truncate_with_ellipsis("0:57", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Medium length song name", col_min_widths[0]),
                    truncate_with_ellipsis("Medium length artist name", col_min_widths[1]),
                    truncate_with_ellipsis("Medium length album name", col_min_widths[2]),
                    truncate_with_ellipsis("12:54", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis(
                        "Really really really long song name",
                        col_min_widths[0],
                    ),
                    truncate_with_ellipsis(
                        "Really really really long artist name",
                        col_min_widths[1],
                    ),
                    truncate_with_ellipsis(
                        "Really really really long album name",
                        col_min_widths[2],
                    ),
                    truncate_with_ellipsis("12:59:30", col_min_widths[3]),
                ]),
            ],
            1 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Album One", col_min_widths[0]),
                    truncate_with_ellipsis("Artist X", col_min_widths[1]),
                    truncate_with_ellipsis("1997", col_min_widths[2]),
                    truncate_with_ellipsis("45:02", col_min_widths[3]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Album Two", col_min_widths[0]),
                    truncate_with_ellipsis("Artist Y", col_min_widths[1]),
                    truncate_with_ellipsis("2009", col_min_widths[2]),
                    truncate_with_ellipsis("16:03", col_min_widths[3]),
                ]),
            ],
            2 => vec![
                Row::new(vec![
                    truncate_with_ellipsis("Playlist 1", col_min_widths[0]),
                    truncate_with_ellipsis("15", col_min_widths[1]),
                    truncate_with_ellipsis("30:00", col_min_widths[2]),
                ]),
                Row::new(vec![
                    truncate_with_ellipsis("Playlist 2", col_min_widths[0]),
                    truncate_with_ellipsis("1", col_min_widths[1]),
                    truncate_with_ellipsis("2:30", col_min_widths[2]),
                ]),
            ],
            3 => vec![
                Row::new(vec![truncate_with_ellipsis(
                    "Following Artist A",
                    col_min_widths[0],
                )]),
                Row::new(vec![truncate_with_ellipsis(
                    "Following Artist B",
                    col_min_widths[0],
                )]),
            ],
            _ => vec![],
        };

        // render table
        let table = Table::new(rows, col_widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .column_spacing(1);
        frame.render_widget(table, subchunks[1]);

        // center search filters
        let tab_width = width / NUM_SEARCHFILTERS;
        fn center_text_in_width(text: &str, width: usize) -> String {
            let total_padding = width - text.chars().count();
            let padding = (total_padding / 2) - 1;
            format!("{}{}{}", " ".repeat(padding), text, " ".repeat(padding))
        }

        // render search filters
        let searchfilter: Vec<Span<'static>> = searchfilters
            .iter()
            .map(|filter| Span::raw(center_text_in_width(filter, tab_width)))
            .collect();
        let searchfilter_widget = Tabs::new(searchfilter)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("filter")
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded),
            )
            .select(selected_searchfilter)
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(searchfilter_widget, subchunks[2]);
    } else {
        let paragraph = Paragraph::new("Content of Feed Tab")
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
