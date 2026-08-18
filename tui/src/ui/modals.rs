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

    let backend_note = "gstreamer (real audio)".to_string();

    let mut lines = vec![
        Line::styled("Global", header_style),
        row("<Tab>", "Switch between Library / Playlists"),
        row("<↑>/<↓>, <j>/<k>", "Move selection"),
        row("<Ctrl+d> / <Ctrl+u>", "Jump a page down / up"),
        row("<g> / <Shift+G>", "Jump to top / bottom"),
        row("<c>", "Jump to now playing"),
        row("<Enter>", "Play selected song / open selected playlist"),
        row("<Space>", "Pause / resume"),
        row("<n>", "Next track"),
        row("<1-9> then <n>", "Jump to Nth song in Up Next (<Esc> cancels)"),
        row("<b>", "Previous track"),
        row("<a>", "Queue selected song next"),
        row("<s>", "Shuffle"),
        row("<u>", "Un-shuffle"),
        row("<[> / <]>", "Volume down / up"),
        row("<Shift+A>", "Song actions: add to / create playlist"),
        row("<d>", "Change directory (used by both Library and Playlists)"),
        row("<q>, <Esc>", "Quit (with confirmation)"),
        row("<?>", "Toggle this help"),
        Line::raw(""),
        Line::styled("Library", header_style),
        row("</>", "Search the library (live filter)"),
        row("<o> / <Shift+O>", "Cycle library category (grouping)"),
        row("<p> / <Shift+P>", "Cycle library sort (order within group)"),
        row("<m>", "Cycle playlist display: hidden / count / names"),
        Line::raw(""),
        Line::styled("Playlists", header_style),
        row("</>", "Search by name / within the open playlist (live filter)"),
        row("<Enter>", "Open playlist / play selected song within it"),
        row("<Esc>", "Back to playlist browser"),
        row("<o> / <Shift+O>", "Cycle category within the open playlist"),
        row("<p> / <Shift+P>", "Cycle sort within the open playlist"),
        row("<r>", "Remove selected song from playlist (confirm)"),
        Line::raw(""),
        Line::styled(format!("  Backend: {backend_note}"), note_style),
        Line::styled("  Mouse: not supported", note_style),
        Line::raw(""),
        Line::from("press any key to close").alignment(Alignment::Center).style(note_style),
    ];

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
