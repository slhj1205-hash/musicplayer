use lyre_core::SongId;
use ratatui::{buffer::Buffer, layout::Rect};

use crate::app::{is_filtering, App};

use super::{filtered_label_for, label_for, render_song_list_panel, PanelHeight};

pub fn render(app: &mut App, area: Rect, buf: &mut Buffer) {
    let current = app.queue.current_id();

    app.visible_rows();
    let rows = app.rows.rows_unchecked();

    let category = app.library_panel.category;
    let sort = app.library_panel.sort;
    let filtering = is_filtering(&app.library_panel.search_query);
    let playlist_mode = app.library_panel.playlist_mode;
    let playlists = &app.playlists;

    let playlist_names = |song_id: SongId| -> Vec<String> {
        playlists
            .containing(song_id)
            .iter()
            .filter_map(|&id| playlists.get(id).map(|p| p.name().to_string()))
            .collect()
    };

    let PanelHeight(height) = render_song_list_panel(
        area,
        buf,
        &mut app.library_panel.list_state,
        rows,
        current,
        &app.library,
        |song| if filtering { filtered_label_for(song, sort) } else { label_for(song, category, sort) },
        "Library",
        app.library_panel.category.label(),
        app.library_panel.sort.label(),
        app.library_panel.searching,
        &app.library_panel.search_query,
        Some((playlist_mode, &playlist_names)),
    );
    app.library_panel.page_height = height;
}
