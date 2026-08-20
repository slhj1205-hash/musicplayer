mod footer;
mod header;
mod library;
mod metadata_modal;
mod modals;
mod now_playing;
mod playlists;
mod rows;
mod song_modal;
mod style;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    widgets::{Paragraph, Widget},
};

use crate::app::{App, Panel};

pub use rows::{label_for, render_no_matches, render_song_list_panel, viewport, PanelHeight};
pub use style::{
    centered_rect, content_style, dim_area, display_width, focus_style, format_duration, format_mtime,
    marker_style, marquee_window, modal_block, modal_body_style, plural, search_title, side_by_side_rect,
    sort_title, styled_list, titled_block, titled_block_split, unfocused_style,
};

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Length(2),
        ]);
        let [header_area, dir_area, body_area, position_area, footer_area] = area.layout(&layout);

        let body_layout = Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]);
        let [left_area, now_playing_area] = body_area.layout(&body_layout);

        style::reset_marquee_activity();

        header::render(self, header_area, buf);
        render_dir_input(self, dir_area, buf);
        match self.panel {
            Panel::Library => library::render(self, left_area, buf),
            Panel::Playlists => playlists::render(self, left_area, buf),
        }
        now_playing::render(self, now_playing_area, buf);
        now_playing::render_position(self, position_area, buf);
        footer::render(self, footer_area, buf);

        self.animating.set(style::marquee_active());

        if self.modal.confirming_quit {
            modals::render_quit_confirm(area, buf);
        } else if let Some((playlist_id, song_id)) = self.modal.confirming_remove {
            modals::render_remove_confirm(self, playlist_id, song_id, area, buf);
        } else if self.modal.showing_help {
            modals::render_help_overlay(area, buf);
        } else if self.modal.song_modal.is_some() {
            song_modal::render(self, area, buf);
        } else if self.modal.metadata_modal.is_some() {
            metadata_modal::render(self, area, buf);
        }
    }
}

fn render_dir_input(app: &App, area: Rect, buf: &mut Buffer) {
    let title = if app.dir.editing_dir {
        " Directory (<Enter> to load, <Esc> to cancel) ".to_string()
    } else {
        format!(" Directory {} ", crate::keymap::display_for(crate::keymap::Action::ChangeDirectory))
    };
    let border_style = if app.dir.editing_dir { focus_style() } else { unfocused_style() };

    Paragraph::new(app.dir.dir_input.as_str())
        .style(content_style())
        .block(titled_block(title, border_style))
        .render(area, buf);
}
