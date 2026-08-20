use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{palette::tailwind, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Widget},
};

use crate::app::{App, YoutubeField, YoutubeModal};

use super::{centered_rect, dim_area, format_duration, modal_block, modal_body_style};

const WIDTH: u16 = 56;

fn label_style() -> Style {
    Style::new().fg(tailwind::SLATE.c400)
}

fn value_style(focused: bool) -> Style {
    if focused {
        Style::new().fg(tailwind::SLATE.c100).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(tailwind::SLATE.c200)
    }
}

fn error_style() -> Style {
    Style::new().fg(tailwind::RED.c400)
}

fn hint_style() -> Style {
    Style::new().fg(tailwind::SLATE.c400)
}

fn render_lines(title: &str, lines: Vec<Line<'static>>, full_area: Rect, buf: &mut Buffer) {
    dim_area(full_area, buf);

    let height = lines.len() as u16 + 2;
    let popup = centered_rect(WIDTH, height, full_area);
    Widget::render(Clear, popup, buf);

    Paragraph::new(Text::from(lines))
        .style(modal_body_style())
        .block(modal_block(title).padding(ratatui::widgets::Padding::horizontal(2)))
        .render(popup, buf);
}

pub fn render(app: &App, full_area: Rect, buf: &mut Buffer) {
    let Some(modal) = &app.modal.youtube_modal else { return };

    match modal {
        YoutubeModal::EnteringUrl { url_input, error } => render_entering_url(url_input, error.as_deref(), full_area, buf),
        YoutubeModal::Fetching { url } => render_fetching(url, full_area, buf),
        YoutubeModal::ConfirmingVideo { info, .. } => render_confirming_video(info, full_area, buf),
        YoutubeModal::EditingFields(fields) => render_editing_fields(fields, full_area, buf),
        YoutubeModal::ResolvingCollision { existing_path, .. } => render_resolving_collision(existing_path, full_area, buf),
        YoutubeModal::Downloading { file_name, .. } => render_downloading(file_name, full_area, buf),
    }
}

fn render_entering_url(url_input: &str, error: Option<&str>, full_area: Rect, buf: &mut Buffer) {
    let mut lines = vec![
        Line::raw(""),
        Line::from(vec![Span::styled(format!("{url_input}▏"), value_style(true))]),
        Line::raw(""),
    ];
    if let Some(error) = error {
        lines.push(Line::from(error.to_string()).alignment(Alignment::Center).style(error_style()));
        lines.push(Line::raw(""));
    }
    lines.push(Line::from("<Enter> fetch · <Esc> cancel").alignment(Alignment::Center).style(hint_style()));

    render_lines(" Download from YouTube ", lines, full_area, buf);
}

fn render_fetching(url: &str, full_area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::raw(""),
        Line::from("fetching video info…").alignment(Alignment::Center),
        Line::from(url.to_string()).alignment(Alignment::Center).style(hint_style()),
        Line::raw(""),
    ];

    render_lines(" Download from YouTube ", lines, full_area, buf);
}

fn render_confirming_video(info: &lyre_core::youtube::VideoInfo, full_area: Rect, buf: &mut Buffer) {
    let uploader = info.uploader.as_deref().unwrap_or("unknown");
    let duration = info.duration.map(format_duration).unwrap_or_else(|| "unknown".to_string());

    let lines = vec![
        Line::raw(""),
        Line::from(info.title.clone()).alignment(Alignment::Center).style(value_style(true)),
        Line::from(format!("by {uploader} · {duration}")).alignment(Alignment::Center).style(hint_style()),
        Line::raw(""),
        Line::from("is this the right video?").alignment(Alignment::Center),
        Line::raw(""),
        Line::from(vec![
            Span::styled("<y>", Style::new().fg(tailwind::GREEN.c400).add_modifier(Modifier::BOLD)),
            Span::raw(" yes      "),
            Span::styled("<n>", Style::new().fg(tailwind::RED.c400).add_modifier(Modifier::BOLD)),
            Span::raw(" no"),
        ])
        .alignment(Alignment::Center),
    ];

    render_lines(" Confirm Video ", lines, full_area, buf);
}

fn render_editing_fields(fields: &crate::app::YoutubeFieldsModal, full_area: Rect, buf: &mut Buffer) {
    let mut lines: Vec<Line> = Vec::with_capacity(YoutubeField::ALL.len() + 5);
    lines.push(Line::raw(""));

    for &field in YoutubeField::ALL {
        let focused = field == fields.focused;
        let cursor = if focused { "▏" } else { "" };
        let label = format!("{:<10}", field.label());
        lines.push(Line::from(vec![
            Span::styled(label, label_style()),
            Span::styled(format!("{}{cursor}", field.value(fields)), value_style(focused)),
        ]));
    }

    lines.push(Line::raw(""));
    if let Some(error) = &fields.error {
        lines.push(Line::from(error.clone()).alignment(Alignment::Center).style(error_style()));
        lines.push(Line::raw(""));
    }
    lines.push(
        Line::from("<Tab>/<Shift+Tab> field · <Enter> download · <Esc> cancel")
            .alignment(Alignment::Center)
            .style(hint_style()),
    );

    render_lines(" Download from YouTube ", lines, full_area, buf);
}

fn render_resolving_collision(existing_path: &std::path::Path, full_area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::raw(""),
        Line::from(existing_path.display().to_string()).alignment(Alignment::Center),
        Line::from("already exists").alignment(Alignment::Center),
        Line::raw(""),
        Line::from(vec![
            Span::styled("<o>", Style::new().fg(tailwind::AMBER.c400).add_modifier(Modifier::BOLD)),
            Span::raw(" overwrite      "),
            Span::styled("<r>", Style::new().fg(tailwind::CYAN.c400).add_modifier(Modifier::BOLD)),
            Span::raw(" rename"),
        ])
        .alignment(Alignment::Center),
    ];

    render_lines(" File Exists ", lines, full_area, buf);
}

fn render_downloading(file_name: &str, full_area: Rect, buf: &mut Buffer) {
    let lines = vec![
        Line::raw(""),
        Line::from("downloading…").alignment(Alignment::Center),
        Line::from(file_name.to_string()).alignment(Alignment::Center).style(hint_style()),
        Line::raw(""),
    ];

    render_lines(" Download from YouTube ", lines, full_area, buf);
}
