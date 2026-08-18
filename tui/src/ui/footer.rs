use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{palette::tailwind, Modifier, Style},
    widgets::{Paragraph, Widget},
};

use crate::app::{App, StatusKind};

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    let layout = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]);
    let [status_area, help_area] = area.layout(&layout);

    let status_style = match app.status.kind {
        StatusKind::Info => Style::new().fg(tailwind::SLATE.c300),
        StatusKind::Success => Style::new().fg(tailwind::GREEN.c400),
        StatusKind::Error => Style::new().fg(tailwind::RED.c400).add_modifier(Modifier::BOLD),
    };
    let status_text = if app.status.kind == StatusKind::Error && !app.status.text.is_empty() {
        format!("⚠ {}", app.status.text)
    } else {
        app.status.text.clone()
    };
    Paragraph::new(status_text).style(status_style).render(status_area, buf);

    let help = "<↑>/<↓> select · <Enter> play · <Space> pause · <Tab> playlists · <p> playlist actions · <q> quit · <?> for more";
    Paragraph::new(help)
        .style(Style::new().fg(tailwind::SLATE.c500))
        .render(help_area, buf);
}
