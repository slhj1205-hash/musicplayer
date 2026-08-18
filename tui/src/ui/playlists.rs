use lyre_core::PlaylistId;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{ListItem, ListState, StatefulWidget},
};

use crate::{
    app::{App, PlaylistView},
    theme,
};

use super::{
    focus_style, label_for, plural, render_no_matches, render_song_list_panel, search_title, styled_list,
    titled_block, titled_block_split, unfocused_style, viewport,
};

fn playlist_name_style() -> Style {
    Style::new().fg(theme::PLAYLIST_NAME)
}

fn playlist_count_style() -> Style {
    Style::new().fg(theme::PLAYLIST_SONG_COUNT)
}

pub fn render(app: &mut App, area: Rect, buf: &mut Buffer) {
    match app.playlist_panel.view {
        PlaylistView::Browsing => render_browsing(app, area, buf),
        PlaylistView::Viewing(id) => render_viewing(app, id, area, buf),
    }
}

fn render_browsing(app: &mut App, area: Rect, buf: &mut Buffer) {
    let ids = app.visible_playlist_ids();
    let match_count = ids.len();
    let empty_store = app.playlists.is_empty();
    let searching = app.playlist_panel.searching;
    let query_empty = app.playlist_panel.search_query.is_empty();
    let border_style = if searching { focus_style() } else { unfocused_style() };

    let block = if empty_store && !searching && query_empty {
        let key = crate::keymap::display_for(crate::keymap::Action::OpenSongModal);
        titled_block(format!(" Playlists — none yet, press {key} on a song to create one "), unfocused_style())
    } else {
        let left_title = search_title("Playlists", searching, &app.playlist_panel.search_query, match_count, border_style);
        titled_block_split(left_title, Line::from(""), border_style)
    };

    let inner_height = block.inner(area).height as usize;
    app.playlist_panel.page_height = inner_height;

    if !empty_store && match_count == 0 && !query_empty {
        app.playlist_panel.list_state.select(None);
        render_no_matches(area, buf, block, &app.playlist_panel.search_query, "playlists");
        return;
    }

    let window = viewport(&mut app.playlist_panel.list_state, ids.len(), inner_height);

    let items: Vec<ListItem> = ids[window.start..window.end]
        .iter()
        .map(|&id| {
            let playlist = app.playlists.get(id);
            let name = playlist.map(|p| p.name()).unwrap_or("<unknown>");
            let count = playlist.map(|p| p.len()).unwrap_or(0);
            let count_text = format!("  ({count} song{})", plural(count, "s"));
            ListItem::new(Line::from(vec![
                Span::styled(name.to_string(), playlist_name_style()),
                Span::styled(count_text, playlist_count_style()),
            ]))
        })
        .collect();

    let mut local = ListState::default().with_offset(0);
    local.select(window.selected);

    let list = styled_list(items, block);
    StatefulWidget::render(list, area, buf, &mut local);
}

fn render_viewing(app: &mut App, id: PlaylistId, area: Rect, buf: &mut Buffer) {
    let name = app.playlists.get(id).map(|p| p.name().to_string()).unwrap_or_else(|| "<deleted>".to_string());
    app.visible_rows();
    let rows = app.rows.rows_unchecked();
    let current = app.queue.current_id();

    let category = app.playlist_panel.category;
    let sort = app.playlist_panel.sort;
    app.playlist_panel.page_height = render_song_list_panel(
        area,
        buf,
        &mut app.playlist_panel.list_state,
        rows,
        current,
        &app.library,
        |song| label_for(song, category, sort),
        &name,
        app.playlist_panel.category.label(),
        app.playlist_panel.sort.label(),
        app.playlist_panel.searching,
        &app.playlist_panel.search_query,
        None,
    );
}
