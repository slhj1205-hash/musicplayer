use ratatui::widgets::ListState;

use lyre_core::{player::PlayerEvent, PlaylistId, Queue, SongId};

use super::state::{heading_selected_message, Panel, PlaylistView, QueueSource, Row, StatusKind};
use super::{App, EventsChanged};

impl App {

    pub(super) fn drain_player_events(&mut self) -> EventsChanged {
        let events = self.player.poll_events();
        let changed = if events.is_empty() { EventsChanged::Unchanged } else { EventsChanged::Changed };
        for event in events {
            match event {
                PlayerEvent::SongEnded => self.advance(),
                PlayerEvent::Error(msg) => {
                    self.queue.clear_current();
                    self.set_status(format!("playback error: {msg}"), StatusKind::Error);
                }
                PlayerEvent::PositionTick(_) | PlayerEvent::StateChanged(_) => {}
            }
        }
        changed
    }

    pub(super) fn activate_selected(&mut self) {
        match self.panel {
            Panel::Library => self.play_selected_library(),
            Panel::Playlists => match self.playlist_panel.view {
                PlaylistView::Browsing => self.open_selected_playlist(),
                PlaylistView::Viewing(id) => self.play_selected_in_playlist(id),
            },
        }
    }

    pub(super) fn play_selected_library(&mut self) {
        match self.selected_row() {
            Some(Row::Song(id, _)) => {
                if self.queue_source != QueueSource::Library {
                    self.queue = Queue::new(self.display_order.clone());
                    self.queue_source = QueueSource::Library;
                }
                if let Some(played) = self.queue.play_id(id) {
                    self.play_current(played);
                }
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

    pub(super) fn open_selected_playlist(&mut self) {
        let Some(id) = self.selected_playlist_id() else {
            self.set_status("no playlists yet -- press p on a song to create one", StatusKind::Info);
            return;
        };
        self.playlist_panel.view = PlaylistView::Viewing(id);
        self.playlist_panel.search_query.clear();
        self.playlist_panel.list_state = ListState::default();
        let target = self.visible_rows().iter().position(|r| matches!(r, Row::Song(_, _)));
        self.playlist_panel.list_state.select(target);
        let name = self.playlists.get(id).map(|p| p.name().to_string()).unwrap_or_default();
        self.set_status(format!("viewing \"{name}\""), StatusKind::Info);
    }

    pub(super) fn play_selected_in_playlist(&mut self, id: PlaylistId) {
        if self.playlists.get(id).is_none() {
            return;
        }
        match self.selected_row() {
            Some(Row::Song(song_id, _)) => {
                let Some(playlist) = self.playlists.get(id) else {
                    return;
                };
                self.queue = Queue::from_playlist(playlist);
                self.queue_source = QueueSource::Playlist(id);
                if let Some(played) = self.queue.play_id(song_id) {
                    self.play_current(played);
                }
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

    pub(super) fn advance(&mut self) {
        match self.queue.next() {
            Some(id) => self.play_current(id),
            None => {
                self.queue.clear_current();
                if let Err(e) = self.player.stop() {
                    self.set_status(format!("failed to stop cleanly: {e}"), StatusKind::Error);
                } else {
                    self.set_status("end of queue", StatusKind::Info);
                }
            }
        }
    }

    pub(super) fn go_back(&mut self) {
        match self.queue.previous() {
            Some(id) => self.play_current(id),
            None => self.set_status("no previous track", StatusKind::Info),
        }
    }

    pub(super) fn jump_to_upcoming(&mut self) {
        let input = std::mem::take(&mut self.pending_number);
        match input.parse::<usize>() {
            Ok(0) => self.set_status("Up Next positions start at 1", StatusKind::Error),
            Ok(n) => match self.queue.play_upcoming(n) {
                Some(id) => self.play_current(id),
                None => self.set_status("Up Next is empty", StatusKind::Error),
            },
            Err(_) => self.set_status("invalid Up Next position", StatusKind::Error),
        }
    }

    pub(super) fn play_current(&mut self, id: SongId) {
        let Some(song) = self.library.get(id) else {
            self.set_status("selected song is no longer in the library", StatusKind::Error);
            self.queue.clear_current();
            return;
        };
        match self.player.play(song) {
            Ok(()) => self.set_status(format!("playing: {song}"), StatusKind::Success),
            Err(e) => {
                self.set_status(format!("failed to play: {e}"), StatusKind::Error);
                self.queue.clear_current();
            }
        }
    }

    pub(super) fn queue_selected_next(&mut self) {
        match self.selected_row() {
            Some(Row::Song(id, _)) => {
                self.queue.queue_next(id);
                let label = self.library.get(id).map(|s| s.to_string()).unwrap_or_else(|| "song".to_string());
                self.set_status(format!("queued next: {label}"), StatusKind::Success);
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
}
