use lyre_core::{PlaylistId, SongId};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{palette::tailwind, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Widget},
};

use crate::app::App;
use crate::app_name::APP_NAME;
use crate::keymap::{self, Section};

use super::{centered_rect, dim_area, modal_block, modal_body_style};

fn key_style() -> Style {
    Style::new().fg(tailwind::CYAN.c300).add_modifier(Modifier::BOLD)
}

fn yes_no_line() -> Line<'static> {
    Line::from(vec![
        Span::styled("<y>", Style::new().fg(tailwind::GREEN.c400).add_modifier(Modifier::BOLD)),
        Span::raw(" yes      "),
        Span::styled("<n>", Style::new().fg(tailwind::RED.c400).add_modifier(Modifier::BOLD)),
        Span::raw(" no"),
    ])
    .alignment(Alignment::Center)
}

fn render_confirm(title: &str, lines: Vec<Line<'static>>, width: u16, height: u16, full_area: Rect, buf: &mut Buffer) {
    dim_area(full_area, buf);

    let popup = centered_rect(width, height, full_area);
    Widget::render(Clear, popup, buf);

    Paragraph::new(Text::from(lines))
        .style(modal_body_style())
        .block(modal_block(title))
        .render(popup, buf);
}

pub fn render_quit_confirm(full_area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::raw(""),
        Line::from(format!("Quit {APP_NAME}?")).alignment(Alignment::Center),
        Line::raw(""),
        yes_no_line(),
    ];

    render_confirm(" Quit ", lines, 40, 7, full_area, buf);
}

pub fn render_remove_confirm(app: &App, playlist_id: PlaylistId, song_id: SongId, full_area: Rect, buf: &mut Buffer) {
    let song_label = app.library.get(song_id).map(|s| s.to_string()).unwrap_or_else(|| "this song".to_string());
    let playlist_label =
        app.playlists.get(playlist_id).map(|p| p.name().to_string()).unwrap_or_else(|| "this playlist".to_string());

    let lines = vec![
        Line::raw(""),
        Line::from(song_label).alignment(Alignment::Center),
        Line::from(format!("Remove from \"{playlist_label}\"?")).alignment(Alignment::Center),
        Line::raw(""),
        yes_no_line(),
    ];

    render_confirm(" Remove Song ", lines, 46, 8, full_area, buf);
}

pub fn render_romanized_artist_confirm(app: &App, full_area: Rect, buf: &mut Buffer) {
    let Some(confirm) = &app.modal.romanized_artist_confirm else { return };

    let plural = if confirm.count == 1 { "" } else { "s" };
    let lines = vec![
        Line::raw(""),
        Line::from(format!("Apply \"{}\" as the romanized artist", confirm.value)).alignment(Alignment::Center),
        Line::from(format!("to {} other song{plural} by {}?", confirm.count, confirm.artist_display))
            .alignment(Alignment::Center),
        Line::raw(""),
        yes_no_line(),
    ];

    render_confirm(" Apply Romanized Artist ", lines, 54, 8, full_area, buf);
}

pub fn render_help_overlay(full_area: Rect, buf: &mut Buffer) {
    dim_area(full_area, buf);

    let header_style = Style::new().fg(tailwind::AMBER.c400).add_modifier(Modifier::BOLD);
    let note_style = Style::new().fg(tailwind::SLATE.c400);

    let row = |key: &str, desc: &str| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!("  {key:<20}"), key_style()),
            Span::raw(desc.to_string()),
        ])
    };

    let section_lines = |section: Section| -> Vec<Line<'static>> {
        keymap::help_rows(section).into_iter().map(|(key, desc)| row(&key, desc)).collect()
    };

    let backend_note = "gstreamer (real audio)".to_string();

    let mut lines = vec![Line::styled("Global", header_style)];
    lines.extend(section_lines(Section::Global));
    lines.push(Line::raw(""));
    lines.push(Line::styled("Library", header_style));
    lines.extend(section_lines(Section::Library));
    lines.push(Line::raw(""));
    lines.push(Line::styled("Playlists", header_style));
    lines.extend(section_lines(Section::Playlists));
    lines.extend([
        Line::raw(""),
        Line::styled(format!("  Backend: {backend_note}"), note_style),
        Line::styled("  Mouse: not supported", note_style),
        Line::raw(""),
        Line::from("press any key to close").alignment(Alignment::Center).style(note_style),
    ]);

    let height = (lines.len() as u16 + 2).min(full_area.height);
    let popup = centered_rect(58, height, full_area);
    Widget::render(Clear, popup, buf);

    let visible_lines = popup.height.saturating_sub(2) as usize;
    lines.truncate(visible_lines);

    Paragraph::new(Text::from(lines))
        .style(modal_body_style())
        .block(modal_block(" Help "))
        .render(popup, buf);
}
