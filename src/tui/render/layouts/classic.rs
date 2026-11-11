//! Classic: the screen sctui started with, as a skin over the shared widgets. Boxed tab
//! bar, boxed sub-tabs, search and filter-type boxes, the original pane proportions
//! and the 7-row player when the terminal is tall enough; compact header rows and the
//! smaller players when it is not.

use ratatui::{
    Frame,
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Tabs},
};
use ratatui_image::thread::ThreadProtocol;

use crate::keymap::Action;
use crate::theme;
use crate::tui::logic::filtering::FilteredViews;
use crate::tui::logic::nav;
use crate::tui::logic::state::{AppData, AppState, SEARCHFILTERS, SUBTAB_TITLES, visible_tabs};
use crate::tui::render::NowPlaying;
use crate::tui::render::widgets::pane::PaneStyle;
use crate::tui::render::widgets::pane_row::draw_pane_row;
use crate::tui::render::widgets::player::{draw_player, player_size};
use crate::tui::render::widgets::{BRAND_MARK, header, hints};

use super::{take_bottom, take_top};

/// Terminal rows below which the boxed header gives way to two plain rows.
pub const ROWS_BOXED: u16 = 24;

/// Each pane's share of the row, as the original screen split them.
fn weights(state: &AppState) -> Option<&'static [u16]> {
    match nav::location(state) {
        (0, 1) | (1, 2) => Some(&[33, 67]),
        (0, 2) | (1, 1) => Some(&[65, 35]),
        (0, 3) | (1, 3) => Some(&[20, 40, 40]),
        (2, _) => Some(&[40, 60]),
        _ => None,
    }
}

fn boxed(title: &'static str) -> Block<'static> {
    Block::default().title(title).title_alignment(Alignment::Center).borders(Borders::ALL).border_type(BorderType::Rounded)
}

fn tabs<'a>(titles: impl IntoIterator<Item = Span<'a>>, selected: usize, block: Block<'a>) -> Tabs<'a> {
    let t = theme::current();
    Tabs::new(titles)
        .block(block)
        .select(selected)
        .style(Style::new().fg(t.fg))
        .highlight_style(Style::new().fg(t.accent).add_modifier(Modifier::BOLD))
}

/// The Search query as the search box shows it.
fn query_line(state: &AppState) -> Line<'static> {
    let t = theme::current();
    if state.search_typing {
        Line::styled(format!("{}▏", state.query), Style::new().fg(t.fg).add_modifier(Modifier::BOLD))
    } else if state.query.is_empty() {
        let key = state.keymap.first_label(Action::Search);
        let key = if key.is_empty() { "/".to_string() } else { key };
        Line::styled(format!("press {key} to search"), Style::new().fg(t.dim))
    } else {
        Line::styled(state.query.clone(), Style::new().fg(t.fg))
    }
}

pub fn draw(
    frame: &mut Frame,
    state: &mut AppState,
    data: &mut AppData,
    views: &FilteredViews,
    np: &NowPlaying,
    cover: Option<&mut ThreadProtocol>,
) {
    let mut rest = frame.area();
    let hint_rows = hints::rows(state, rest);
    take_bottom(&mut rest, hint_rows);
    let size = player_size(true, frame.area().height);
    let player = take_bottom(&mut rest, size.height());
    draw_player(frame, player, size, state, data, np, cover);

    if frame.area().height < ROWS_BOXED {
        header::draw_compact(frame, take_top(&mut rest, 2), state, data);
    } else {
        let mark = Span::styled(BRAND_MARK, Style::new().fg(theme::current().accent));
        let title = Line::from(vec![mark, Span::styled("sctui", Style::new().add_modifier(Modifier::BOLD))]);
        let block = Block::default().title(title).title_alignment(Alignment::Center).borders(Borders::ALL).border_type(BorderType::Rounded);
        let titles = visible_tabs(state).iter().map(|t| Span::raw(*t));
        frame.render_widget(tabs(titles, state.selected_tab, block), take_top(&mut rest, 3));

        if state.selected_tab == 0 {
            let block = Block::default().borders(Borders::ALL).border_type(BorderType::Rounded);
            let titles = SUBTAB_TITLES.iter().map(|t| Span::raw(*t));
            frame.render_widget(tabs(titles, state.selected_subtab, block), take_top(&mut rest, 3));
            if state.search_popup_visible {
                let cursor = if state.search_typing { "▏" } else { "" };
                let input = Paragraph::new(format!("{}{cursor}", state.search_query))
                    .block(boxed("search"))
                    .alignment(Alignment::Center);
                frame.render_widget(input, take_top(&mut rest, 3));
            }
        } else if state.selected_tab == 1 {
            let input = Paragraph::new(query_line(state)).block(boxed("search")).alignment(Alignment::Center);
            frame.render_widget(input, take_top(&mut rest, 3));

            // Search types, each centred in its quarter of the box.
            let area = take_bottom(&mut rest, 3);
            let quarter = area.width as usize / SEARCHFILTERS.len();
            let titles = SEARCHFILTERS.iter().map(|title| {
                let pad = (quarter.saturating_sub(title.chars().count()) / 2).saturating_sub(1);
                Span::raw(format!("{0}{title}{0}", " ".repeat(pad)))
            });
            frame.render_widget(tabs(titles, state.selected_searchfilter, boxed("filter")), area);
        }
    }

    let weights = weights(state);
    draw_pane_row(frame, rest, state, data, views, PaneStyle { boxed_title: true }, weights, np);
}
