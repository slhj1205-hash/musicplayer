use lyre_core::MetadataEdits;

use super::state::{heading_selected_message, MetadataEditModal, MetadataField, Row, StatusKind};
use super::App;

impl App {
    pub(super) fn open_metadata_modal(&mut self) {
        match self.selected_row() {
            Some(Row::Song(id, _)) => {
                let Some(song) = self.library.get(id) else {
                    self.set_status("selected song is no longer in the library", StatusKind::Error);
                    return;
                };
                let edits = MetadataEdits::from_metadata(song.metadata());
                self.modal.metadata_modal =
                    Some(MetadataEditModal { song: id, edits, focused: MetadataField::Title, error: None });
            }
            Some(Row::Header(heading)) => {
                self.set_status(heading_selected_message(&heading), StatusKind::Info);
            }
            None => self.set_status("select a song first", StatusKind::Info),
        }
    }

    pub(super) fn save_metadata_edit(&mut self, modal: MetadataEditModal) {
        let MetadataEditModal { song, edits, focused, .. } = modal;

        match self.library.update_metadata(song, &edits) {
            Ok(new_id) => {
                self.playlists.rename_song_id(song, new_id);
                self.queue.rename_song_id(song, new_id);
                if let Some(pos) = self.display_order.iter().position(|&id| id == song) {
                    self.display_order[pos] = new_id;
                }
                self.library_revision += 1;

                let label = self.library.get(new_id).map(|s| s.to_string()).unwrap_or_else(|| "song".to_string());
                self.set_status(format!("updated metadata for {label}"), StatusKind::Success);
                self.select_song_by_id(new_id);
            }
            Err(e) => {
                self.modal.metadata_modal = Some(MetadataEditModal { song, edits, focused, error: Some(e.to_string()) });
            }
        }
    }
}
