use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState},
};
use crate::theme;

use crate::config::Settings;
use crate::keymap::Action;
use crate::player::eq;
use crate::tui::logic::state::{AppState, HelpPage};
use crate::tui::render::utils::styled_header;

use super::utils::{centered_rect, centered_rect_fixed};

/// Key reference and editor, generated from the live keymap. Tab cycles it through
/// the settings and equaliser pages.
pub fn render_help(frame: &mut Frame, state: &AppState) {
    if state.help_page == HelpPage::Equalizer {
        let body = render_shell(
            frame,
            state,
            " Equaliser ",
            "↑↓: adjust   ←→: band   r: flat   Tab: keys   Esc: close",
        );
        render_bands(frame, body, state.eq.gains, state.help_eq_selected);
        return;
    }

    if state.help_page == HelpPage::Settings {
        let mut settings = state.settings;
        let rows: Vec<Row> = Settings::ROWS
            .iter()
            .map(|(label, field)| {
                let on = *field(&mut settings);
                let row = Row::new(vec![label.to_string(), if on { "on" } else { "off" }.to_string()]);
                if on {
                    row
                } else {
                    row.style(Style::default().fg(theme::current().dim))
                }
            })
            .collect();
        render_page(
            frame,
            state,
            " Settings ",
            &["Setting", "State"],
            rows,
            state.help_settings_selected,
            "Enter/Space: toggle   ↑↓: move   Tab: equaliser   Esc: close",
        );
        return;
    }

    let rows: Vec<Row> = Action::ALL
        .iter()
        .map(|action| {
            let keys = state.keymap.labels(*action);
            let mut row = Row::new(vec![action.description().to_string(), keys.clone()]);
            if keys == "unbound" {
                row = row.style(Style::default().fg(theme::current().dim));
            }
            row
        })
        .collect();
    render_page(
        frame,
        state,
        " Keys ",
        &["Action", "Keys"],
        rows,
        state.help_selected,
        "Enter: add key   Backspace: unbind   r: default   ↑↓: move   Tab: settings   Esc: close   ·  saved to ~/.config/sctui/config.toml",
    );
}

/// Widest a band column gets: enough for `230 Hz` and `+12 dB`.
const WIDE_CELL: u16 = 6;
/// The fallback when those do not fit: `230` and `+12`.
const NARROW_CELL: u16 = 3;
/// Rows either side of the zero line. Past this the faders stop growing with the
/// popup and stay a graphic EQ rather than a skyscraper.
const MAX_HALF: usize = 8;

/// Six vertical faders side by side, 0 dB on the shared centre line, fill growing
/// up for a boost and down for a cut. Centred in whatever space the popup leaves.
fn render_bands(frame: &mut Frame, area: Rect, gains: [i8; eq::BANDS.len()], selected: usize) {
    // The zero line plus the two label rows is the smallest thing worth drawing.
    if area.height < 3 || area.width < NARROW_CELL {
        return;
    }
    let bands = eq::BANDS.len() as u16;
    let units = area.width >= WIDE_CELL * bands + bands - 1;
    let (cell, bar) = if units { (WIDE_CELL, 4) } else { (NARROW_CELL, 2) };
    let width = cell * bands + bands - 1;
    let half = (((area.height - 3) / 2) as usize).min(MAX_HALF);

    let palette = theme::current();
    let dim = Style::default().fg(palette.dim);
    // `faded` is the zero line, which is scenery for every band but the selected one.
    let style = |band: usize, faded: bool| {
        if band == selected {
            // The same highlight the keys and settings tables give their row.
            Style::default().bg(palette.selection_bg).fg(palette.fg).add_modifier(Modifier::BOLD)
        } else if faded || gains[band] == 0 {
            dim
        } else {
            Style::default()
        }
    };
    let mut lines: Vec<Line> = Vec::with_capacity(2 * half + 3);
    let mut push = |gap: char, faded: bool, cell_at: &dyn Fn(usize) -> String| {
        let mut spans = Vec::with_capacity(2 * eq::BANDS.len());
        for band in 0..eq::BANDS.len() {
            if band > 0 {
                spans.push(Span::styled(gap.to_string(), dim));
            }
            spans.push(Span::styled(cell_at(band), style(band, faded)));
        }
        lines.push(Line::from(spans));
    };

    push(' ', false, &|band| format!("{:^1$}", gain_label(gains[band], units), cell as usize));
    for row in (1..=half).rev() {
        push(' ', false, &|band| {
            format!("{:^1$}", fill(gains[band], half, row, true).repeat(bar), cell as usize)
        });
    }
    push('─', true, &|_| "─".repeat(cell as usize));
    for row in 1..=half {
        push(' ', false, &|band| {
            format!("{:^1$}", fill(gains[band], half, row, false).repeat(bar), cell as usize)
        });
    }
    push(' ', false, &|band| format!("{:^1$}", band_label(eq::BANDS[band], units), cell as usize));

    // ponytail: under ~33 columns even the narrow group is wider than the popup
    // and the paragraph clips it. Showing fewer bands at once would be the fix.
    let height = lines.len() as u16;
    frame.render_widget(Paragraph::new(lines), centered_rect_fixed(width, height, area));
}

/// The character for one cell of a fader, `row` rows above (or below) the zero
/// line. Half blocks, so a single decibel still shows on a short popup.
fn fill(db: i8, half: usize, row: usize, above: bool) -> &'static str {
    if db == 0 || (db > 0) != above {
        return " ";
    }
    let max = eq::MAX_GAIN_DB as usize;
    let halves = (db.unsigned_abs() as usize * half * 2 + max / 2) / max;
    match halves.saturating_sub((row - 1) * 2) {
        0 => " ",
        1 if above => "▄",
        1 => "▀",
        _ => "█",
    }
}

/// Band label for one column: `60 Hz` when the column is wide, else `60`.
fn band_label(hz: f32, units: bool) -> String {
    match (units, hz >= 1000.0) {
        (true, _) => eq::band_label(hz),
        (false, true) => format!("{}k", hz / 1000.0),
        (false, false) => format!("{hz:.0}"),
    }
}

/// Gain label for one column: `+3 dB` when the column is wide, else `+3`.
fn gain_label(db: i8, units: bool) -> String {
    match (units, db) {
        (true, _) => eq::gain_label(db),
        (false, 0) => "0".to_string(),
        (false, _) => format!("{db:+}"),
    }
}

/// One page of the popup: a two-column table over the shared status and hint lines.
fn render_page(
    frame: &mut Frame,
    state: &AppState,
    title: &str,
    header: &[&str],
    rows: Vec<Row>,
    selected: usize,
    hint: &str,
) {
    let table_area = render_shell(frame, state, title, hint);
    let table = Table::new(rows, vec![Constraint::Percentage(62), Constraint::Percentage(38)])
        .header(styled_header(header))
        .column_spacing(1)
        .row_highlight_style(
            Style::default()
                .bg(theme::current().selection_bg)
                .fg(theme::current().fg)
                .add_modifier(Modifier::BOLD),
        );
    let mut table_state = TableState::default().with_selected(Some(selected));
    frame.render_stateful_widget(table, table_area, &mut table_state);
}

/// The popup frame every page shares: border, status line and hint line. Returns
/// the area left over for the page's own body.
fn render_shell(frame: &mut Frame, state: &AppState, title: &str, hint: &str) -> Rect {
    let popup_area = centered_rect(76, 80, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(title)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded);
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    let [body_area, status_area, hint_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let (status, status_style) = match (&state.help_capture, &state.help_message) {
        (Some(_), Some(msg)) => (msg.clone(), Style::default().fg(theme::current().warning).add_modifier(Modifier::BOLD)),
        (_, Some(msg)) => (msg.clone(), Style::default().fg(theme::current().accent)),
        _ => (String::new(), Style::default()),
    };
    frame.render_widget(Paragraph::new(status).style(status_style).alignment(Alignment::Center), status_area);
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(theme::current().dim))
            .alignment(Alignment::Center),
        hint_area,
    );
    body_area
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    /// The equaliser page as it lands on a terminal of the given size: the glyphs,
    /// and the columns the selection highlight painted.
    fn screen(
        width: u16,
        height: u16,
        gains: [i8; eq::BANDS.len()],
        selected: usize,
    ) -> (String, Vec<u16>) {
        let mut state = AppState {
            help_page: HelpPage::Equalizer,
            help_eq_selected: selected,
            ..Default::default()
        };
        state.eq.gains = gains;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| render_help(f, &state)).unwrap();
        let buf = terminal.backend().buffer();
        let text = (0..height)
            .map(|y| (0..width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        let bg = theme::current().selection_bg;
        let highlighted = (0..width)
            .filter(|&x| (0..height).any(|y| buf[(x, y)].bg == bg))
            .collect();
        (text, highlighted)
    }

    /// Column of the middle of `label`'s fader, counted in cells rather than bytes.
    fn col_of(row: &str, label: &str) -> usize {
        row[..row.find(label).unwrap()].chars().count() + 2
    }

    #[test]
    fn faders_are_vertical_and_centred() {
        let (s, highlighted) = screen(80, 24, [12, -12, 3, 0, -5, 1], 0);
        let lines: Vec<&str> = s.lines().collect();

        // Labels top and bottom, zero line between them, and the group centred.
        let gains = lines.iter().position(|l| l.contains("+12 dB")).unwrap();
        let inside = |l: &str| l.trim_matches(|c| c == ' ' || c == '│').to_string();
        let zero = lines
            .iter()
            .position(|l| {
                let l = inside(l);
                !l.is_empty() && l.chars().all(|c| c == '─')
            })
            .unwrap();
        let bands = lines.iter().position(|l| l.contains("230 Hz")).unwrap();
        assert!(gains < zero && zero < bands, "{s}");
        let row = lines[bands];
        let lead = row.chars().take_while(|c| *c == ' ' || *c == '│').count();
        let trail = row.chars().rev().take_while(|c| *c == ' ' || *c == '│').count();
        assert!(lead.abs_diff(trail) <= 1, "band group is not centred:\n{s}");

        // A boost fills above the zero line, a cut below, and flat fills neither.
        let col = col_of(row, "60 Hz");
        assert_eq!(lines[zero - 1].chars().nth(col), Some('█'), "+12 dB should fill up:\n{s}");
        assert_eq!(lines[zero + 1].chars().nth(col), Some(' '), "+12 dB must not fill down:\n{s}");
        let col = col_of(row, "230 Hz");
        assert_eq!(lines[zero + 1].chars().nth(col), Some('█'), "-12 dB should fill down:\n{s}");
        let col = col_of(row, "3 kHz");
        assert_eq!(lines[zero - 1].chars().nth(col), Some(' '), "0 dB is flat:\n{s}");
        assert_eq!(lines[zero + 1].chars().nth(col), Some(' '), "0 dB is flat:\n{s}");

        // Only the selected band is highlighted, and only its own column.
        let start = row[..row.find("60 Hz").unwrap()].chars().count() as u16;
        assert_eq!(highlighted, (start..start + WIDE_CELL).collect::<Vec<_>>(), "{s}");
    }

    #[test]
    fn a_cramped_popup_still_draws_every_band() {
        // 40x12 leaves three body rows and no room for units on the labels.
        let (s, highlighted) = screen(40, 12, [12, -12, 3, 0, -5, 1], 3);
        assert!(s.contains("16k"), "narrow labels missing:\n{s}");
        assert!(s.contains("-12"), "narrow gains missing:\n{s}");
        assert_eq!(highlighted.len(), NARROW_CELL as usize, "{s}");
        // Smaller still: the popup runs out of body entirely, but must not panic.
        for (w, h) in [(20u16, 6u16), (10, 4), (4, 3), (1, 1)] {
            screen(w, h, [12, -12, 3, 0, -5, 1], 5);
        }
    }
}
