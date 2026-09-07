use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::theme::{self, Theme};

use super::utils::centered_rect_fixed;

/// Theme chooser: one row per built-in theme with a swatch of its roles. The
/// highlighted theme is applied live as a preview.
pub fn render_theme_picker(frame: &mut Frame, selected: usize, active_name: &str) {
    let t = theme::current();
    let height = Theme::BUILTIN.len() as u16 + 5;
    let popup_area = centered_rect_fixed(52, height, frame.area());
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Theme ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded);
    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    let [list_area, _, hint_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let rows: Vec<Line> = Theme::BUILTIN
        .iter()
        .enumerate()
        .map(|(i, th)| {
            let is_sel = i == selected;
            let marker = if th.name == active_name { "● " } else { "  " };
            let name_style = if is_sel {
                Style::default().bg(t.selection_bg).fg(t.selection_fg).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg)
            };
            Line::from(vec![
                Span::styled(format!(" {marker}{:<18}", th.name), name_style),
                Span::raw(" "),
                Span::styled("██", Style::default().fg(th.accent)),
                Span::styled("██", Style::default().fg(th.secondary)),
                Span::styled("██", Style::default().fg(th.fg)),
                Span::styled("██", Style::default().fg(th.muted)),
                Span::styled("██", Style::default().fg(th.selection_bg)),
                Span::styled("██", Style::default().fg(th.warning)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(rows), list_area);
    frame.render_widget(
        Paragraph::new("↑↓ preview   Enter: keep   Esc: revert")
            .style(Style::default().fg(t.dim))
            .alignment(Alignment::Center),
        hint_area,
    );
}
