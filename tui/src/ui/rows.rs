use lyre_core::{Library, Song, SongId};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, ListItem, ListState, Paragraph, StatefulWidget, Widget},
};

use crate::{
    app::{PlaylistDisplayMode, Row},
    theme,
};

use super::{
    display_width, focus_style, marquee_window, search_title, sort_title, styled_list, titled_block_split,
    unfocused_style,
};

const CHROME_WIDTH: usize = 4;

const TITLE_WIDTH_RATIO: usize = 70;
const ARTIST_WIDTH_RATIO: usize = 30;
const MIN_MARQUEE_WIDTH: usize = 6;

fn header_style() -> Style {
    Style::new().fg(theme::SECTION_HEADER).add_modifier(Modifier::BOLD)
}

fn title_style(is_current: bool) -> Style {
    if is_current {
        Style::new().fg(theme::TITLE_CURRENT).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme::TITLE)
    }
}

fn artist_style() -> Style {
    Style::new().fg(theme::ARTIST)
}

fn detail_style() -> Style {
    Style::new().fg(theme::DETAIL)
}

fn playlist_style() -> Style {
    Style::new().fg(theme::PLAYLIST_TAG)
}

fn separator_style() -> Style {
    Style::new().fg(theme::SEPARATOR)
}

fn now_playing_marker_style(is_current: bool) -> Style {
    if is_current {
        Style::new().fg(theme::TITLE_CURRENT).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(theme::NOW_PLAYING_MARKER_IDLE)
    }
}

fn missing_style() -> Style {
    Style::new().fg(theme::MISSING_SONG)
}

pub struct SongLabel<'a> {
    pub title: &'a str,
    pub artist: Option<&'a str>,
    pub detail: Option<String>,
}

pub fn label_for(song: &Song, category: crate::app::Category, sort: crate::app::Sort) -> SongLabel<'_> {
    use crate::app::{Category, Sort};

    let detail = match sort {
        Sort::Duration => Some(super::format_duration(song.metadata().duration)),
        Sort::DateModified => Some(super::format_mtime(song.modified())),
        Sort::Title | Sort::Artist | Sort::Path => None,
    };
    let artist = if category == Category::Artist { None } else { Some(song.artist()) };

    SongLabel { title: song.title(), artist, detail }
}

pub fn filtered_label_for(song: &Song, sort: crate::app::Sort) -> SongLabel<'_> {
    use crate::app::Sort;

    let detail = match sort {
        Sort::Duration => Some(super::format_duration(song.metadata().duration)),
        Sort::DateModified => Some(super::format_mtime(song.modified())),
        Sort::Title | Sort::Artist | Sort::Path => None,
    };

    SongLabel { title: song.title(), artist: Some(song.artist()), detail }
}

fn playlist_suffix(names: &[String], mode: PlaylistDisplayMode, available: usize) -> Option<String> {
    if names.is_empty() {
        return None;
    }
    match mode {
        PlaylistDisplayMode::Hidden => None,
        PlaylistDisplayMode::Count => Some(format!("󰲹 {}", names.len())),
        PlaylistDisplayMode::Expanded => {
            if available == 0 {
                return None;
            }
            let joined = names.join(", ");
            Some(marquee_window(&joined, available).into_owned())
        }
    }
}

pub fn song_count(rows: &[Row]) -> usize {
    rows.iter().filter(|r| matches!(r, Row::Song(_, _))).count()
}

pub fn keep_header_in_view(list_state: &mut ListState, rows: &[Row]) {
    if let Some(selected) = list_state.selected()
        && selected > 0 && matches!(rows.get(selected - 1), Some(Row::Header(_))) && list_state.offset() > selected - 1 {
            *list_state.offset_mut() = selected - 1;
        }
}

pub type PlaylistLookup<'a> = (PlaylistDisplayMode, &'a dyn Fn(SongId) -> Vec<String>);

pub struct Viewport {
    pub start: usize,
    pub end: usize,
    pub selected: Option<usize>,
}

pub struct PanelHeight(pub usize);

pub fn viewport(list_state: &mut ListState, len: usize, height: usize) -> Viewport {
    if len == 0 || height == 0 {
        return Viewport { start: 0, end: 0, selected: None };
    }

    let selected = list_state.selected().filter(|&i| i < len);
    let mut offset = list_state.offset().min(len.saturating_sub(1));

    if let Some(sel) = selected {
        if sel < offset {
            offset = sel;
        } else if sel >= offset + height {
            offset = sel + 1 - height;
        }
    }
    offset = offset.min(len.saturating_sub(height.min(len)));

    *list_state.offset_mut() = offset;

    let end = (offset + height).min(len);
    Viewport { start: offset, end, selected: selected.map(|s| s - offset) }
}

pub fn song_list_items<'a>(
    rows: &[Row],
    current: Option<SongId>,
    library: &'a Library,
    mut label: impl for<'s> FnMut(&'s Song) -> SongLabel<'s>,
    available_width: usize,
    playlist_info: Option<PlaylistLookup<'_>>,
) -> Vec<ListItem<'a>> {
    let content_width = available_width.saturating_sub(CHROME_WIDTH);

    rows.iter()
        .map(|row| match row {
            Row::Header(heading) => ListItem::new(heading.clone()).style(header_style()),
            Row::Song(id, depth) => {
                let is_current = Some(*id) == current;
                let indent = "  ".repeat(*depth);
                let marker = if is_current { "♪ " } else { "  " };
                let prefix_width = display_width(&indent) + display_width(marker);
                let mut used = prefix_width;

                let mut spans: Vec<Span> = Vec::new();
                if !indent.is_empty() {
                    spans.push(Span::raw(indent));
                }
                spans.push(Span::styled(marker, now_playing_marker_style(is_current)));

                match library.get(*id) {
                    Some(song) => {
                        let SongLabel { title, artist, detail } = label(song);

                        let text_budget = content_width.saturating_sub(prefix_width);
                        let title_max =
                            ((text_budget * TITLE_WIDTH_RATIO) / 100).max(MIN_MARQUEE_WIDTH.min(text_budget));
                        let artist_max =
                            ((text_budget * ARTIST_WIDTH_RATIO) / 100).max(MIN_MARQUEE_WIDTH.min(text_budget));

                        let title_text = marquee_window(title, title_max);
                        used += display_width(&title_text);
                        spans.push(Span::styled(title_text, title_style(is_current)));

                        if let Some(artist) = artist {
                            let sep = " — ";
                            used += display_width(sep);
                            spans.push(Span::styled(sep, separator_style()));

                            let artist_text = marquee_window(artist, artist_max);
                            used += display_width(&artist_text);
                            spans.push(Span::styled(artist_text, artist_style()));
                        }

                        if let Some(detail) = detail {
                            let text = format!(" ({detail})");
                            used += display_width(&text);
                            spans.push(Span::styled(text, detail_style()));
                        }

                        if let Some((mode, lookup)) = playlist_info
                            && mode != PlaylistDisplayMode::Hidden {
                                let names = lookup(*id);
                                let sep = " · ";
                                let remaining = content_width.saturating_sub(used + display_width(sep));
                                if let Some(suffix) = playlist_suffix(&names, mode, remaining) {
                                    spans.push(Span::styled(sep, separator_style()));
                                    spans.push(Span::styled(suffix, playlist_style()));
                                }
                            }
                    }
                    None => spans.push(Span::styled("<missing>", missing_style())),
                }

                ListItem::new(Line::from(spans))
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn render_song_list_panel(
    area: Rect,
    buf: &mut Buffer,
    list_state: &mut ListState,
    rows: &[Row],
    current: Option<SongId>,
    library: &Library,
    label: impl for<'s> FnMut(&'s Song) -> SongLabel<'s>,
    title_prefix: &str,
    category_label: &str,
    sort_label: &str,
    searching: bool,
    query: &str,
    playlist_info: Option<PlaylistLookup<'_>>,
) -> PanelHeight {
    keep_header_in_view(list_state, rows);

    let match_count = song_count(rows);
    let border_style = if searching { focus_style() } else { unfocused_style() };
    let left_title = search_title(title_prefix, searching, query, match_count, border_style);
    let right_title = sort_title(category_label, sort_label, border_style);
    let block = titled_block_split(left_title, right_title, border_style);

    let inner_height = block.inner(area).height as usize;

    if match_count == 0 && !query.is_empty() {
        list_state.select(None);
        render_no_matches(area, buf, block, query, "songs");
        return PanelHeight(inner_height);
    }

    let window = viewport(list_state, rows.len(), inner_height);

    let items = song_list_items(
        &rows[window.start..window.end],
        current,
        library,
        label,
        area.width as usize,
        playlist_info,
    );

    let mut local = ListState::default().with_offset(0);
    local.select(window.selected);

    let list = styled_list(items, block);
    StatefulWidget::render(list, area, buf, &mut local);

    PanelHeight(inner_height)
}

pub fn render_no_matches(area: Rect, buf: &mut Buffer, block: Block<'static>, query: &str, noun: &str) {
    let inner = block.inner(area);
    Widget::render(block, area, buf);

    if inner.height == 0 {
        return;
    }

    let message_area = Rect {
        x: inner.x,
        y: inner.y + inner.height.saturating_sub(1) / 2,
        width: inner.width,
        height: 1,
    };

    Paragraph::new(format!("No {noun} match \"{query}\""))
        .style(Style::new().fg(theme::EMPTY_STATE).add_modifier(Modifier::ITALIC))
        .alignment(Alignment::Center)
        .render(message_area, buf);
}
