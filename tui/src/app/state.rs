use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use ratatui::widgets::ListState;

use lyre_core::{needs_romanization, MetadataEdits, PlaylistId, SongId};

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
    pub metadata_modal: Option<MetadataEditModal>,
    pub youtube_modal: Option<YoutubeModal>,
    pub romanized_artist_confirm: Option<RomanizedArtistConfirmModal>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

pub fn is_filtering(query: &str) -> bool {
    query.split_whitespace().next().is_some()
}

pub(crate) fn cycle<T: Copy + PartialEq>(all: &[T], current: T, delta: isize) -> T {
    let len = all.len() as isize;
    let idx = all.iter().position(|x| *x == current).unwrap_or(0) as isize;
    all[(idx + delta).rem_euclid(len) as usize]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Library,
    Playlists,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
pub enum MetadataField {
    Title,
    TitleSort,
    Artist,
    ArtistSort,
    Album,
    Genre,
    Track,
    Date,
}

impl MetadataField {
    pub const ALL: &'static [MetadataField] = &[
        MetadataField::Title,
        MetadataField::TitleSort,
        MetadataField::Artist,
        MetadataField::ArtistSort,
        MetadataField::Album,
        MetadataField::Genre,
        MetadataField::Track,
        MetadataField::Date,
    ];

    pub fn visible(edits: &MetadataEdits) -> Vec<MetadataField> {
        Self::ALL.iter().copied().filter(|field| field.is_visible(edits)).collect()
    }

    fn is_visible(&self, edits: &MetadataEdits) -> bool {
        match self {
            MetadataField::TitleSort => needs_romanization(&edits.title),
            MetadataField::ArtistSort => needs_romanization(&edits.artist),
            _ => true,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            MetadataField::Title => "Title",
            MetadataField::TitleSort => "Title (roman.)",
            MetadataField::Artist => "Artist",
            MetadataField::ArtistSort => "Artist (roman.)",
            MetadataField::Album => "Album",
            MetadataField::Genre => "Genre",
            MetadataField::Track => "Track",
            MetadataField::Date => "Date",
        }
    }

    pub fn next(self, edits: &MetadataEdits) -> MetadataField {
        cycle(&Self::visible(edits), self, 1)
    }

    pub fn prev(self, edits: &MetadataEdits) -> MetadataField {
        cycle(&Self::visible(edits), self, -1)
    }

    pub fn value<'a>(&self, edits: &'a MetadataEdits) -> &'a str {
        match self {
            MetadataField::Title => &edits.title,
            MetadataField::TitleSort => &edits.title_sort,
            MetadataField::Artist => &edits.artist,
            MetadataField::ArtistSort => &edits.artist_sort,
            MetadataField::Album => &edits.album,
            MetadataField::Genre => &edits.genre,
            MetadataField::Track => &edits.track,
            MetadataField::Date => &edits.date,
        }
    }

    pub fn value_mut<'a>(&self, edits: &'a mut MetadataEdits) -> &'a mut String {
        match self {
            MetadataField::Title => &mut edits.title,
            MetadataField::TitleSort => &mut edits.title_sort,
            MetadataField::Artist => &mut edits.artist,
            MetadataField::ArtistSort => &mut edits.artist_sort,
            MetadataField::Album => &mut edits.album,
            MetadataField::Genre => &mut edits.genre,
            MetadataField::Track => &mut edits.track,
            MetadataField::Date => &mut edits.date,
        }
    }
}

pub struct MetadataEditModal {
    pub song: SongId,
    pub edits: MetadataEdits,
    pub original_artist_sort: String,
    pub focused: MetadataField,
    pub error: Option<String>,
}

pub struct RomanizedArtistConfirmModal {
    pub artist_display: String,
    pub artist_sort_key: String,
    pub value: String,
    pub reference_song: SongId,
    pub count: usize,
}

pub enum YoutubeModal {
    EnteringUrl { url_input: String, error: Option<String>, restore: Option<YoutubeFieldsModal> },
    EditingFields(YoutubeFieldsModal),
    ResolvingCollision { fields: YoutubeFieldsModal, existing_path: PathBuf },
    Downloading { file_name: String, dest_path: PathBuf, fields: YoutubeFieldsModal },
}

pub enum FetchStatus {
    Pending,
    Ready { title: String, uploader: Option<String> },
}

pub enum DownloadStatus {
    Pending,
    Ready(PathBuf),
}

pub struct YoutubeFieldsModal {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub title_sort: String,
    pub artist_sort: String,
    pub directory: String,
    pub file_name: String,
    pub file_name_overridden: bool,
    pub focused: YoutubeField,
    pub error: Option<String>,
    pub fetch_status: FetchStatus,
    pub download_status: DownloadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YoutubeField {
    Title,
    TitleSort,
    Artist,
    ArtistSort,
    Album,
    Directory,
    FileName,
}

impl YoutubeField {
    pub const ALL: &'static [YoutubeField] = &[
        YoutubeField::Title,
        YoutubeField::TitleSort,
        YoutubeField::Artist,
        YoutubeField::ArtistSort,
        YoutubeField::Album,
        YoutubeField::Directory,
        YoutubeField::FileName,
    ];

    pub fn visible(fields: &YoutubeFieldsModal) -> Vec<YoutubeField> {
        Self::ALL.iter().copied().filter(|field| field.is_visible(fields)).collect()
    }

    fn is_visible(&self, fields: &YoutubeFieldsModal) -> bool {
        match self {
            YoutubeField::TitleSort => needs_romanization(&fields.title),
            YoutubeField::ArtistSort => needs_romanization(&fields.artist),
            _ => true,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            YoutubeField::Title => "Title",
            YoutubeField::TitleSort => "Title (roman.)",
            YoutubeField::Artist => "Artist",
            YoutubeField::ArtistSort => "Artist (roman.)",
            YoutubeField::Album => "Album",
            YoutubeField::Directory => "Directory",
            YoutubeField::FileName => "Filename",
        }
    }

    pub fn next(self, fields: &YoutubeFieldsModal) -> YoutubeField {
        cycle(&Self::visible(fields), self, 1)
    }

    pub fn prev(self, fields: &YoutubeFieldsModal) -> YoutubeField {
        cycle(&Self::visible(fields), self, -1)
    }

    pub fn value<'a>(&self, fields: &'a YoutubeFieldsModal) -> &'a str {
        match self {
            YoutubeField::Title => &fields.title,
            YoutubeField::TitleSort => &fields.title_sort,
            YoutubeField::Artist => &fields.artist,
            YoutubeField::ArtistSort => &fields.artist_sort,
            YoutubeField::Album => &fields.album,
            YoutubeField::Directory => &fields.directory,
            YoutubeField::FileName => &fields.file_name,
        }
    }

    pub fn value_mut<'a>(&self, fields: &'a mut YoutubeFieldsModal) -> &'a mut String {
        match self {
            YoutubeField::Title => &mut fields.title,
            YoutubeField::TitleSort => &mut fields.title_sort,
            YoutubeField::Artist => &mut fields.artist,
            YoutubeField::ArtistSort => &mut fields.artist_sort,
            YoutubeField::Album => &mut fields.album,
            YoutubeField::Directory => &mut fields.directory,
            YoutubeField::FileName => &mut fields.file_name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChooseActionField {
    AddToPlaylist,
    RemoveFromPlaylist,
    CreatePlaylist,
}

impl ChooseActionField {
    pub const ALL: &'static [ChooseActionField] =
        &[ChooseActionField::AddToPlaylist, ChooseActionField::RemoveFromPlaylist, ChooseActionField::CreatePlaylist];
}

pub enum SidePanel {
    AddToPlaylist {
        options: Vec<PlaylistId>,
        pinned: Vec<PlaylistId>,
        list_state: ListState,
    },
    RemoveFromPlaylist {
        options: Vec<PlaylistId>,
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
