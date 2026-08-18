use lyre_core::SongId;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::app::App;

use super::{label_for, render_song_list_panel};

pub fn render(app: &mut App, area: Rect, buf: &mut Buffer) {
    let current = app.queue.current_id();

    app.visible_rows();
    let rows = app.rows.rows_unchecked();

    let category = app.library_panel.category;
    let sort = app.library_panel.sort;
    let playlist_mode = app.library_panel.playlist_mode;
    let playlists = &app.playlists;

    let playlist_names = |song_id: SongId| -> Vec<String> {
        playlists
            .containing(song_id)
            .iter()
            .filter_map(|&id| playlists.get(id).map(|p| p.name().to_string()))
            .collect()
    };

    app.library_panel.page_height = render_song_list_panel(
        area,
        buf,
        &mut app.library_panel.list_state,
        rows,
        current,
        &app.library,
        |song| label_for(song, category, sort),
        "Library",
        app.library_panel.category.label(),
        app.library_panel.sort.label(),
        app.library_panel.searching,
        &app.library_panel.search_query,
        Some((playlist_mode, &playlist_names)),
    );
}
