use std::{
    collections::hash_map::DefaultHasher,
    fmt, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use lofty::{
    file::{AudioFile, TaggedFileExt},
    probe::Probe,
    tag::{items::Timestamp, Accessor},
};

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "aac", "aif", "aifc", "aiff", "ape", "flac", "m4a", "m4b", "m4p", "mp1", "mp2", "mp3", "mp4", "mpc", "mpp", "oga",
    "ogg", "opus", "spx", "wav", "wave", "wv",
];

const MAX_EXTENSION_LEN: usize = 5;

pub fn is_supported_audio(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    if ext.is_empty() || ext.len() > MAX_EXTENSION_LEN || !ext.is_ascii() {
        return false;
    }
    let mut buf = [0u8; MAX_EXTENSION_LEN];
    let bytes = ext.as_bytes();
    buf[..bytes.len()].copy_from_slice(bytes);
    buf[..bytes.len()].make_ascii_lowercase();
    let lower = &buf[..bytes.len()];
    SUPPORTED_EXTENSIONS.iter().any(|known| known.as_bytes() == lower)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct SongId(u64);

impl SongId {
    pub fn compute(path: &Path, len: u64, modified_secs: u64) -> SongId {
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        len.hash(&mut hasher);
        modified_secs.hash(&mut hasher);
        SongId(hasher.finish())
    }

    pub fn from_path(path: &Path) -> SongId {
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let (len, modified) = Song::fingerprint(&canonical).unwrap_or((0, 0));
        SongId::compute(&canonical, len, modified)
    }
}

impl fmt::Display for SongId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    pub title: Option<Arc<str>>,
    pub artist: Option<Arc<str>>,
    pub album: Option<Arc<str>>,
    pub genre: Option<Arc<str>>,
    pub track: Option<u32>,
    #[serde(with = "timestamp_serde")]
    pub date: Option<Timestamp>,
    pub duration: Duration,
}

mod timestamp_serde {
    use lofty::tag::items::Timestamp;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    pub fn serialize<S>(value: &Option<Timestamp>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.map(|t| t.to_string()).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Timestamp>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: Option<String> = Option::deserialize(deserializer)?;
        raw.map(|s| Timestamp::from_str(&s).map_err(serde::de::Error::custom))
            .transpose()
    }
}

impl Metadata {

    pub fn probe(path: &Path) -> Result<Metadata, Error> {
        let probed = Probe::open(path).map_err(|source| Error::Probe { path: path.to_path_buf(), source })?;
        let tagged_file = probed.read().map_err(|source| Error::Read { path: path.to_path_buf(), source })?;
        Ok(Metadata::from_tagged_file(&tagged_file))
    }

    fn from_tagged_file(tf: &lofty::file::TaggedFile) -> Self {
        let tag = tf.primary_tag().or_else(|| tf.first_tag());
        Metadata {
            title: tag.and_then(|t| t.title()).map(|c| Arc::from(c.as_ref())),
            artist: tag.and_then(|t| t.artist()).map(|c| Arc::from(c.as_ref())),
            album: tag.and_then(|t| t.album()).map(|c| Arc::from(c.as_ref())),
            genre: tag.and_then(|t| t.genre()).map(|c| Arc::from(c.as_ref())),
            track: tag.and_then(|t| t.track()),
            date: tag.and_then(|t| t.date()),
            duration: tf.properties().duration(),
        }
    }
}

pub const UNKNOWN_TITLE: &str = "Unknown Title";
pub const UNKNOWN_ARTIST: &str = "Unknown Artist";
pub const UNKNOWN_ALBUM: &str = "Unknown Album";

#[derive(Debug)]
struct SortKeys {
    title: Box<str>,
    artist: Box<str>,
    album: Box<str>,
}

impl SortKeys {
    fn build(title: &str, artist: &str, album: &str) -> SortKeys {
        SortKeys {
            title: title.chars().flat_map(char::to_lowercase).collect(),
            artist: artist.chars().flat_map(char::to_lowercase).collect(),
            album: album.chars().flat_map(char::to_lowercase).collect(),
        }
    }

    fn title(&self) -> &str {
        &self.title
    }
    fn artist(&self) -> &str {
        &self.artist
    }
    fn album(&self) -> &str {
        &self.album
    }
}

#[derive(Clone, Debug)]
pub struct Song {
    id: SongId,
    path: Arc<Path>,
    metadata: Arc<Metadata>,
    keys: Arc<SortKeys>,

    mtime_secs: u64,
}

impl Song {

    pub fn load(path: impl AsRef<Path>) -> Result<Song, Error> {
        let path = path.as_ref();
        let metadata = Metadata::probe(path)?;
        let mtime = fs::metadata(path).map(|m| mtime_secs(&m)).unwrap_or(0);
        Ok(Song::assemble(SongId::from_path(path), Arc::from(path), metadata, mtime))
    }

    pub fn load_with_stat(path: impl AsRef<Path>, len: u64, modified_secs: u64) -> Result<Song, Error> {
        let path = path.as_ref();
        let metadata = Metadata::probe(path)?;
        Ok(Song::assemble(
            SongId::compute(path, len, modified_secs),
            Arc::from(path),
            metadata,
            modified_secs,
        ))
    }

    pub fn from_cached_with_stat(path: PathBuf, len: u64, modified_secs: u64, metadata: Metadata) -> Song {
        let id = SongId::compute(&path, len, modified_secs);
        Song::assemble(id, Arc::from(path), metadata, modified_secs)
    }

    pub fn from_cached(path: PathBuf, metadata: Metadata) -> Song {
        let id = SongId::from_path(&path);
        let mtime = fs::metadata(&path).map(|m| mtime_secs(&m)).unwrap_or(0);
        Song::assemble(id, Arc::from(path), metadata, mtime)
    }

    fn assemble(id: SongId, path: Arc<Path>, metadata: Metadata, mtime_secs: u64) -> Song {
        let title = metadata.title.as_deref().unwrap_or_else(|| stem_of(&path));
        let artist = metadata.artist.as_deref().unwrap_or(UNKNOWN_ARTIST);
        let album = metadata.album.as_deref().unwrap_or(UNKNOWN_ALBUM);
        let keys = Arc::new(SortKeys::build(title, artist, album));

        Song { id, path, metadata: Arc::new(metadata), keys, mtime_secs }
    }

    #[inline]
    pub fn id(&self) -> SongId {
        self.id
    }
    #[inline]
    pub fn path(&self) -> &Path {
        &self.path
    }
    #[inline]
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    #[inline]
    pub fn modified(&self) -> u64 {
        self.mtime_secs
    }

    pub fn title(&self) -> &str {
        self.metadata.title.as_deref().unwrap_or_else(|| stem_of(&self.path))
    }
    pub fn artist(&self) -> &str {
        self.metadata.artist.as_deref().unwrap_or(UNKNOWN_ARTIST)
    }
    pub fn album(&self) -> &str {
        self.metadata.album.as_deref().unwrap_or(UNKNOWN_ALBUM)
    }

    #[inline]
    pub fn sort_title(&self) -> &str {
        self.keys.title()
    }

    #[inline]
    pub fn sort_artist(&self) -> &str {
        self.keys.artist()
    }

    #[inline]
    pub fn sort_album(&self) -> &str {
        self.keys.album()
    }

    #[inline]
    pub fn matches(&self, needle: &str) -> bool {
        needle.is_empty()
            || self.keys.title().contains(needle)
            || self.keys.artist().contains(needle)
            || self.keys.album().contains(needle)
    }

    #[inline]
    pub fn fuzzy_term_score(&self, term: &str) -> Option<u32> {
        if term.is_empty() {
            return Some(0);
        }
        let title_field = self.keys.title();
        let artist_field = self.keys.artist();
        let album_field = self.keys.album();

        let title = fuzzy_subsequence_score(term, title_field)
            .map(|s| normalize_by_length(s, title_field.chars().count()) * 3 / 2);
        let artist = fuzzy_subsequence_score(term, artist_field)
            .map(|s| normalize_by_length(s, artist_field.chars().count()));
        let album = fuzzy_subsequence_score(term, album_field)
            .map(|s| normalize_by_length(s, album_field.chars().count()));

        [title, artist, album].into_iter().flatten().max()
    }

    pub fn fuzzy_score(&self, terms: &[&str]) -> Option<u32> {
        let mut total = 0u32;
        for term in terms {
            total += self.fuzzy_term_score(term)?;
        }
        Some(total)
    }

    pub fn fingerprint(path: &Path) -> Option<(u64, u64)> {
        let meta = fs::metadata(path).ok()?;
        Some((meta.len(), mtime_secs(&meta)))
    }
}

fn normalize_by_length(score: u32, field_len: usize) -> u32 {
    score * 100 / (field_len as u32 + 8)
}

fn is_word_boundary(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => matches!(c, ' ' | '-' | '_' | '(' | ')' | '.' | '/' | '\''),
    }
}

fn fuzzy_subsequence_score(pattern: &str, target: &str) -> Option<u32> {
    let pattern_len = pattern.chars().count();
    if pattern_len == 0 {
        return Some(0);
    }
    let target_len = target.chars().count();
    if target_len < pattern_len {
        return None;
    }

    let mut pattern_chars = pattern.chars();
    let mut current_pattern_char = pattern_chars.next();
    let mut prev_pattern_char: Option<char> = None;
    let mut prev_target_char: Option<char> = None;
    let mut matched = 0usize;
    let mut score = 0u32;
    let mut start_match = false;
    let mut remaining = target_len;

    for target_char in target.chars() {
        remaining -= 1;

        if let Some(pattern_char) = current_pattern_char {
            if target_char == pattern_char {
                start_match = true;
                score += 10;
                if prev_target_char.is_some() && prev_target_char == prev_pattern_char {
                    score += 10;
                }
                if is_word_boundary(prev_target_char) {
                    score += 15;
                }
                prev_pattern_char = Some(pattern_char);
                matched += 1;
                current_pattern_char = pattern_chars.next();
                if current_pattern_char.is_none() {
                    return Some(score);
                }
            } else if start_match {
                score = score.saturating_sub(1);
            }
        }

        prev_target_char = Some(target_char);

        if remaining < pattern_len - matched {
            return None;
        }
    }

    None
}

pub(crate) fn mtime_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn stem_of(path: &Path) -> &str {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or(UNKNOWN_TITLE)
}

impl PartialEq for Song {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Song {}
impl Hash for Song {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Display for Song {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {} ({})", self.artist(), self.title(), self.album())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not open or identify audio file at {path}: {source}")]
    Probe {
        path: PathBuf,
        #[source]
        source: lofty::error::LoftyError,
    },
    #[error("failed to read tags from {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: lofty::error::LoftyError,
    },
}
