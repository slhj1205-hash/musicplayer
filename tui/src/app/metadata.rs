use lyre_core::MetadataEdits;

use super::state::{heading_selected_message, MetadataEditModal, MetadataField, Row, RomanizedArtistConfirmModal, StatusKind};
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
                let original_artist_sort = edits.artist_sort.clone();
                self.modal.metadata_modal = Some(MetadataEditModal {
                    song: id,
                    edits,
                    original_artist_sort,
                    focused: MetadataField::Title,
                    error: None,
                });
            }
            Some(Row::Header(heading)) => {
                self.set_status(heading_selected_message(&heading), StatusKind::Info);
            }
            None => self.set_status("select a song first", StatusKind::Info),
        }
    }

    pub(super) fn save_metadata_edit(&mut self, modal: MetadataEditModal) {
        let MetadataEditModal { song, edits, original_artist_sort, focused, .. } = modal;

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

                self.maybe_prompt_romanized_artist(new_id, &edits.artist_sort, &original_artist_sort);
            }
            Err(e) => {
                self.modal.metadata_modal =
                    Some(MetadataEditModal { song, edits, original_artist_sort, focused, error: Some(e.to_string()) });
            }
        }
    }

    pub(super) fn maybe_prompt_romanized_artist(&mut self, saved_song: lyre_core::SongId, artist_sort: &str, original: &str) {
        let value = artist_sort.trim();
        if value.is_empty() || value == original.trim() {
            return;
        }
        let Some(reference) = self.library.get(saved_song) else { return };
        let artist_sort_key = reference.sort_artist().to_string();
        let artist_display = reference.artist().to_string();

        let count = self.library.count_matching_artist(&artist_sort_key, saved_song);
        if count == 0 {
            return;
        }

        self.modal.romanized_artist_confirm = Some(RomanizedArtistConfirmModal {
            artist_display,
            artist_sort_key,
            value: value.to_string(),
            reference_song: saved_song,
            count,
        });
    }

    pub(super) fn confirm_romanized_artist_apply(&mut self, confirm: RomanizedArtistConfirmModal) {
        let renames = self.library.update_artist_sort_for_matching(
            &confirm.artist_sort_key,
            &confirm.value,
            confirm.reference_song,
        );

        for (old_id, new_id) in &renames {
            self.playlists.rename_song_id(*old_id, *new_id);
            self.queue.rename_song_id(*old_id, *new_id);
            if let Some(pos) = self.display_order.iter().position(|&id| id == *old_id) {
                self.display_order[pos] = *new_id;
            }
        }

        if !renames.is_empty() {
            self.library_revision += 1;
        }

        let applied = renames.len();
        self.set_status(
            format!("applied romanized artist to {applied} other song{}", if applied == 1 { "" } else { "s" }),
            StatusKind::Success,
        );
    }
}
