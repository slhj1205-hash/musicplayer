use crossterm::event::{KeyCode, KeyEvent};

use lyre_core::SongId;

use crate::keymap::{self, Action, Direction};

use super::navigation::move_wrapping;
use super::state::{ChooseActionField, Panel, PlaylistView, SidePanel, StatusKind};
use super::App;

impl App {
    pub(super) fn handle_key(&mut self, key: KeyEvent) {
        if self.modal.confirming_quit {
            self.handle_confirm_quit_key(key);
            return;
        }
        if self.modal.confirming_remove.is_some() {
            self.handle_confirm_remove_key(key);
            return;
        }
        if self.modal.showing_help {
            self.modal.showing_help = false;
            return;
        }
        if self.modal.song_modal.is_some() {
            self.handle_song_modal_key(key);
            return;
        }
        if self.modal.metadata_modal.is_some() {
            self.handle_metadata_modal_key(key);
            return;
        }
        if self.modal.youtube_modal.is_some() {
            self.handle_youtube_modal_key(key);
            return;
        }
        if self.dir.editing_dir {
            self.handle_dir_input_key(key);
            return;
        }
        if self.library_panel.searching {
            self.handle_search_key(key);
            return;
        }
        if self.playlist_panel.searching {
            self.handle_playlist_search_key(key);
            return;
        }

        let had_pending_number = !self.pending_number.is_empty();
        let is_digit = matches!(key.code, KeyCode::Char(c) if c.is_ascii_digit());
        if !is_digit && key.code != KeyCode::Char('n') {
            self.pending_number.clear();
        }

        const MAX_PENDING_NUMBER_DIGITS: usize = 3;

        if is_digit {
            if let KeyCode::Char(c) = key.code
                && self.pending_number.len() < MAX_PENDING_NUMBER_DIGITS
            {
                self.pending_number.push(c);
                self.set_status(
                    format!("jump to Up Next #{} (press <n>, <Esc> to cancel)", self.pending_number),
                    StatusKind::Info,
                );
            }
            return;
        }

        if key.code == KeyCode::Esc {
            if had_pending_number {
                self.set_status("cancelled queue jump", StatusKind::Info);
            } else if self.panel == Panel::Playlists && matches!(self.playlist_panel.view, PlaylistView::Viewing(_)) {
                if self.playlist_panel.search_query.is_empty() {
                    self.playlist_panel.view = PlaylistView::Browsing;
                    self.reset_playlist_browse_selection();
                } else {
                    self.playlist_panel.search_query.clear();
                    self.sync_selection_to_rows();
                    self.set_status("cleared search", StatusKind::Info);
                }
            } else if self.panel == Panel::Playlists
                && self.playlist_panel.view == PlaylistView::Browsing
                && !self.playlist_panel.search_query.is_empty()
            {
                self.playlist_panel.search_query.clear();
                self.sync_playlist_browse_selection();
                self.set_status("cleared search", StatusKind::Info);
            } else if self.panel == Panel::Library && !self.library_panel.search_query.is_empty() {
                self.library_panel.search_query.clear();
                self.sync_selection_to_rows();
                self.set_status("cleared search", StatusKind::Info);
            } else {
                self.modal.confirming_quit = true;
            }
            return;
        }

        match keymap::lookup(key) {
            Some(Action::TogglePanel) => self.toggle_panel(),
            Some(Action::MoveDown) => self.move_selection(1),
            Some(Action::MoveUp) => self.move_selection(-1),
            Some(Action::PageDown) => self.jump_page(Direction::Forwards),
            Some(Action::PageUp) => self.jump_page(Direction::Backwards),
            Some(Action::JumpTop) => self.select_first_row(),
            Some(Action::JumpBottom) => self.select_last_row(),
            Some(Action::JumpToCurrent) => self.jump_to_current(),
            Some(Action::Activate) => self.activate_selected(),
            Some(Action::TogglePlayback) => {
                if let Err(e) = self.player.toggle() {
                    self.set_status(format!("playback error: {e}"), StatusKind::Error);
                }
            }
            Some(Action::NextOrJump) => {
                if self.pending_number.is_empty() {
                    self.advance();
                } else {
                    self.jump_to_upcoming();
                }
            }
            Some(Action::PrevTrack) => self.go_back(),
            Some(Action::QueueNext) => self.queue_selected_next(),
            Some(Action::OpenSongModal) => self.open_song_modal(),
            Some(Action::OpenMetadataEditModal) => self.open_metadata_modal(),
            Some(Action::OpenYoutubeModal) => self.open_youtube_modal(),
            Some(Action::RemoveFromPlaylist) => self.open_remove_confirm(),
            Some(Action::ChangeDirectory) => {
                self.dir.dir_input = self.library.root().display().to_string();
                self.dir.editing_dir = true;
            }
            Some(Action::ToggleSearch) => match self.panel {
                Panel::Library => self.library_panel.searching = true,
                Panel::Playlists => self.playlist_panel.searching = true,
            },
            Some(Action::CycleCategory(direction)) => self.cycle_category(direction),
            Some(Action::CycleSort(direction)) => self.cycle_sort(direction),
            Some(Action::CyclePlaylistDisplayMode) => self.cycle_library_playlist_mode(),
            Some(Action::Shuffle) => {
                self.queue.shuffle();
                self.set_status("shuffled", StatusKind::Info);
            }
            Some(Action::Unshuffle) => {
                self.queue.unshuffle();
                self.set_status("restored original order", StatusKind::Info);
            }
            Some(Action::VolumeUp) => self.player.adjust_volume(0.05),
            Some(Action::VolumeDown) => self.player.adjust_volume(-0.05),
            Some(Action::Quit) => self.modal.confirming_quit = true,
            Some(Action::ToggleHelp) => self.modal.showing_help = true,
            Some(Action::None) | None => {}
        }
    }

    fn handle_confirm_quit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                self.playlists.flush();
                self.should_exit = true;
            }
            _ => self.modal.confirming_quit = false,
        }
    }

    fn handle_confirm_remove_key(&mut self, key: KeyEvent) {
        let Some((playlist_id, song_id)) = self.modal.confirming_remove.take() else { return };
        if !matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            return;
        }

        let label = self.library.get(song_id).map(|s| s.to_string()).unwrap_or_else(|| "song".to_string());
        if self.playlists.remove_song(playlist_id, song_id) {
            self.set_status(format!("removed {label}"), StatusKind::Success);
            self.sync_selection_to_rows();
        } else {
            self.set_status("failed to remove song from playlist", StatusKind::Error);
        }
    }

    fn handle_dir_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.begin_dir_scan(),
            KeyCode::Esc => {
                self.dir.editing_dir = false;
                self.dir.dir_input = self.library.root().display().to_string();
            }
            KeyCode::Backspace => {
                self.dir.dir_input.pop();
            }
            KeyCode::Char(c) => self.dir.dir_input.push(c),
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.library_panel.searching = false,
            KeyCode::Esc => {
                if self.library_panel.search_query.is_empty() {
                    self.library_panel.searching = false;
                } else {
                    self.library_panel.search_query.clear();
                    self.sync_selection_to_rows();
                }
            }
            KeyCode::Backspace => {
                self.library_panel.search_query.pop();
                self.sync_selection_to_rows();
            }
            KeyCode::Char(c) => {
                self.library_panel.search_query.push(c);
                self.sync_selection_to_rows();
            }
            _ => {}
        }
    }

    fn handle_playlist_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.playlist_panel.searching = false,
            KeyCode::Esc => {
                if self.playlist_panel.search_query.is_empty() {
                    self.playlist_panel.searching = false;
                } else {
                    self.playlist_panel.search_query.clear();
                    self.sync_playlist_selection();
                }
            }
            KeyCode::Backspace => {
                self.playlist_panel.search_query.pop();
                self.sync_playlist_selection();
            }
            KeyCode::Char(c) => {
                self.playlist_panel.search_query.push(c);
                self.sync_playlist_selection();
            }
            _ => {}
        }
    }

    fn handle_song_modal_key(&mut self, key: KeyEvent) {
        let Some(modal) = self.modal.song_modal.take() else { return };
        match modal.side {
            Some(side) => self.handle_side_panel_key(key, modal.song, modal.selected, modal.name_input, side),
            None => self.handle_choose_action_key(key, modal.song, modal.selected, modal.name_input),
        }
    }

    fn handle_choose_action_key(
        &mut self,
        key: KeyEvent,
        song: SongId,
        mut selected: ChooseActionField,
        mut name_input: String,
    ) {
        if selected == ChooseActionField::CreatePlaylist {
            match key.code {
                KeyCode::Esc => return,
                KeyCode::Up | KeyCode::Down if !self.playlists.is_empty() => {
                    selected = ChooseActionField::AddToPlaylist;
                }
                KeyCode::Enter => {
                    let trimmed = name_input.trim();
                    if trimmed.is_empty() {
                        self.set_status("playlist name can't be empty", StatusKind::Error);
                        self.set_song_modal(song, selected, name_input, None);
                        return;
                    }
                    let id = self.playlists.create(trimmed);
                    self.playlists.add_song(id, song);
                    self.set_status(format!("created \"{trimmed}\" and added the song"), StatusKind::Success);
                    return;
                }
                KeyCode::Backspace => {
                    name_input.pop();
                }
                KeyCode::Char(c) => name_input.push(c),
                _ => {}
            }
            self.set_song_modal(song, selected, name_input, None);
            return;
        }

        match key.code {
            KeyCode::Esc => return,
            KeyCode::Char('j') | KeyCode::Char('k') | KeyCode::Down | KeyCode::Up => {
                selected = ChooseActionField::CreatePlaylist;
            }
            KeyCode::Enter => match self.build_add_to_playlist_side(song) {
                Some(side) => {
                    self.set_song_modal(song, selected, name_input, Some(side));
                    return;
                }
                None => {
                    self.set_status(
                        "no other playlists to add to -- select Create Playlist instead",
                        StatusKind::Info,
                    );
                }
            },
            _ => {}
        }
        self.set_song_modal(song, selected, name_input, None);
    }

    fn handle_side_panel_key(
        &mut self,
        key: KeyEvent,
        song: SongId,
        selected: ChooseActionField,
        name_input: String,
        side: SidePanel,
    ) {
        let SidePanel::AddToPlaylist { options, pinned, mut list_state } = side;

        match key.code {
            KeyCode::Esc => {
                self.set_song_modal(song, selected, name_input, None);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                move_wrapping(&mut list_state, options.len(), 1);
                self.set_song_modal(song, selected, name_input, Some(SidePanel::AddToPlaylist { options, pinned, list_state }));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                move_wrapping(&mut list_state, options.len(), -1);
                self.set_song_modal(song, selected, name_input, Some(SidePanel::AddToPlaylist { options, pinned, list_state }));
            }
            KeyCode::Enter => {
                if let Some(&target) = list_state.selected().and_then(|i| options.get(i)) {
                    let name = self.playlists.get(target).map(|p| p.name().to_string()).unwrap_or_default();
                    if self.playlists.add_song(target, song) {
                        self.set_status(format!("added to \"{name}\""), StatusKind::Success);
                    } else {
                        self.set_status(format!("already in \"{name}\""), StatusKind::Info);
                    }
                }
            }
            _ => {
                self.set_song_modal(song, selected, name_input, Some(SidePanel::AddToPlaylist { options, pinned, list_state }));
            }
        }
    }

    fn handle_metadata_modal_key(&mut self, key: KeyEvent) {
        let Some(mut modal) = self.modal.metadata_modal.take() else { return };

        match key.code {
            KeyCode::Esc => {}
            KeyCode::Tab | KeyCode::Down => {
                modal.focused = modal.focused.next();
                modal.error = None;
                self.modal.metadata_modal = Some(modal);
            }
            KeyCode::BackTab | KeyCode::Up => {
                modal.focused = modal.focused.prev();
                modal.error = None;
                self.modal.metadata_modal = Some(modal);
            }
            KeyCode::Enter => self.save_metadata_edit(modal),
            KeyCode::Backspace => {
                modal.focused.value_mut(&mut modal.edits).pop();
                modal.error = None;
                self.modal.metadata_modal = Some(modal);
            }
            KeyCode::Char(c) => {
                modal.focused.value_mut(&mut modal.edits).push(c);
                modal.error = None;
                self.modal.metadata_modal = Some(modal);
            }
            _ => {
                self.modal.metadata_modal = Some(modal);
            }
        }
    }
}
