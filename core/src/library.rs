use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use rayon::prelude::*;

use crate::{
    scan_cache::{Entry, Probed, ScanCache},
    song::{self, is_supported_audio, mtime_secs, Metadata, MetadataEdits, Song, SongId},
};

pub struct Library {
    root: PathBuf,
    songs: HashMap<SongId, Song>,
    by_path: Vec<SongId>,
}

impl Library {
    pub fn scan(root: impl AsRef<Path>, cache_path: impl AsRef<Path>) -> Result<(Library, ScanStats), Error> {
        let root = root.as_ref();
        let cache_path = cache_path.as_ref();

        if !root.exists() {
            return Err(Error::PathNotFound(root.to_path_buf()));
        }
        if !root.is_dir() {
            return Err(Error::NotADirectory(root.to_path_buf()));
        }
        let root = root.canonicalize().map_err(|_| Error::PathNotFound(root.to_path_buf()))?;

        let cache = ScanCache::load(cache_path);
        let mut stats = ScanStats::default();

        let mut files = Vec::new();
        collect_files(&root, &mut files, &mut stats);

        files.sort_unstable();
        stats.files_considered = files.len();

        let outcomes: Vec<Outcome> = files.par_iter().map(|path| probe_file(path, &root, &cache)).collect();

        let mut songs: HashMap<SongId, Song> = HashMap::with_capacity(files.len());
        let mut by_path: Vec<SongId> = Vec::with_capacity(files.len());
        let mut next_cache = ScanCache::new();
        let mut cache_changed = false;

        for (path, outcome) in files.into_iter().zip(outcomes) {
            let Outcome { size, mtime, result } = outcome;

            let relative = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();

            match result {
                ProbeResult::Unstattable => {
                    stats.skipped_files += 1;
                    continue;
                }
                ProbeResult::Unreadable { freshly_probed } => {
                    stats.skipped_files += 1;
                    if freshly_probed {
                        stats.reprobed += 1;
                        cache_changed = true;
                    } else {
                        stats.cache_hits += 1;
                    }
                    next_cache.insert(relative, Entry { size, mtime, probed: Probed::Unreadable });
                }
                ProbeResult::Tags { metadata, freshly_probed } => {
                    if freshly_probed {
                        stats.reprobed += 1;
                        cache_changed = true;
                    } else {
                        stats.cache_hits += 1;
                    }
                    next_cache.insert(relative, Entry { size, mtime, probed: Probed::Tags(metadata.clone()) });

                    let song = Song::from_cached_with_stat(path, size, mtime, metadata);
                    if let Some(id) = insert_song(&mut songs, song, &mut stats.skipped_files) {
                        by_path.push(id);
                    }
                }
            }
        }

        if next_cache.len() != cache.len() {
            cache_changed = true;
        }
        if cache_changed {
            next_cache.save(cache_path);
        }

        debug_assert_eq!(by_path.len(), songs.len());

        Ok((Library { root, songs, by_path }, stats))
    }

    pub fn empty(root: impl Into<PathBuf>) -> Library {
        Library { root: root.into(), songs: HashMap::new(), by_path: Vec::new() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn get(&self, id: SongId) -> Option<&Song> {
        self.songs.get(&id)
    }

    pub fn contains(&self, id: SongId) -> bool {
        self.songs.contains_key(&id)
    }

    pub fn len(&self) -> usize {
        self.songs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    #[inline]
    pub fn ids_by_path(&self) -> &[SongId] {
        &self.by_path
    }

    pub fn songs_by_path(&self) -> impl Iterator<Item = &Song> + '_ {
        self.by_path.iter().filter_map(|id| self.songs.get(id))
    }

    pub fn ids(&self) -> impl Iterator<Item = SongId> + '_ {
        self.songs.keys().copied()
    }

    pub fn update_metadata(&mut self, id: SongId, edits: &MetadataEdits) -> Result<SongId, UpdateMetadataError> {
        let Some(song) = self.songs.get(&id) else {
            return Err(UpdateMetadataError::NotFound);
        };
        let path = song.path().to_path_buf();

        Metadata::write(&path, edits)?;

        let updated = Song::load(&path)?;
        let new_id = updated.id();

        if let Some(pos) = self.by_path.iter().position(|&existing| existing == id) {
            self.by_path[pos] = new_id;
        }
        self.songs.remove(&id);
        self.songs.insert(new_id, updated);

        Ok(new_id)
    }

    pub fn insert(&mut self, song: Song) -> InsertOutcome {
        let id = song.id();
        if let Some(existing) = self.songs.get(&id) {
            if existing.path() != song.path() {
                eprintln!(
                    "warning: SongId collision between {} and {} -- keeping the first, skipping the second",
                    existing.path().display(),
                    song.path().display()
                );
            }
            return InsertOutcome::Collision { existing: id };
        }
        self.songs.insert(id, song);
        self.by_path.push(id);
        InsertOutcome::Inserted(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted(SongId),
    Collision { existing: SongId },
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateMetadataError {
    #[error("song not found in library")]
    NotFound,
    #[error(transparent)]
    Metadata(#[from] song::Error),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ScanStats {

    pub cache_hits: usize,

    pub reprobed: usize,

    pub skipped_files: usize,

    pub unreadable_dirs: usize,

    pub files_considered: usize,
}

impl ScanStats {

    pub fn skipped(&self) -> usize {
        self.skipped_files + self.unreadable_dirs
    }
}

fn collect_files(dir: &Path, files: &mut Vec<PathBuf>, stats: &mut ScanStats) {
    let mut pending = vec![dir.to_path_buf()];

    while let Some(current) = pending.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("warning: failed to read directory {}: {e}", current.display());
                stats.unreadable_dirs += 1;
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("warning: failed to read a directory entry: {e}");
                    stats.unreadable_dirs += 1;
                    continue;
                }
            };

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => {
                    eprintln!("warning: failed to stat {}: {e}", entry.path().display());
                    stats.skipped_files += 1;
                    continue;
                }
            };

            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                pending.push(entry.path());
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let path = entry.path();

            if is_supported_audio(&path) {
                files.push(path);
            }
        }
    }
}

struct Outcome {
    size: u64,
    mtime: u64,
    result: ProbeResult,
}

enum ProbeResult {
    Tags { metadata: Metadata, freshly_probed: bool },
    Unreadable { freshly_probed: bool },
    Unstattable,
}

fn probe_file(path: &Path, root: &Path, cache: &ScanCache) -> Outcome {
    let Some(meta) = fs::metadata(path).ok() else {
        return Outcome { size: 0, mtime: 0, result: ProbeResult::Unstattable };
    };
    let (size, mtime) = (meta.len(), mtime_secs(&meta));

    let relative = path.strip_prefix(root).unwrap_or(path);

    match cache.get_fresh(relative, size, mtime) {
        Some(Probed::Tags(metadata)) => {
            return Outcome {
                size,
                mtime,
                result: ProbeResult::Tags { metadata: metadata.clone(), freshly_probed: false },
            };
        }
        Some(Probed::Unreadable) => {
            return Outcome { size, mtime, result: ProbeResult::Unreadable { freshly_probed: false } };
        }
        None => {}
    }

    let probed = Metadata::probe(path);

    match probed {
        Ok(metadata) => Outcome { size, mtime, result: ProbeResult::Tags { metadata, freshly_probed: true } },
        Err(_) => Outcome { size, mtime, result: ProbeResult::Unreadable { freshly_probed: true } },
    }
}

fn insert_song(songs: &mut HashMap<SongId, Song>, song: Song, skipped: &mut usize) -> Option<SongId> {
    if let Some(existing) = songs.get(&song.id()) {
        if existing.path() != song.path() {
            eprintln!(
                "warning: SongId collision between {} and {} -- keeping the first, skipping the second",
                existing.path().display(),
                song.path().display()
            );
            *skipped += 1;
        }
        return None;
    }
    let id = song.id();
    songs.insert(id, song);
    Some(id)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("path does not exist: {}", .0.display())]
    PathNotFound(PathBuf),
    #[error("not a directory: {}", .0.display())]
    NotADirectory(PathBuf),
}
