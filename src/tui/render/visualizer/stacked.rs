use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Style},
    text::Line,
    widgets::{Block, Borders},
};
use crate::theme;

use super::common::frame_block;
use super::oscilloscope::{scope_chart, scope_dataset, scope_points};

/// The oscilloscope with L on top and R underneath instead of overlaid.
pub fn render_stacked_scope(frame: &mut Frame, area: Rect, samples: &[f32]) {
    let block = frame_block();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 {
        return;
    }

    let [top, rule, bottom] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
        Constraint::Fill(1),
    ])
    .areas(inner);

    let (left, right) = scope_points(samples);
    let max_x = left.len().max(1) as f64;

    frame.render_widget(
        scope_chart(vec![scope_dataset(&left, theme::current().accent)], max_x),
        top,
    );
    frame.render_widget(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(theme::current().dim))
            .title(Line::from(" L ↑ ").left_aligned())
            .title(Line::from(" ↓ R ").right_aligned()),
        rule,
    );
    frame.render_widget(
        scope_chart(vec![scope_dataset(&right, theme::current().secondary)], max_x),
        bottom,
    );
}
