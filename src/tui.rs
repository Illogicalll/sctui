use crate::api::{API, Album, Playlist, Track};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState, Tabs},
};
use std::result::Result::Ok;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

static NUM_TABS: usize = 3;
static NUM_SUBTABS: usize = 6;
static NUM_SEARCHFILTERS: usize = 4;
static NUM_FEED_ACTIVITY_COLS: usize = 4;
static NUM_FEED_INFO_COLS: usize = 3;
static REFRESH_THRESHOLD: usize = 5;

// prep terminal and color_eyre error reporting
pub fn run(api: &mut Arc<Mutex<API>>) -> anyhow::Result<()> {
    color_eyre::install().map_err(|e| anyhow::anyhow!(e))?;
    let terminal = ratatui::init();
    let result = start(terminal, api);
    ratatui::restore();
    result
}

// state management and render loop
fn start(mut terminal: DefaultTerminal, api: &mut Arc<Mutex<API>>) -> anyhow::Result<()> {
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
    let mut info_pane_selected = false;
    let mut selected_info_row = 0;

    let mut api_guard = api.lock().unwrap();

    let mut likes: Vec<Track> = api_guard.get_liked_tracks()?.into_iter().collect();
    let mut likes_state = TableState::default();
    likes_state.select(Some(selected_row));

    let mut playlists: Vec<Playlist> = api_guard.get_playlists()?.into_iter().collect();
    let mut playlists_state = TableState::default();
    playlists_state.select(Some(selected_row));

    let mut albums: Vec<Album> = api_guard.get_albums()?.into_iter().collect();
    let mut albums_state = TableState::default();
    albums_state.select(Some(selected_row));

    drop(api_guard);

    let get_table_rows_count = |selected_subtab: usize,
                                likes: &Vec<Track>,
                                playlists: &Vec<Playlist>,
                                albums: &Vec<Album>|
     -> usize {
        match selected_subtab {
            0 => likes.len(),
            1 => playlists.len(),
            2 => albums.len(),
            3 => 2,
            4 => 2,
            5 => 2,
            _ => 0,
        }
    };

    let (tx_likes, rx_likes): (Sender<Vec<Track>>, Receiver<Vec<Track>>) = mpsc::channel();
    let (tx_playlists, rx_playlists): (Sender<Vec<Playlist>>, Receiver<Vec<Playlist>>) =
        mpsc::channel();
    let (tx_albums, rx_albums): (Sender<Vec<Album>>, Receiver<Vec<Album>>) = mpsc::channel();

    loop {
        // attempt to update application data between frames (to avoid hitching)
        while let Ok(new_likes) = rx_likes.try_recv() {
            likes.extend(new_likes.into_iter());
        }

        while let Ok(new_playlists) = rx_playlists.try_recv() {
            playlists.extend(new_playlists.into_iter());
        }

        while let Ok(new_albums) = rx_albums.try_recv() {
            albums.extend(new_albums.into_iter());
        }

        terminal.draw(|frame| {
            render(
                frame,
                &likes,
                &mut likes_state,
                &playlists,
                &mut playlists_state,
                &albums,
                &mut albums_state,
                selected_tab,
                &tab_titles,
                selected_subtab,
                &subtab_titles,
                selected_row,
                &query,
                &searchfilters,
                selected_searchfilter,
                info_pane_selected,
                selected_info_row,
            )
        })?;

        // input handling
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
                    } else if selected_tab == 2 {
                        info_pane_selected = !info_pane_selected
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
                    } else if selected_tab == 2 {
                        info_pane_selected = !info_pane_selected
                    }
                }
                KeyCode::Down => {
                    let max_rows =
                        get_table_rows_count(selected_subtab, &likes, &playlists, &albums);
                    let max_info_rows = get_info_table_rows_count();
                    if selected_tab == 2
                        && info_pane_selected
                        && selected_info_row + 1 < max_info_rows
                    {
                        selected_info_row += 1;
                    } else {
                        if selected_row + 1 < max_rows {
                            selected_row += 1;

                            match selected_subtab {
                                0 => {
                                    likes_state.select(Some(selected_row));
                                }
                                1 => {
                                    playlists_state.select(Some(selected_row));
                                }
                                _ => {}
                            }

                            if max_rows >= REFRESH_THRESHOLD
                                && selected_row + REFRESH_THRESHOLD >= max_rows
                            {
                                match selected_subtab {
                                    0 => {
                                        let api_clone = Arc::clone(api);
                                        let tx = tx_likes.clone();

                                        std::thread::spawn(move || {
                                            let new_likes =
                                                api_clone.lock().unwrap().get_liked_tracks();
                                            if let Ok(tracks) = new_likes {
                                                let _ = tx.send(tracks);
                                            }
                                        });
                                    }
                                    1 => {
                                        let api_clone = Arc::clone(api);
                                        let tx = tx_playlists.clone();

                                        std::thread::spawn(move || {
                                            let new_playlists =
                                                api_clone.lock().unwrap().get_playlists();
                                            if let Ok(playlists) = new_playlists {
                                                let _ = tx.send(playlists);
                                            }
                                        });
                                    }
                                    2 => {
                                        let api_clone = Arc::clone(api);
                                        let tx = tx_albums.clone();

                                        std::thread::spawn(move || {
                                            let new_albums = api_clone.lock().unwrap().get_albums();
                                            if let Ok(albums) = new_albums {
                                                let _ = tx.send(albums);
                                            }
                                        });
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                KeyCode::Up => {
                    if selected_tab == 2 && info_pane_selected && selected_info_row > 0 {
                        selected_info_row -= 1;
                    } else if selected_row > 0 {
                        selected_row -= 1;
                        likes_state.select(Some(selected_row));
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

fn get_info_table_rows_count() -> usize {
    2
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
    likes: &Vec<Track>,
    likes_state: &mut TableState,
    playlists: &Vec<Playlist>,
    playlists_state: &mut TableState,
    albums: &Vec<Album>,
    albums_state: &mut TableState,
    selected_tab: usize,
    tab_titles: &[&str],
    selected_subtab: usize,
    subtab_titles: &[&str],
    selected_row: usize,
    query: &str,
    searchfilters: &[&str],
    selected_searchfilter: usize,
    info_pane_selected: bool,
    selected_info_row: usize,
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

        // define headers and column widths for tables for each subtab
        let (header, col_widths) = match selected_subtab {
            0 | 5 => (
                styled_header(&["Title", "Artist(s)", "Duration", "Streams"]),
                vec![
                    Constraint::Percentage(55),
                    Constraint::Percentage(25),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                ],
            ),
            1 => (
                styled_header(&["Name", "No. Songs", "Duration"]),
                vec![
                    Constraint::Percentage(80),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                ],
            ),
            2 => (
                styled_header(&["Title", "Artist(s)", "Year", "No. Songs", "Duration"]),
                vec![
                    Constraint::Percentage(50),
                    Constraint::Percentage(20),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                ],
            ),
            3 | 4 => (styled_header(&["Name"]), vec![Constraint::Percentage(100)]),
            _ => (
                Row::new(vec![] as Vec<Cell>),
                vec![Constraint::Percentage(100)],
            ),
        };

        // truncation to avoid cut-off text in columns
        let col_min_widths = calculate_min_widths(&col_widths, width);

        // define rows for each subtab
        let rows = match selected_subtab {
            0 => likes
                .iter()
                .map(|track| {
                    Row::new(vec![
                        truncate_with_ellipsis(&track.title, col_min_widths[0]),
                        truncate_with_ellipsis(&track.artists, col_min_widths[1]),
                        truncate_with_ellipsis(&track.duration, col_min_widths[2]),
                        truncate_with_ellipsis(&track.playback_count, col_min_widths[3]),
                    ])
                })
                .collect(),
            1 => playlists
                .iter()
                .map(|playlist| {
                    Row::new(vec![
                        truncate_with_ellipsis(&playlist.title, col_min_widths[0]),
                        truncate_with_ellipsis(&playlist.track_count, col_min_widths[1]),
                        truncate_with_ellipsis(&playlist.duration, col_min_widths[2]),
                    ])
                })
                .collect(),
            2 => albums
                .iter()
                .map(|album| {
                    Row::new(vec![
                        truncate_with_ellipsis(&album.title, col_min_widths[0]),
                        truncate_with_ellipsis(&album.artists, col_min_widths[1]),
                        truncate_with_ellipsis(&album.release_year, col_min_widths[2]),
                        truncate_with_ellipsis(&album.track_count, col_min_widths[3]),
                        truncate_with_ellipsis(&album.duration, col_min_widths[4]),
                    ])
                })
                .collect(),
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

        let state: &mut TableState = match selected_subtab {
            0 => likes_state,
            1 => playlists_state,
            2 => albums_state,
            _ => likes_state,
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

        frame.render_stateful_widget(table, subchunks[1], state);
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
    }
    //
    // ========
    // FEED TAB
    // ========
    //
    else {
        // split the content area vertically
        let subchunks = Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(chunks[1]);

        let activity_header = styled_header(&["User", "Action", "Media Type", "Age"]);

        // truncation to avoid cut-off text in columns
        let activity_col_widths = calculate_column_widths(NUM_FEED_ACTIVITY_COLS);
        let activity_col_min_widths = calculate_min_widths(&activity_col_widths, width / 2);

        // define activity table content
        let activity_rows = vec![
            vec![
                truncate_with_ellipsis("User 1", activity_col_min_widths[0]),
                truncate_with_ellipsis("Post", activity_col_min_widths[1]),
                truncate_with_ellipsis("Track", activity_col_min_widths[2]),
                truncate_with_ellipsis("2d", activity_col_min_widths[3]),
            ],
            vec![
                truncate_with_ellipsis("User 2", activity_col_min_widths[0]),
                truncate_with_ellipsis("Repost", activity_col_min_widths[1]),
                truncate_with_ellipsis("Album", activity_col_min_widths[2]),
                truncate_with_ellipsis("5d", activity_col_min_widths[3]),
            ],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, cols)| {
            let row = Row::new(cols);
            if i == selected_row && info_pane_selected {
                row.style(Style::default().bg(Color::Gray).fg(Color::Black))
            } else if i == selected_row && !info_pane_selected {
                row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
            } else {
                row
            }
        })
        .collect::<Vec<_>>();

        // render activity table
        let table = Table::new(activity_rows, activity_col_widths)
            .header(activity_header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("activity")
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(if info_pane_selected {
                        Style::default()
                    } else {
                        Style::default().fg(Color::Cyan)
                    }),
            )
            .column_spacing(1);
        frame.render_widget(table, subchunks[0]);

        let info_header = styled_header(&["Title", "Artist", "Dur."]);

        // truncation to avoid cut-off text in columns
        let info_col_widths = calculate_column_widths(NUM_FEED_INFO_COLS);
        let info_col_min_widths = calculate_min_widths(&info_col_widths, width / 2);

        // define info table content
        let info_rows = vec![
            vec![
                truncate_with_ellipsis("Track 1", info_col_min_widths[0]),
                truncate_with_ellipsis("Artist 1", info_col_min_widths[1]),
                truncate_with_ellipsis("1:30", info_col_min_widths[2]),
            ],
            vec![
                truncate_with_ellipsis("Track 2", info_col_min_widths[0]),
                truncate_with_ellipsis("Artist 1", info_col_min_widths[1]),
                truncate_with_ellipsis("2:10", info_col_min_widths[2]),
            ],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, cols)| {
            let row = Row::new(cols);
            if i == selected_info_row && info_pane_selected {
                row.style(Style::default().bg(Color::LightBlue).fg(Color::White))
            } else if i == selected_info_row && !info_pane_selected {
                row.style(Style::default().bg(Color::Gray).fg(Color::Black))
            } else {
                row
            }
        })
        .collect::<Vec<_>>();

        // render info table
        let table = Table::new(info_rows, info_col_widths)
            .header(info_header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("info")
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(if info_pane_selected {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    }),
            )
            .column_spacing(1);

        frame.render_widget(table, subchunks[1]);
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
