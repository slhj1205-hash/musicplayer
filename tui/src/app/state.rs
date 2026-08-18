use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use ratatui::widgets::ListState;

use lyre_core::{PlaylistId, SongId};

#[derive(Default)]
pub struct DirScanState {
    pub dir_input: String,
    pub editing_dir: bool,
    pub pending_scan: Option<PathBuf>,
}

#[derive(Default)]
pub struct LibraryPanelState {
    pub list_state: ListState,
    pub search_query: String,
    pub searching: bool,
    pub category: Category,
    pub sort: Sort,
    pub playlist_mode: PlaylistDisplayMode,

    pub page_height: usize,
}

#[derive(Default)]
pub struct PlaylistPanelState {
    pub view: PlaylistView,
    pub list_state: ListState,
    pub search_query: String,
    pub searching: bool,
    pub category: Category,
    pub sort: Sort,

    pub page_height: usize,
}

#[derive(Default)]
pub struct ModalState {
    pub confirming_quit: bool,
    pub confirming_remove: Option<(PlaylistId, SongId)>,
    pub showing_help: bool,
    pub song_modal: Option<SongModal>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    #[default]
    None,
    Artist,
    Path,
}

impl Category {
    pub const ALL: &'static [Category] = &[Category::None, Category::Artist, Category::Path];

    pub fn label(&self) -> &'static str {
        match self {
            Category::None => "none",
            Category::Artist => "artist",
            Category::Path => "path",
        }
    }

    pub fn next(self) -> Category {
        cycle(Self::ALL, self, 1)
    }

    pub fn prev(self) -> Category {
        cycle(Self::ALL, self, -1)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Title,
    Duration,
    Artist,
    Path,

    DateModified,
}

impl Sort {
    pub const ALL: &'static [Sort] = &[Sort::Title, Sort::Duration, Sort::Artist, Sort::Path, Sort::DateModified];

    pub fn label(&self) -> &'static str {
        match self {
            Sort::Title => "title",
            Sort::Duration => "duration",
            Sort::Artist => "artist",
            Sort::Path => "path",
            Sort::DateModified => "date modified",
        }
    }

    pub fn next(self) -> Sort {
        cycle(Self::ALL, self, 1)
    }

    pub fn prev(self) -> Sort {
        cycle(Self::ALL, self, -1)
    }
}

fn cycle<T: Copy + PartialEq>(all: &[T], current: T, delta: isize) -> T {
    let len = all.len() as isize;
    let idx = all.iter().position(|x| *x == current).unwrap_or(0) as isize;
    all[(idx + delta).rem_euclid(len) as usize]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Library,
    Playlists,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistDisplayMode {
    #[default]
    Hidden,
    Count,
    Expanded,
}

impl PlaylistDisplayMode {
    pub fn cycle(self) -> PlaylistDisplayMode {
        match self {
            PlaylistDisplayMode::Hidden => PlaylistDisplayMode::Count,
            PlaylistDisplayMode::Count => PlaylistDisplayMode::Expanded,
            PlaylistDisplayMode::Expanded => PlaylistDisplayMode::Hidden,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PlaylistDisplayMode::Hidden => "hidden",
            PlaylistDisplayMode::Count => "count",
            PlaylistDisplayMode::Expanded => "names",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSource {
    Library,
    Playlist(PlaylistId),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistView {
    #[default]
    Browsing,
    Viewing(PlaylistId),
}

#[derive(Debug, Clone)]
pub enum Row {
    Header(String),
    Song(SongId, usize),
}

pub struct SongModal {
    pub song: SongId,
    pub selected: ChooseActionField,
    pub name_input: String,
    pub side: Option<SidePanel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChooseActionField {
    AddToPlaylist,
    CreatePlaylist,
}

pub enum SidePanel {
    AddToPlaylist {
        options: Vec<PlaylistId>,
        pinned: Vec<PlaylistId>,
        list_state: ListState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Success,
    Error,
}

const STATUS_TTL: Duration = Duration::from_secs(4);

pub struct StatusMessage {
    pub text: String,
    pub kind: StatusKind,
    set_at: Instant,
}

impl StatusMessage {
    pub(super) fn new(text: impl Into<String>, kind: StatusKind) -> StatusMessage {
        StatusMessage { text: text.into(), kind, set_at: Instant::now() }
    }

    pub(super) fn expire_if_stale(&mut self) -> bool {
        if self.kind != StatusKind::Error && !self.text.is_empty() && self.set_at.elapsed() > STATUS_TTL {
            self.text.clear();
            return true;
        }
        false
    }
}

pub(super) fn heading_selected_message(heading: &str) -> String {
    format!("\"{heading}\" is a heading -- select a song under it")
}
