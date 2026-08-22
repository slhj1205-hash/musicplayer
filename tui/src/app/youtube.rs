use std::{
    fs, path::PathBuf,
    sync::mpsc,
};

use crossterm::event::KeyCode;

use lyre_core::{youtube, InsertOutcome, MetadataEdits, Metadata, Song};

use super::state::{DownloadStatus, FetchStatus, YoutubeField, YoutubeFieldsModal, YoutubeModal};
use super::state::StatusKind;
use super::App;

pub enum DownloadEvent {
    InfoReady { title: String, uploader: Option<String> },
    DownloadReady(PathBuf),
    Failed(String),
}

pub(super) fn channel() -> (mpsc::Sender<DownloadEvent>, mpsc::Receiver<DownloadEvent>) {
    mpsc::channel()
}

impl App {
    pub fn handle_youtube_event_for_test(&mut self, event: DownloadEvent) {
        self.handle_youtube_event(event);
    }

    pub fn drain_youtube_events_for_test(&mut self) -> bool {
        self.drain_youtube_events()
    }

    pub(super) fn drain_youtube_events(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.youtube_rx.try_recv() {
            changed = true;
            self.handle_youtube_event(event);
        }
        changed
    }

    fn handle_youtube_event(&mut self, event: DownloadEvent) {
        match event {
            DownloadEvent::InfoReady { title, uploader } => {
                if let Some(fields) = self.youtube_fields_mut() {
                    fields.fetch_status = FetchStatus::Ready { title, uploader };
                }
            }
            DownloadEvent::DownloadReady(path) => match self.modal.youtube_modal.take() {
                Some(YoutubeModal::Downloading { fields, dest_path, .. }) => {
                    self.finalize_youtube_download(fields, path, dest_path);
                }
                Some(YoutubeModal::EditingFields(mut fields)) => {
                    fields.download_status = DownloadStatus::Ready(path);
                    self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields));
                }
                Some(YoutubeModal::ResolvingCollision { mut fields, existing_path }) => {
                    fields.download_status = DownloadStatus::Ready(path);
                    self.modal.youtube_modal = Some(YoutubeModal::ResolvingCollision { fields, existing_path });
                }
                other => {
                    self.modal.youtube_modal = other;
                    youtube::discard_temp_file(&path);
                }
            },
            DownloadEvent::Failed(message) => self.interrupt_youtube_with_error(message),
        }
    }

    fn youtube_fields_mut(&mut self) -> Option<&mut YoutubeFieldsModal> {
        match self.modal.youtube_modal.as_mut()? {
            YoutubeModal::EditingFields(fields) => Some(fields),
            YoutubeModal::ResolvingCollision { fields, .. } => Some(fields),
            YoutubeModal::Downloading { fields, .. } => Some(fields),
            YoutubeModal::EnteringUrl { .. } => None,
        }
    }

    fn interrupt_youtube_with_error(&mut self, message: String) {
        let fields = match self.modal.youtube_modal.take() {
            Some(YoutubeModal::EditingFields(fields)) => fields,
            Some(YoutubeModal::ResolvingCollision { fields, .. }) => fields,
            Some(YoutubeModal::Downloading { fields, .. }) => fields,
            _ => return,
        };

        let url_input = fields.url.clone();
        self.modal.youtube_modal =
            Some(YoutubeModal::EnteringUrl { url_input, error: Some(message), restore: Some(fields) });
    }

    fn finalize_youtube_download(&mut self, fields: YoutubeFieldsModal, temp_path: PathBuf, dest_path: PathBuf) {
        if let Err(e) = youtube::finalize_download(&temp_path, &dest_path) {
            self.set_status(format!("download finished, but failed to save it: {e}"), StatusKind::Error);
            return;
        }

        let edits = MetadataEdits {
            title: fields.title.clone(),
            artist: fields.artist.clone(),
            album: fields.album.clone(),
            genre: String::new(),
            track: String::new(),
            date: String::new(),
            title_sort: fields.title_sort.clone(),
            artist_sort: fields.artist_sort.clone(),
        };

        if let Err(e) = Metadata::write(&dest_path, &edits) {
            self.set_status(format!("downloaded, but failed to tag: {e}"), StatusKind::Error);
            return;
        }

        let song = match Song::load(&dest_path) {
            Ok(song) => song,
            Err(e) => {
                self.set_status(format!("downloaded and tagged, but failed to load it back: {e}"), StatusKind::Error);
                return;
            }
        };

        let label = song.to_string();
        match self.library.insert(song) {
            InsertOutcome::Inserted(id) => {
                self.display_order.push(id);
                if self.queue_source() == super::QueueSource::Library {
                    self.queue.insert(id);
                }
                self.library_revision += 1;
                self.rows.invalidate();
                self.set_status(format!("downloaded and added: {label}"), StatusKind::Success);
                self.select_song_by_id(id);

                self.maybe_prompt_romanized_artist(id, &fields.artist_sort, "");
            }
            InsertOutcome::Collision { .. } => {
                self.set_status("downloaded song already exists in the library", StatusKind::Info);
            }
        }
    }

    pub(super) fn open_youtube_modal(&mut self) {
        self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input: String::new(), error: None, restore: None });
    }

    pub(super) fn handle_youtube_modal_key(&mut self, key: crossterm::event::KeyEvent) {
        let Some(modal) = self.modal.youtube_modal.take() else { return };

        match modal {
            YoutubeModal::EnteringUrl { mut url_input, restore, .. } => match key.code {
                KeyCode::Esc => self.modal.youtube_modal = None,
                KeyCode::Enter => {
                    let url = url_input.trim().to_string();
                    if url.is_empty() {
                        self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl {
                            url_input,
                            error: Some("enter a URL first".to_string()),
                            restore,
                        });
                        return;
                    }
                    self.spawn_fetch_and_download(url.clone());
                    self.modal.youtube_modal = Some(YoutubeModal::EditingFields(start_youtube_fields(url, restore)));
                }
                KeyCode::Backspace => {
                    url_input.pop();
                    self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input, error: None, restore });
                }
                KeyCode::Char(c) => {
                    url_input.push(c);
                    self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input, error: None, restore });
                }
                _ => self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input, error: None, restore }),
            },
            YoutubeModal::EditingFields(fields) => self.handle_youtube_fields_key(key, fields),
            YoutubeModal::ResolvingCollision { fields, existing_path } => match key.code {
                KeyCode::Char('o') => self.start_or_await_youtube_download(fields, existing_path),
                KeyCode::Char('r') | KeyCode::Esc => {
                    let mut fields = fields;
                    fields.focused = YoutubeField::FileName;
                    self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields));
                }
                _ => self.modal.youtube_modal = Some(YoutubeModal::ResolvingCollision { fields, existing_path }),
            },
            YoutubeModal::Downloading { file_name, dest_path, fields } => match key.code {
                KeyCode::Esc => {}
                _ => self.modal.youtube_modal = Some(YoutubeModal::Downloading { file_name, dest_path, fields }),
            },
        }
    }

    fn handle_youtube_fields_key(&mut self, key: crossterm::event::KeyEvent, mut fields: YoutubeFieldsModal) {
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Tab | KeyCode::Down => {
                let focused = fields.focused;
                fields.focused = focused.next(&fields);
                fields.error = None;
                self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields));
            }
            KeyCode::BackTab | KeyCode::Up => {
                let focused = fields.focused;
                fields.focused = focused.prev(&fields);
                fields.error = None;
                self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields));
            }
            KeyCode::Backspace => {
                let focused = fields.focused;
                focused.value_mut(&mut fields).pop();
                self.sync_youtube_file_name(&mut fields);
                fields.error = None;
                self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields));
            }
            KeyCode::Char(c) => {
                let focused = fields.focused;
                focused.value_mut(&mut fields).push(c);
                self.sync_youtube_file_name(&mut fields);
                fields.error = None;
                self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields));
            }
            KeyCode::Enter => self.confirm_youtube_fields(fields),
            _ => self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields)),
        }
    }

    fn sync_youtube_file_name(&self, fields: &mut YoutubeFieldsModal) {
        match fields.focused {
            YoutubeField::FileName => {
                fields.file_name_overridden = !fields.file_name.is_empty();
            }
            YoutubeField::Title | YoutubeField::Artist if !fields.file_name_overridden => {
                fields.file_name = youtube::generate_file_name(&fields.artist, &fields.title);
            }
            _ => {}
        }
    }

    fn confirm_youtube_fields(&mut self, fields: YoutubeFieldsModal) {
        let directory = match resolve_directory(self.library.root(), &fields.directory) {
            Ok(dir) => dir,
            Err(message) => {
                self.modal.youtube_modal = Some(YoutubeModal::EditingFields(YoutubeFieldsModal { error: Some(message), ..fields }));
                return;
            }
        };

        if fields.file_name.trim().is_empty() {
            self.modal.youtube_modal = Some(YoutubeModal::EditingFields(YoutubeFieldsModal {
                error: Some("filename can't be empty".to_string()),
                ..fields
            }));
            return;
        }

        let dest_path = directory.join(&fields.file_name);

        if dest_path.exists() {
            self.modal.youtube_modal = Some(YoutubeModal::ResolvingCollision { fields, existing_path: dest_path });
            return;
        }

        self.start_or_await_youtube_download(fields, dest_path);
    }

    fn start_or_await_youtube_download(&mut self, fields: YoutubeFieldsModal, dest_path: PathBuf) {
        match &fields.download_status {
            DownloadStatus::Ready(temp_path) => {
                let temp_path = temp_path.clone();
                self.finalize_youtube_download(fields, temp_path, dest_path);
            }
            DownloadStatus::Pending => {
                let file_name = fields.file_name.clone();
                self.modal.youtube_modal = Some(YoutubeModal::Downloading { file_name, dest_path, fields });
            }
        }
    }

    #[cfg(feature = "youtube")]
    fn spawn_fetch_and_download(&self, url: String) {
        let tx = self.youtube_tx.clone();
        std::thread::spawn(move || {
            let Some(binaries_dir) = crate::config::youtube_binaries_dir() else {
                let _ = tx.send(DownloadEvent::Failed("could not resolve a cache directory".to_string()));
                return;
            };
            let scratch_dir = std::env::temp_dir();
            let tx_info = tx.clone();

            let result = youtube::fetch_and_download(&url, &binaries_dir, &scratch_dir, move |info| {
                let _ = tx_info.send(DownloadEvent::InfoReady { title: info.title, uploader: info.uploader });
            });

            match result {
                Ok(path) => {
                    let _ = tx.send(DownloadEvent::DownloadReady(path));
                }
                Err(e) => {
                    let _ = tx.send(DownloadEvent::Failed(e.to_string()));
                }
            }
        });
    }

    #[cfg(not(feature = "youtube"))]
    fn spawn_fetch_and_download(&self, _url: String) {
        let _ = self
            .youtube_tx
            .send(DownloadEvent::Failed("YouTube support was not built into this binary".to_string()));
    }
}

pub fn start_youtube_fields(url: String, restore: Option<YoutubeFieldsModal>) -> YoutubeFieldsModal {
    match restore {
        Some(mut fields) => {
            fields.url = url;
            fields.focused = YoutubeField::Title;
            fields.error = None;
            fields.fetch_status = FetchStatus::Pending;
            fields.download_status = DownloadStatus::Pending;
            fields
        }
        None => YoutubeFieldsModal {
            url,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            title_sort: String::new(),
            artist_sort: String::new(),
            directory: "./".to_string(),
            file_name: String::new(),
            file_name_overridden: false,
            focused: YoutubeField::Title,
            error: None,
            fetch_status: FetchStatus::Pending,
            download_status: DownloadStatus::Pending,
        },
    }
}

fn resolve_directory(root: &std::path::Path, subpath: &str) -> Result<PathBuf, String> {
    let trimmed = subpath.trim();
    let candidate = std::path::Path::new(trimmed);

    if candidate.is_absolute() {
        return Err("directory must be relative to the library root".to_string());
    }
    if candidate.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Err("directory cannot contain '..'".to_string());
    }

    let joined = root.join(candidate);
    fs::create_dir_all(&joined).map_err(|e| format!("failed to create directory: {e}"))?;

    let canonical_joined = joined.canonicalize().map_err(|e| format!("failed to resolve directory: {e}"))?;
    let canonical_root = root.canonicalize().map_err(|e| format!("failed to resolve library root: {e}"))?;

    if !canonical_joined.starts_with(&canonical_root) {
        return Err("directory must stay within the library root".to_string());
    }

    Ok(canonical_joined)
}
