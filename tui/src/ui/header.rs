use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{palette::tailwind, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use lyre_core::player::PlaybackState;

use crate::app::App;
use crate::app_name::APP_NAME;

pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    let (state_label, state_style) = match app.player.state() {
        PlaybackState::Idle => ("idle", Style::new().fg(tailwind::SLATE.c400)),
        PlaybackState::Playing => {
            ("playing", Style::new().fg(tailwind::GREEN.c400).add_modifier(Modifier::BOLD))
        }
        PlaybackState::Paused => {
            ("paused", Style::new().fg(tailwind::AMBER.c400).add_modifier(Modifier::BOLD))
        }
    };

    let line = Line::from(vec![
        Span::raw(format!(" {APP_NAME} — {} song(s) — ", app.library.len())),
        Span::styled(state_label, state_style),
        Span::raw(format!(" — volume {:.0}% ", app.player.volume() * 100.0)),
    ]);

    Paragraph::new(line)
        .style(Style::new().fg(tailwind::SLATE.c100))
        .render(area, buf);
}
