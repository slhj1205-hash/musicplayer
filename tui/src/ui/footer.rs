use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Paragraph, Widget},
};

use crate::app::{App, StatusKind};
use crate::keymap;
use crate::theme;

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    let layout = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]);
    let [status_area, help_area] = area.layout(&layout);

    let status_style = match app.status.kind {
        StatusKind::Info => Style::new().fg(theme::STATUS_INFO),
        StatusKind::Success => Style::new().fg(theme::SUCCESS),
        StatusKind::Error => Style::new().fg(theme::ERROR).add_modifier(Modifier::BOLD),
    };
    let status_text = if app.status.kind == StatusKind::Error && !app.status.text.is_empty() {
        format!("⚠ {}", app.status.text)
    } else {
        app.status.text.clone()
    };
    Paragraph::new(status_text).style(status_style).render(status_area, buf);

    Paragraph::new(keymap::FOOTER_HINT)
        .style(Style::new().fg(theme::TEXT_DIM))
        .render(help_area, buf);
}
