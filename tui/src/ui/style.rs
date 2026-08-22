use std::borrow::Cow;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, HighlightSpacing, List, ListItem},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme;

pub fn display_width(s: &str) -> usize {
    s.width()
}

pub fn focus_style() -> Style {
    Style::new().fg(theme::FOCUS)
}

pub fn unfocused_style() -> Style {
    Style::new().fg(theme::TEXT_SECONDARY)
}

pub fn content_style() -> Style {
    Style::new().fg(theme::TEXT_SECONDARY)
}

pub fn modal_body_style() -> Style {
    Style::new().fg(theme::TEXT_PRIMARY)
}

fn title_style(border_style: Style) -> Style {
    border_style.add_modifier(Modifier::BOLD)
}

pub fn titled_block(title: impl Into<String>, border_style: Style) -> Block<'static> {
    Block::bordered()
        .title(Span::styled(title.into(), title_style(border_style)))
        .border_style(border_style)
}

pub fn titled_block_split(left: Line<'static>, right: Line<'static>, border_style: Style) -> Block<'static> {
    Block::bordered()
        .title(left.alignment(Alignment::Left))
        .title(right.alignment(Alignment::Right))
        .border_style(border_style)
}

pub fn modal_block(title: impl Into<String>) -> Block<'static> {
    let border_style = focus_style();
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .style(Style::new().bg(theme::MODAL_BACKGROUND))
        .title(Span::styled(title.into(), title_style(border_style)))
        .title_alignment(Alignment::Center)
}

pub fn selected_style() -> Style {
    Style::new().bg(theme::SELECTED_BACKGROUND).add_modifier(Modifier::BOLD)
}

pub fn marker_style(selected: bool) -> (&'static str, Style) {
    if selected {
        ("▶ ", selected_style())
    } else {
        ("  ", content_style())
    }
}

pub fn styled_list<'a>(items: Vec<ListItem<'a>>, block: Block<'a>) -> List<'a> {
    List::new(items)
        .block(block)
        .highlight_style(selected_style())
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always)
}

pub fn plural(count: usize, suffix: &str) -> &str {
    if count == 1 {
        ""
    } else {
        suffix
    }
}

fn sort_label_width() -> usize {
    let fixed = display_width(&format!(" <o> group: {}  <p> sort: {} ", "", ""));
    let max_category = crate::app::Category::ALL.iter().map(|c| display_width(c.label())).max().unwrap_or(0);
    let max_sort = crate::app::Sort::ALL.iter().map(|s| display_width(s.label())).max().unwrap_or(0);
    fixed + max_category + max_sort
}

pub fn search_title(
    title_prefix: &str,
    searching: bool,
    query: &str,
    match_count: usize,
    border_style: Style,
) -> Line<'static> {
    let matches = format!("{match_count} match{}", plural(match_count, "es"));

    let search_segment = if searching {
        if query.is_empty() {
            "</> ▏".to_string()
        } else {
            format!("</> {query}▏ {matches}")
        }
    } else if !query.is_empty() {
        format!("\"{query}\" · {matches}")
    } else {
        "</> search".to_string()
    };

    Line::styled(format!(" {title_prefix} — {search_segment} "), title_style(border_style))
}

pub fn sort_title(category_label: &str, sort_label: &str, border_style: Style) -> Line<'static> {
    let text = format!(" <o> group: {category_label}  <p> sort: {sort_label} ");
    let fill_len = sort_label_width().saturating_sub(display_width(&text));
    let fill = "─".repeat(fill_len);

    Line::from(vec![Span::styled(text, title_style(border_style)), Span::styled(fill, border_style)])
}

pub fn dim_area(area: Rect, buf: &mut Buffer) {
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_fg(theme::DIM_FOREGROUND);
                cell.set_bg(theme::DIM_BACKGROUND);
            }
        }
    }
}

pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

pub fn side_by_side_rect(base_width: u16, side_width: u16, height: u16, has_side: bool, area: Rect) -> (Rect, Rect) {
    let gap: u16 = 2;
    let total_width = if has_side { base_width.saturating_add(gap).saturating_add(side_width) } else { base_width };
    let total_width = total_width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(total_width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;

    let base_width = base_width.min(total_width);
    let base = Rect { x, y, width: base_width, height };
    let side = Rect { x: x + base_width + gap, y, width: side_width.min(total_width.saturating_sub(base_width + gap)), height };
    (base, side)
}

const MARQUEE_PAUSE_MS: u128 = 4500;
const MARQUEE_STEP_MS: u128 = 150;
const MARQUEE_GAP: &str = "    ";

thread_local! {
    static MARQUEE_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn reset_marquee_activity() {
    MARQUEE_ACTIVE.with(|c| c.set(false));
}

pub fn marquee_active() -> bool {
    MARQUEE_ACTIVE.with(|c| c.get())
}

pub fn marquee_window(text: &str, visible_width: usize) -> Cow<'_, str> {
    if visible_width == 0 {
        return Cow::Borrowed("");
    }

    if display_width(text) <= visible_width {
        return Cow::Borrowed(text);
    }

    let chars: Vec<char> = text.chars().collect();

    MARQUEE_ACTIVE.with(|c| c.set(true));

    let gap: Vec<char> = MARQUEE_GAP.chars().collect();
    let loop_len = chars.len() + gap.len();

    let cycle_ms = MARQUEE_PAUSE_MS + loop_len as u128 * MARQUEE_STEP_MS;

    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let phase = now_ms % cycle_ms;

    let offset = if phase < MARQUEE_PAUSE_MS {
        0
    } else {
        (((phase - MARQUEE_PAUSE_MS) / MARQUEE_STEP_MS) as usize) % loop_len
    };

    if offset == 0 {
        let ellipsis_width = '…'.width().unwrap_or(1);
        let show_ellipsis = visible_width >= ellipsis_width;
        let budget = if show_ellipsis { visible_width - ellipsis_width } else { visible_width };
        let mut truncated = String::new();
        let mut used = 0usize;
        for &c in &chars {
            let w = c.width().unwrap_or(0);
            if used + w > budget {
                break;
            }
            truncated.push(c);
            used += w;
        }
        if show_ellipsis {
            truncated.push('…');
        }
        return Cow::Owned(truncated);
    }

    let mut out = String::new();
    let mut used = 0usize;
    let mut i = 0usize;
    while used < visible_width && i <= loop_len {
        let idx = (offset + i) % loop_len;
        let c = if idx < chars.len() { chars[idx] } else { gap[idx - chars.len()] };
        let w = c.width().unwrap_or(0);
        if used + w > visible_width {
            break;
        }
        out.push(c);
        used += w;
        i += 1;
    }
    Cow::Owned(out)
}

pub fn format_duration(d: Duration) -> String {
    let total = d.as_secs();
    format!("{:02}:{:02}", total / 60, total % 60)
}

pub fn format_mtime(mtime_secs: u64) -> String {
    if mtime_secs == 0 {
        return "unknown".to_string();
    }

    let days = (mtime_secs / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}")
}
