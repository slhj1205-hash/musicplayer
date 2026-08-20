use ratatui::widgets::ListState;

use lyre_core::PlaylistId;

use super::state::{heading_selected_message, ChooseActionField, Panel, PlaylistView, Row, SidePanel, SongModal, StatusKind};
use super::App;

impl App {
    pub(super) fn set_song_modal(
        &mut self,
        song: lyre_core::SongId,
        selected: ChooseActionField,
        name_input: String,
        side: Option<SidePanel>,
    ) {
        self.modal.song_modal = Some(SongModal { song, selected, name_input, side });
    }

    pub(super) fn open_song_modal(&mut self) {
        match self.selected_row() {
            Some(Row::Song(id, _)) => {
                let selected =
                    if self.playlists.is_empty() { ChooseActionField::CreatePlaylist } else { ChooseActionField::AddToPlaylist };
                self.set_song_modal(id, selected, String::new(), None);
            }
            Some(Row::Header(heading)) => {
                self.set_status(heading_selected_message(&heading), StatusKind::Info);
            }
            None => self.set_status("select a song first", StatusKind::Info),
        }
    }

    pub(super) fn open_remove_confirm(&mut self) {
        if self.panel != Panel::Playlists {
            return;
        }
        let PlaylistView::Viewing(playlist_id) = self.playlist_panel.view else { return };

        match self.selected_row() {
            Some(Row::Song(song_id, _)) => {
                self.modal.confirming_remove = Some((playlist_id, song_id));
            }
            Some(Row::Header(heading)) => {
                self.set_status(
                    heading_selected_message(&heading),
                    StatusKind::Info,
                );
            }
            None => self.set_status("select a song first", StatusKind::Info),
        }
    }

    pub(super) fn toggle_panel(&mut self) {
        self.panel = match self.panel {
            Panel::Library => Panel::Playlists,
            Panel::Playlists => Panel::Library,
        };
    }

    pub(super) fn build_add_to_playlist_side(&self, song: lyre_core::SongId) -> Option<SidePanel> {
        let currently_viewing =
            if let (Panel::Playlists, PlaylistView::Viewing(id)) = (self.panel, self.playlist_panel.view) {
                Some(id)
            } else {
                None
            };
        add_to_playlist_side(&self.playlists, currently_viewing, song)
    }
}

fn add_to_playlist_side(
    playlists: &lyre_core::PlaylistStore,
    currently_viewing: Option<PlaylistId>,
    song: lyre_core::SongId,
) -> Option<SidePanel> {
    let mut pinned: Vec<PlaylistId> = currently_viewing.into_iter().collect();

    for &id in playlists.ids_sorted_by_name() {
        if pinned.contains(&id) {
            continue;
        }
        if playlists.contains(id, song) {
            pinned.push(id);
        }
    }

    let mut options = playlists.ids_sorted_by_name().to_vec();
    options.retain(|id| !pinned.contains(id));

    if options.is_empty() {
        return None;
    }

    let mut list_state = ListState::default();
    list_state.select(Some(0));

    Some(SidePanel::AddToPlaylist { options, pinned, list_state })
}
