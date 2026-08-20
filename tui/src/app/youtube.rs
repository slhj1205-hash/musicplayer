use std::{
    fs, path::PathBuf,
    sync::mpsc,
};

use crossterm::event::KeyCode;

use lyre_core::{youtube, InsertOutcome, MetadataEdits, Metadata, Song};

use super::state::{YoutubeField, YoutubeFieldsModal, YoutubeModal};
use super::state::StatusKind;
use super::App;

pub enum DownloadEvent {
    InfoReady(youtube::VideoInfo),
    InfoError(String),
    DownloadComplete(PathBuf),
    DownloadError(String),
}

pub(super) fn channel() -> (mpsc::Sender<DownloadEvent>, mpsc::Receiver<DownloadEvent>) {
    mpsc::channel()
}

impl App {
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
            DownloadEvent::InfoReady(info) => {
                if let Some(YoutubeModal::Fetching { url }) = &self.modal.youtube_modal {
                    self.modal.youtube_modal = Some(YoutubeModal::ConfirmingVideo { url: url.clone(), info });
                }
            }
            DownloadEvent::InfoError(message) => {
                self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input: String::new(), error: Some(message) });
            }
            DownloadEvent::DownloadComplete(path) => self.finish_youtube_download(path),
            DownloadEvent::DownloadError(message) => {
                self.set_status(format!("download failed: {message}"), StatusKind::Error);
                self.modal.youtube_modal = None;
            }
        }
    }

    fn finish_youtube_download(&mut self, path: PathBuf) {
        let Some(YoutubeModal::Downloading { fields, .. }) = self.modal.youtube_modal.take() else {
            return;
        };

        let edits = MetadataEdits {
            title: fields.title.clone(),
            artist: fields.artist.clone(),
            album: fields.album.clone(),
            genre: String::new(),
            track: String::new(),
            date: String::new(),
        };

        if let Err(e) = Metadata::write(&path, &edits) {
            self.set_status(format!("downloaded, but failed to tag: {e}"), StatusKind::Error);
            return;
        }

        let song = match Song::load(&path) {
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
                self.library_revision += 1;
                self.rows.invalidate();
                self.set_status(format!("downloaded and added: {label}"), StatusKind::Success);
                self.select_song_by_id(id);
            }
            InsertOutcome::Collision { .. } => {
                self.set_status("downloaded song already exists in the library", StatusKind::Info);
            }
        }
    }

    pub(super) fn open_youtube_modal(&mut self) {
        self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input: String::new(), error: None });
    }

    pub(super) fn handle_youtube_modal_key(&mut self, key: crossterm::event::KeyEvent) {
        let Some(modal) = self.modal.youtube_modal.take() else { return };

        match modal {
            YoutubeModal::EnteringUrl { mut url_input, .. } => match key.code {
                KeyCode::Esc => self.modal.youtube_modal = None,
                KeyCode::Enter => {
                    let url = url_input.trim().to_string();
                    if url.is_empty() {
                        self.modal.youtube_modal =
                            Some(YoutubeModal::EnteringUrl { url_input, error: Some("enter a URL first".to_string()) });
                        return;
                    }
                    self.spawn_fetch_info(url.clone());
                    self.modal.youtube_modal = Some(YoutubeModal::Fetching { url });
                }
                KeyCode::Backspace => {
                    url_input.pop();
                    self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input, error: None });
                }
                KeyCode::Char(c) => {
                    url_input.push(c);
                    self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input, error: None });
                }
                _ => self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input, error: None }),
            },
            YoutubeModal::Fetching { url } => match key.code {
                KeyCode::Esc => {}
                _ => self.modal.youtube_modal = Some(YoutubeModal::Fetching { url }),
            },
            YoutubeModal::ConfirmingVideo { url, info } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.modal.youtube_modal = Some(YoutubeModal::EditingFields(YoutubeFieldsModal {
                        url,
                        title: String::new(),
                        artist: String::new(),
                        album: String::new(),
                        directory: String::new(),
                        file_name: String::new(),
                        file_name_overridden: false,
                        focused: YoutubeField::Title,
                        error: None,
                    }));
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.modal.youtube_modal = Some(YoutubeModal::EnteringUrl { url_input: String::new(), error: None });
                }
                _ => self.modal.youtube_modal = Some(YoutubeModal::ConfirmingVideo { url, info }),
            },
            YoutubeModal::EditingFields(fields) => self.handle_youtube_fields_key(key, fields),
            YoutubeModal::ResolvingCollision { fields, existing_path } => match key.code {
                KeyCode::Char('o') => {
                    let url = fields.url.clone();
                    let file_name = fields.file_name.clone();
                    self.spawn_download(url, existing_path);
                    self.modal.youtube_modal = Some(YoutubeModal::Downloading { file_name, fields });
                }
                KeyCode::Char('r') | KeyCode::Esc => {
                    let mut fields = fields;
                    fields.focused = YoutubeField::FileName;
                    self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields));
                }
                _ => self.modal.youtube_modal = Some(YoutubeModal::ResolvingCollision { fields, existing_path }),
            },
            YoutubeModal::Downloading { file_name, fields } => match key.code {
                KeyCode::Esc => {}
                _ => self.modal.youtube_modal = Some(YoutubeModal::Downloading { file_name, fields }),
            },
        }
    }

    fn handle_youtube_fields_key(&mut self, key: crossterm::event::KeyEvent, mut fields: YoutubeFieldsModal) {
        match key.code {
            KeyCode::Esc => {}
            KeyCode::Tab | KeyCode::Down => {
                fields.focused = fields.focused.next();
                fields.error = None;
                self.modal.youtube_modal = Some(YoutubeModal::EditingFields(fields));
            }
            KeyCode::BackTab | KeyCode::Up => {
                fields.focused = fields.focused.prev();
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

        let url = fields.url.clone();
        let file_name = fields.file_name.clone();
        self.spawn_download(url, dest_path);
        self.modal.youtube_modal = Some(YoutubeModal::Downloading { file_name, fields });
    }

    fn spawn_fetch_info(&self, url: String) {
        let tx = self.youtube_tx.clone();
        std::thread::spawn(move || {
            let Some(binaries_dir) = crate::config::youtube_binaries_dir() else {
                let _ = tx.send(DownloadEvent::InfoError("could not resolve a cache directory".to_string()));
                return;
            };
            match youtube::fetch_info(&url, &binaries_dir) {
                Ok(info) => {
                    let _ = tx.send(DownloadEvent::InfoReady(info));
                }
                Err(e) => {
                    let _ = tx.send(DownloadEvent::InfoError(e.to_string()));
                }
            }
        });
    }

    fn spawn_download(&self, url: String, dest_path: PathBuf) {
        let tx = self.youtube_tx.clone();
        std::thread::spawn(move || {
            let Some(binaries_dir) = crate::config::youtube_binaries_dir() else {
                let _ = tx.send(DownloadEvent::DownloadError("could not resolve a cache directory".to_string()));
                return;
            };
            match youtube::download_audio(&url, &binaries_dir, &dest_path) {
                Ok(()) => {
                    let _ = tx.send(DownloadEvent::DownloadComplete(dest_path));
                }
                Err(e) => {
                    let _ = tx.send(DownloadEvent::DownloadError(e.to_string()));
                }
            }
        });
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
