//! The setup wizard's screens: welcome and done boxes, the layout and theme lists (to
//! one side, so the real layout behind them is the preview), and the practice stage's
//! lesson card, which becomes a two-row strip in the hint line's place when the
//! terminal is too small to spare a corner.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::config::Layout;
use crate::keymap::Action;
use crate::theme;
use crate::tui::logic::nav;
use crate::tui::logic::palette;
use crate::tui::logic::state::AppState;
use crate::tui::logic::wizard::{Key, LESSONS, Stage, practising};
use crate::tui::render::layouts::panel::COLS_PANEL;
use crate::tui::render::widgets::player::player_size;
use crate::tui::render::widgets::{clear, fit, hints, put, put_right, text_width};

use super::theme_picker;
use super::utils::centered_rect_fixed;

/// Terminal size from which the lesson is a card in the corner; below it, a strip.
pub const COLS_CARD: u16 = 100;
pub const ROWS_CARD: u16 = 24;
pub const STRIP_ROWS: u16 = 2;
const CARD_WIDTH: u16 = 44;
const CARD_HEIGHT: u16 = 7;

/// The one-time question for a config that predates the wizard.
pub const TOUR_PROMPT: &str = "Keys have changed. Take the two-minute tour?";

/// The practice stage is running on a terminal too small for the lesson card.
pub fn is_strip(state: &AppState, screen: Rect) -> bool {
    practising(state) && (screen.width < COLS_CARD || screen.height < ROWS_CARD)
}

/// Draw whichever wizard screen is up; the strip form of the lesson is `draw_strip`.
pub fn render(frame: &mut Frame, state: &AppState) {
    let Some(wizard) = &state.wizard else { return };
    let screen = frame.area();
    match wizard.stage {
        Stage::Welcome => welcome(frame),
        Stage::Layout => layout_list(frame, wizard.selected),
        Stage::Theme => {
            let area = side(screen, theme_picker::WIDTH, theme_picker::HEIGHT);
            theme_picker::render_theme_picker(frame, area, state.theme_picker_selected, &state.theme_name);
        }
        Stage::Practice if !is_strip(state, screen) => draw_card(frame, card_area(state, screen), state),
        Stage::Practice => {}
        Stage::Done => done(frame, state),
    }
}

/// A `width`×`height` box at the left edge, vertically centred, clamped to `screen`.
fn side(screen: Rect, width: u16, height: u16) -> Rect {
    let area = centered_rect_fixed(width, height, screen);
    Rect { x: screen.x + 2.min(screen.width - area.width), ..area }
}

fn boxed(frame: &mut Frame, area: Rect, title: &str) -> Rect {
    let t = theme::current();
    clear(frame, area);
    let block = Block::default()
        .title(Line::styled(format!(" {title} "), Style::new().fg(t.accent).add_modifier(Modifier::BOLD)))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    // One cell of padding either side of the text.
    Rect { x: inner.x + 1.min(inner.width), width: inner.width.saturating_sub(2), ..inner }
}

fn welcome(frame: &mut Frame) {
    let t = theme::current();
    let inner = boxed(frame, centered_rect_fixed(58, 11, frame.area()), "Welcome to sctui");
    let key = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(t.dim);
    let text = vec![
        Line::raw(""),
        Line::styled("Your SoundCloud library, played from the terminal.", Style::new().fg(t.fg)),
        Line::raw(""),
        Line::styled(
            "Let's get started: pick a layout, a theme and then take a quick tour of the \
             keybinds (nothing will affect your actual SoundCloud library).",
            Style::new().fg(t.muted),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("enter", key),
            Span::styled(" continue      ", dim),
            Span::styled("esc", key),
            Span::styled(" skip all of it", dim),
        ]),
    ];
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
}

fn done(frame: &mut Frame, state: &AppState) {
    let t = theme::current();
    let inner = boxed(frame, centered_rect_fixed(58, 10, frame.area()), "You're set");
    let key = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let (fg, dim) = (Style::new().fg(t.fg), Style::new().fg(t.dim));
    let label = |action| match state.keymap.hint_label(action) {
        unbound if unbound.is_empty() => "(unbound)".to_string(),
        label => label,
    };
    let text = vec![
        Line::raw(""),
        Line::from(vec![Span::styled(label(Action::Help), key), Span::styled("  keys, settings and the equaliser", fg)]),
        Line::from(vec![Span::styled(label(Action::Palette), key), Span::styled("  the palette: every action, layout and theme", fg)]),
        Line::raw(""),
        Line::from(vec![Span::styled("Setup wizard", key), Span::styled(" in the palette, or ", fg), Span::styled("sctui --setup", key), Span::styled(", runs this again.", fg)]),
        Line::raw(""),
        Line::from(vec![Span::styled("enter", key), Span::styled(" start listening", dim)]),
    ];
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }), inner);
}

fn layout_list(frame: &mut Frame, selected: usize) {
    let t = theme::current();
    let area = side(frame.area(), theme_picker::WIDTH, Layout::ALL.len() as u16 + 5);
    let inner = boxed(frame, area, "Layout");
    for (i, layout) in Layout::ALL.into_iter().enumerate() {
        let what = match layout {
            Layout::Classic => "the original look",
            Layout::Tighten => "less chrome, more rows",
            Layout::Sidebar => "sections and playlists on the left",
            Layout::Columns => "Miller columns you step through",
            Layout::PlayerPanel => "a permanent player on the right",
        };
        let row = Rect { y: inner.y + 1 + i as u16, height: 1, ..inner };
        if row.y >= inner.bottom() {
            break;
        }
        let (name, rest) = if i == selected {
            let bar = Style::new().bg(t.selection_bg).fg(t.selection_fg).add_modifier(Modifier::BOLD);
            frame.buffer_mut().set_style(row, bar);
            (bar, bar)
        } else {
            (Style::new().fg(t.fg), Style::new().fg(t.dim))
        };
        put(frame, row, Line::from(vec![Span::styled(format!(" {:<13}", layout.label()), name), Span::styled(what, rest)]));
    }
    let hint = Rect { y: inner.bottom().saturating_sub(1), height: 1.min(inner.height), ..inner };
    put(frame, hint, Line::styled("↑↓ preview   Enter: keep   Esc: skip", Style::new().fg(t.dim)).centered());
}

/// What the lesson card says, however it is drawn.
struct Card {
    /// `Tour 4 / 10`
    title: String,
    /// The goal of the lesson just finished.
    ticked: Option<&'static str>,
    goal: &'static str,
    why: &'static str,
    /// The keys to press, from the live keymap, ticked as each part is done.
    keys: Vec<Span<'static>>,
    /// What is in the way of this lesson where the user is now; usually empty.
    note: &'static str,
}

fn card(state: &AppState) -> Option<Card> {
    let wizard = state.wizard.as_ref().filter(|w| w.stage == Stage::Practice)?;
    let lesson = LESSONS.get(wizard.step)?;
    let t = theme::current();
    let km = &state.keymap;
    let (key, fg, dim) = (Style::new().fg(t.accent).add_modifier(Modifier::BOLD), Style::new().fg(t.fg), Style::new().fg(t.dim));
    let help = km.hint_label(Action::Help);
    let mut keys = Vec::new();
    for (i, (k, what)) in lesson.keys.iter().enumerate() {
        if i > 0 {
            keys.push(Span::raw("   "));
        }
        let (label, in_palette) = match k {
            Key::Bound(action) => (km.hint_label(*action), palette::listed(*action)),
            Key::Fixed(label) => (label.to_string(), false),
        };
        if label.is_empty() {
            // Unbound: say so, and how to do it anyway.
            let palette = km.hint_label(Action::Palette);
            keys.push(Span::styled(format!("{what}: unbound"), Style::new().fg(t.warning)));
            keys.push(Span::styled(
                match in_palette && !palette.is_empty() {
                    true => format!(" · run it from {palette}, bind in {help}"),
                    false => format!(" · bind it in {help}"),
                },
                dim,
            ));
        } else {
            keys.push(Span::styled(label, key));
            keys.push(Span::styled(format!(" {what}"), fg));
        }
        if lesson.keys.len() > 1 && wizard.sub & (1 << i) != 0 {
            keys.push(Span::styled(" ✓", key));
        }
    }
    if lesson.times > 1 {
        keys.push(Span::styled(format!("   {} / {}", wizard.sub.count_ones(), lesson.times), dim));
    }
    let on_tracks = !nav::in_nav(state) && !nav::focused(state).is_container();
    let note = match wizard.step {
        3 if nav::panes(state).len() < 2 && !nav::has_nav(state.settings.layout) => "One pane here: try Library › Playlists.",
        4 if !on_tracks => "This row opens: press again on a track.",
        6 if !on_tracks => "Focus a list of tracks first.",
        7 if state.selected_tab != 0 => "Filters are for the Library lists.",
        _ => "",
    };
    Some(Card {
        title: format!("Tour {} / {}", wizard.step + 1, LESSONS.len()),
        // The queue lesson's toast is the first one the user sees.
        ticked: wizard.ticked.map(|i| if i == 6 { "Queued: that note top-right is a toast" } else { LESSONS[i].goal }),
        goal: lesson.goal,
        why: lesson.why,
        keys,
        note,
    })
}

/// Bottom-right, above the hint line, the player bar and the border (with its position
/// counter) of whatever is there: over the last rows of the rightmost pane (or of the
/// Player panel / track card), never over a whole pane.
fn card_area(state: &AppState, screen: Rect) -> Rect {
    let layout = state.settings.layout;
    let player = match layout {
        Layout::PlayerPanel if screen.width >= COLS_PANEL => 0,
        _ => player_size(layout == Layout::Classic, screen.height).height(),
    };
    let (width, height) = (CARD_WIDTH.min(screen.width), CARD_HEIGHT.min(screen.height));
    Rect {
        x: screen.right() - width,
        y: screen.bottom().saturating_sub(hints::rows(state, screen) + player + 1 + height).max(screen.y),
        width,
        height,
    }
}

fn draw_card(frame: &mut Frame, area: Rect, state: &AppState) {
    let Some(card) = card(state) else { return };
    let t = theme::current();
    let accent = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(t.dim);
    clear(frame, area);
    let block = Block::default()
        .title(Line::styled(format!(" {} ", card.title), accent))
        .title_bottom(Line::styled(" esc skips the tour ", dim).right_aligned())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(t.accent));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = [
        match card.ticked {
            Some(goal) => Line::from(vec![Span::styled("✓ ", accent), Span::styled(goal, dim)]),
            None => Line::styled("Do each step; the tour follows along.", dim),
        },
        Line::styled(card.goal, Style::new().fg(t.fg).add_modifier(Modifier::BOLD)),
        Line::styled(card.why, Style::new().fg(t.muted)),
        Line::from(card.keys),
        Line::styled(card.note, Style::new().fg(t.warning)),
    ];
    for (row, line) in inner.rows().zip(lines) {
        put(frame, row, line);
    }
}

/// The lesson as two full-width rows: progress, goal and (as much as fits of) why;
/// then the keys and whatever is in the way, with the last tick and how to skip at
/// the right when there is room.
pub fn draw_strip(frame: &mut Frame, area: Rect, state: &AppState) {
    let area = area.intersection(frame.area());
    let Some(card) = card(state) else { return };
    if area.height < STRIP_ROWS {
        return;
    }
    clear(frame, area);
    let t = theme::current();
    let accent = Style::new().fg(t.accent).add_modifier(Modifier::BOLD);
    let dim = Style::new().fg(t.dim);
    let head = format!(" {}  ", card.title);
    let room = (area.width as usize).saturating_sub(text_width(&head) + text_width(card.goal) + 3);
    put(frame, Rect { height: 1, ..area }, Line::from(vec![
        Span::styled(head, accent),
        Span::styled(card.goal, Style::new().fg(t.fg).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {}", fit(card.why, room)), Style::new().fg(t.muted)),
    ]));
    let mut keys = vec![Span::raw(" ")];
    keys.extend(card.keys);
    keys.push(Span::styled(format!("   {}", card.note), Style::new().fg(t.warning)));
    let keys = Line::from(keys);
    let second = Rect { y: area.y + 1, height: 1, ..area };
    // The right-hand end only where it does not run into the keys.
    let skip = "esc skips ".to_string();
    let ends = [card.ticked.map(|goal| format!("✓ {goal} · {skip}")), Some(skip)];
    let end = ends.into_iter().flatten().find(|end| keys.width() + text_width(end) + 2 <= area.width as usize);
    put(frame, second, keys);
    if let Some(end) = end {
        put_right(frame, second, &end, dim);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::logic::wizard::{self, Wizard};
    use crate::tui::render::testkit::{fixture, go, render_text};

    #[test]
    fn card_becomes_two_row_strip_when_narrow() {
        let (mut state, mut data, views, np) = fixture();
        wizard::start_practice(&mut state, &data);
        let goal = LESSONS[0].goal;
        // Wide and tall enough: a card in the corner, and the hint line as usual.
        let wide = render_text(120, 32, &mut state, &mut data, &views, &np);
        let lines: Vec<&str> = wide.lines().collect();
        assert!(wide.contains("╭ Tour 1 / 10 ") && wide.contains("esc skips the tour"), "{wide}");
        assert!(lines.iter().any(|l| l.contains(goal) && l.trim_end().ends_with('│')), "{wide}");
        assert!(lines[31].starts_with(" enter play"), "{wide}");
        assert!(wide.contains("j down, three times   0 / 3"), "{wide}");
        assert_eq!(hints::rows(&state, Rect::new(0, 0, 120, 32)), 1);

        // Under 100 columns, or 24 rows: two rows in the hint line's place, no box.
        for (w, h) in [(99, 32), (80, 24), (120, 23), (36, 8)] {
            let narrow = render_text(w, h, &mut state, &mut data, &views, &np);
            let lines: Vec<&str> = narrow.lines().collect();
            assert!(!narrow.contains("╭ Tour") && !narrow.contains("enter play"), "{w}x{h}:\n{narrow}");
            assert!(lines[h as usize - 2].starts_with(" Tour 1 / 10  Move down"), "{w}x{h}:\n{narrow}");
            assert!(lines[h as usize - 1].starts_with(" j down, three times"), "{w}x{h}:\n{narrow}");
            assert_eq!(hints::rows(&state, Rect::new(0, 0, w, h)), STRIP_ROWS);
        }
        assert_eq!(hints::rows(&state, Rect::new(0, 0, 100, 24)), 1);
        // The lesson shows even where the hint line would not.
        state.settings.hint_line = false;
        assert!(render_text(80, 24, &mut state, &mut data, &views, &np).contains(goal));
        assert_eq!(hints::rows(&state, Rect::new(0, 0, 120, 32)), 0);
        assert!(render_text(120, 32, &mut state, &mut data, &views, &np).contains("╭ Tour 1"));
    }

    #[test]
    fn card_reads_the_live_keymap_and_ticks_parts() {
        let (mut state, mut data, views, np) = fixture();
        state.wizard = Some(Wizard { step: 1, sub: 1, ticked: Some(0), ..Wizard::at(Stage::Practice) });
        state.keymap.apply_toml("[keys]\nbottom = \"B\"\ntop = []\n");
        let screen = render_text(120, 32, &mut state, &mut data, &views, &np);
        assert!(screen.contains("✓ Move down a few rows"), "{screen}");
        assert!(screen.contains("B bottom ✓   top: unbound · bind it in ?"), "{screen}");
        // An action the palette lists can be run from there.
        state.wizard = Some(Wizard { step: 6, ..Wizard::at(Stage::Practice) });
        state.keymap.apply_toml("[keys]\nadd_to_queue = []\n");
        let screen = render_text(120, 32, &mut state, &mut data, &views, &np);
        assert!(screen.contains("queue: unbound · run it from :, bind in ?"), "{screen}");
        // What is in the way: one pane, a container row, the wrong tab.
        state.wizard = Some(Wizard { step: 3, ..Wizard::at(Stage::Practice) });
        let screen = render_text(120, 32, &mut state, &mut data, &views, &np);
        assert!(screen.contains("] in   [ back") && screen.contains("try Library › Playlists"), "{screen}");
        go(&mut state, &mut data, 0, 1, 0);
        assert!(!render_text(120, 32, &mut state, &mut data, &views, &np).contains("try Library"));
        state.wizard = Some(Wizard { step: 4, ..Wizard::at(Stage::Practice) });
        assert!(render_text(80, 24, &mut state, &mut data, &views, &np).contains("enter play   This row opens"));
        state.wizard = Some(Wizard { step: 7, ticked: Some(6), ..Wizard::at(Stage::Practice) });
        go(&mut state, &mut data, 2, 0, 0);
        let screen = render_text(120, 32, &mut state, &mut data, &views, &np);
        assert!(screen.contains("✓ Queued: that note top-right is a toast") && screen.contains("Filters are for the Library"), "{screen}");
    }
}
