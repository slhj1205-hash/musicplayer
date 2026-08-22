use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Clear, Paragraph, Widget},
};

use crate::app::{App, MetadataField};
use crate::theme;

use super::{centered_rect, dim_area, modal_block, modal_body_style};

const WIDTH: u16 = 60;

fn label_style() -> Style {
    Style::new().fg(theme::TEXT_MUTED)
}

fn value_style(focused: bool) -> Style {
    if focused {
        Style::new().fg(theme::TEXT_PRIMARY).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme::TEXT_SECONDARY)
    }
}

fn error_style() -> Style {
    Style::new().fg(theme::ERROR)
}

pub fn render(app: &App, full_area: Rect, buf: &mut Buffer) {
    dim_area(full_area, buf);

    let Some(modal) = &app.modal.metadata_modal else { return };

    let visible = MetadataField::visible(&modal.edits);
    let mut lines: Vec<Line> = Vec::with_capacity(visible.len() + 5);
    lines.push(Line::raw(""));

    for field in visible {
        let focused = field == modal.focused;
        let cursor = if focused { "▏" } else { "" };
        let label = format!("{:<16}", field.label());
        lines.push(Line::from(vec![
            Span::styled(label, label_style()),
            Span::styled(format!("{}{cursor}", field.value(&modal.edits)), value_style(focused)),
        ]));
    }

    lines.push(Line::raw(""));
    if let Some(error) = &modal.error {
        lines.push(Line::from(error.as_str()).alignment(Alignment::Center).style(error_style()));
        lines.push(Line::raw(""));
    }
    lines.push(
        Line::from("<Tab>/<Shift+Tab> field · <Enter> save · <Esc> cancel")
            .alignment(Alignment::Center)
            .style(Style::new().fg(theme::TEXT_MUTED)),
    );

    let height = lines.len() as u16 + 2;
    let popup = centered_rect(WIDTH, height, full_area);
    Widget::render(Clear, popup, buf);

    Paragraph::new(Text::from(lines))
        .style(modal_body_style())
        .block(modal_block(" Edit Metadata ").padding(ratatui::widgets::Padding::horizontal(2)))
        .render(popup, buf);
}
