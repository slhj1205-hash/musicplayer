mod input;
mod metadata;
mod navigation;
mod panels;
mod playback;
mod row_builder;
mod state;
mod youtube;

use std::{
    path::PathBuf,
    time::Duration,
};

use color_eyre::Result;
use crossterm::event;
use ratatui::{widgets::ListState, DefaultTerminal};

use lyre_core::{Library, Player, PlaylistStore, Queue, SongId};

use crate::Backend;

pub use row_builder::RowCache;
pub use state::{
    Category, ChooseActionField, DirScanState, LibraryPanelState, MetadataEditModal, MetadataField, ModalState,
    Panel, PlaylistDisplayMode, PlaylistPanelState, PlaylistView, QueueSource, RomanizedArtistConfirmModal, Row,
    SidePanel, Sort, SongModal, StatusKind, StatusMessage, YoutubeField, YoutubeFieldsModal, YoutubeModal,
};

pub struct App {
    pub library: Library,
    pub queue: Queue,
    queue_source: QueueSource,
    pub player: Player<Backend>,
    pub playlists: PlaylistStore,
    display_order: Vec<SongId>,

    pub(crate) library_revision: u64,
    pub rows: RowCache,

    pub(crate) animating: std::cell::Cell<bool>,
    pub status: StatusMessage,
    should_exit: bool,
    pending_number: String,
    pub panel: Panel,

    pub dir: DirScanState,
    pub library_panel: LibraryPanelState,
    pub playlist_panel: PlaylistPanelState,
    pub modal: ModalState,

    youtube_tx: std::sync::mpsc::Sender<youtube::DownloadEvent>,
    youtube_rx: std::sync::mpsc::Receiver<youtube::DownloadEvent>,
}

impl App {
    pub fn new(library: Library, playlists: PlaylistStore, backend: Backend) -> App {
        let display_order: Vec<SongId> = library.ids_by_path().to_vec();
        let queue = Queue::new(display_order.clone());

        let mut library_panel = LibraryPanelState::default();
        if !display_order.is_empty() {
            library_panel.list_state.select(Some(0));
        }

        let status = StatusMessage::new(
            format!("loaded {} song(s) from {}", library.len(), library.root().display()),
            StatusKind::Success,
        );
        let dir = DirScanState { dir_input: library.root().display().to_string(), ..Default::default() };

        let (youtube_tx, youtube_rx) = youtube::channel();

        App {
            library,
            queue,
            queue_source: QueueSource::Library,
            player: Player::new(backend),
            playlists,
            display_order,
            library_revision: 0,
            rows: RowCache::default(),
            animating: std::cell::Cell::new(false),
            status,
            should_exit: false,
            pending_number: String::new(),
            panel: Panel::Library,
            dir,
            library_panel,
            playlist_panel: PlaylistPanelState::default(),
            modal: ModalState::default(),
            youtube_tx,
            youtube_rx,
        }
    }

    pub fn on_key(&mut self, key: event::KeyEvent) {
        self.handle_key(key);
    }

    fn set_status(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.status = StatusMessage::new(text, kind);
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal) -> Result<()> {

        let mut needs_redraw = true;

        while !self.should_exit {
            let status_changed = self.status.expire_if_stale();

            if needs_redraw || status_changed || self.animating.get() {
                terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            }

            needs_redraw = self.handle_events()?;

            self.playlists.flush_if_due();

            if let Some(dir) = self.dir.pending_scan.take() {
                terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
                self.finish_dir_scan(dir);
                needs_redraw = true;
            }

            if self.drain_player_events() {
                needs_redraw = true;
            }

            if self.drain_youtube_events() {
                needs_redraw = true;
            }
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<bool> {

        let timeout = if self.animating.get() {
            Duration::from_millis(120)
        } else {
            Duration::from_millis(400)
        };

        if !event::poll(timeout)? {
            return Ok(false);
        }
        match event::read()? {
            e if e.as_key_press_event().is_some() => {
                self.handle_key(e.as_key_press_event().expect("checked above"));
                Ok(true)
            }

            _ => Ok(true),
        }
    }

    fn begin_dir_scan(&mut self) {
        let new_dir = PathBuf::from(self.dir.dir_input.trim());
        self.set_status(format!("scanning {}…", new_dir.display()), StatusKind::Info);
        self.dir.pending_scan = Some(new_dir);
        self.dir.editing_dir = false;
    }

    fn finish_dir_scan(&mut self, new_dir: PathBuf) {
        let cache_path = crate::config::scan_cache_path(&new_dir);

        match Library::scan(&new_dir, &cache_path) {
            Ok((library, stats)) => {
                let stop_error = self.player.stop().err();

                self.display_order = library.ids_by_path().to_vec();
                self.queue = Queue::new(self.display_order.clone());
                self.queue_source = QueueSource::Library;
                self.library_panel.list_state = ListState::default();
                if !self.display_order.is_empty() {
                    self.library_panel.list_state.select(Some(0));
                }

                let playlists_path = crate::config::playlists_path(library.root());
                let (playlists, prune_stats) = PlaylistStore::load(playlists_path, &library);
                self.playlists.flush();
                self.playlists = playlists;
                self.playlist_panel.view = PlaylistView::Browsing;
                self.playlist_panel.search_query.clear();
                self.playlist_panel.searching = false;
                self.reset_playlist_browse_selection();
                self.modal.song_modal = None;
                self.modal.confirming_remove = None;

                let mut message = format!("loaded {} song(s) from {}", library.len(), library.root().display());
                if stats.skipped() > 0 {
                    message.push_str(&format!(" ({} skipped)", stats.skipped()));
                }
                if prune_stats.songs_removed > 0 {
                    message.push_str(&format!(
                        ", removed {} missing song(s) from playlists",
                        prune_stats.songs_removed
                    ));
                }
                match stop_error {
                    None => self.set_status(message, StatusKind::Success),
                    Some(e) => {
                        message.push_str(&format!(" (failed to stop previous playback cleanly: {e})"));
                        self.set_status(message, StatusKind::Error);
                    }
                }
                self.dir.dir_input = library.root().display().to_string();
                self.library_panel.search_query.clear();

                crate::config::save_last_dir(library.root());
                self.library = library;
                self.library_revision += 1;
                self.rows.invalidate();
            }
            Err(e) => {
                self.set_status(format!("failed to scan {}: {e}", new_dir.display()), StatusKind::Error);
            }
        }
    }
}
