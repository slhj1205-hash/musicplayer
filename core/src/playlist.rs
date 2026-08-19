use std::{
    collections::HashMap,
    fmt, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{library::Library, song::SongId};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Playlist {
    id: PlaylistId,
    name: String,
    songs: Vec<SongId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct PlaylistId(uuid::Uuid);

impl PlaylistId {
    pub fn new() -> PlaylistId {
        PlaylistId(uuid::Uuid::new_v4())
    }
}

impl Default for PlaylistId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PlaylistId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Playlist {
    pub fn new(id: PlaylistId, name: impl Into<String>) -> Playlist {
        Playlist { id, name: name.into(), songs: Vec::new() }
    }

    pub fn from_songs(id: PlaylistId, name: impl Into<String>, songs: Vec<SongId>) -> Playlist {
        Playlist { id, name: name.into(), songs }
    }

    pub fn id(&self) -> PlaylistId {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn rename(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    pub fn songs(&self) -> &[SongId] {
        &self.songs
    }
    pub fn len(&self) -> usize {
        self.songs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn add(&mut self, song: SongId) {
        self.songs.push(song);
    }

    pub fn remove_at(&mut self, position: usize) -> Option<SongId> {
        if position < self.songs.len() {
            Some(self.songs.remove(position))
        } else {
            None
        }
    }

    pub fn remove_all(&mut self, song: SongId) {
        self.songs.retain(|&id| id != song);
    }

    pub fn contains(&self, song: SongId) -> bool {
        self.songs.contains(&song)
    }

    pub fn move_to(&mut self, from: usize, to: usize) {
        if from >= self.songs.len() || to >= self.songs.len() {
            return;
        }
        let song = self.songs.remove(from);
        self.songs.insert(to, song);
    }

    pub(crate) fn retain_songs(&mut self, keep: impl Fn(SongId) -> bool) {
        self.songs.retain(|&id| keep(id));
    }

    fn rename_song_id(&mut self, old: SongId, new: SongId) {
        for id in &mut self.songs {
            if *id == old {
                *id = new;
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PruneStats {
    pub playlists_loaded: usize,
    pub songs_removed: usize,
}

const FLUSH_AFTER: Duration = Duration::from_millis(750);

pub struct PlaylistStore {
    path: PathBuf,
    playlists: HashMap<PlaylistId, Playlist>,
    sorted_ids: Vec<PlaylistId>,
    membership: HashMap<SongId, Vec<PlaylistId>>,
    revision: u64,
    dirty: bool,
    dirty_since: Option<Instant>,
}

impl PlaylistStore {
    pub fn load(path: impl AsRef<Path>, library: &Library) -> (PlaylistStore, PruneStats) {
        let path = path.as_ref().to_path_buf();
        let mut stats = PruneStats::default();
        let mut playlists = HashMap::new();

        let loaded: Vec<Playlist> = fs::read(&path)
            .ok()
            .and_then(|contents| serde_json::from_slice(&contents).ok())
            .unwrap_or_default();

        let mut pruned = false;
        for mut playlist in loaded {
            let before = playlist.len();
            playlist.retain_songs(|id| library.contains(id));
            let removed = before - playlist.len();
            if removed > 0 {
                stats.songs_removed += removed;
                pruned = true;
            }

            stats.playlists_loaded += 1;
            playlists.insert(playlist.id(), playlist);
        }

        let mut store = PlaylistStore {
            path,
            playlists,
            sorted_ids: Vec::new(),
            membership: HashMap::new(),
            revision: 0,
            dirty: false,
            dirty_since: None,
        };
        store.reindex();

        if pruned {
            store.save();
        }
        (store, stats)
    }

    pub fn empty(path: impl Into<PathBuf>) -> PlaylistStore {
        PlaylistStore {
            path: path.into(),
            playlists: HashMap::new(),
            sorted_ids: Vec::new(),
            membership: HashMap::new(),
            revision: 0,
            dirty: false,
            dirty_since: None,
        }
    }

    #[inline]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn get(&self, id: PlaylistId) -> Option<&Playlist> {
        self.playlists.get(&id)
    }

    pub fn len(&self) -> usize {
        self.playlists.len()
    }
    pub fn is_empty(&self) -> bool {
        self.playlists.is_empty()
    }

    #[inline]
    pub fn ids_sorted_by_name(&self) -> &[PlaylistId] {
        &self.sorted_ids
    }

    #[inline]
    pub fn containing(&self, song: SongId) -> &[PlaylistId] {
        self.membership.get(&song).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn contains(&self, playlist: PlaylistId, song: SongId) -> bool {
        self.containing(song).contains(&playlist)
    }

    pub fn create(&mut self, name: impl Into<String>) -> PlaylistId {
        let id = PlaylistId::new();
        self.playlists.insert(id, Playlist::new(id, name));

        self.reindex();
        self.mark_dirty();
        id
    }

    pub fn rename(&mut self, id: PlaylistId, name: impl Into<String>) -> bool {
        let Some(playlist) = self.playlists.get_mut(&id) else {
            return false;
        };
        playlist.rename(name);
        self.reindex();
        self.mark_dirty();
        true
    }

    pub fn add_song(&mut self, id: PlaylistId, song: SongId) -> bool {
        if !self.playlists.contains_key(&id) || self.contains(id, song) {
            return false;
        }

        self.playlists.get_mut(&id).expect("checked above").add(song);
        self.membership.entry(song).or_default().push(id);

        self.revision += 1;
        self.mark_dirty();
        true
    }

    pub fn remove_song(&mut self, id: PlaylistId, song: SongId) -> bool {
        let Some(playlist) = self.playlists.get_mut(&id) else {
            return false;
        };
        let before = playlist.len();
        playlist.remove_all(song);
        if playlist.len() == before {
            return false;
        }

        if let Some(entry) = self.membership.get_mut(&song) {
            entry.retain(|&other| other != id);
            if entry.is_empty() {
                self.membership.remove(&song);
            }
        }

        self.revision += 1;
        self.mark_dirty();
        true
    }

    pub fn rename_song_id(&mut self, old: SongId, new: SongId) -> bool {
        if old == new || !self.membership.contains_key(&old) {
            return false;
        }

        for playlist in self.playlists.values_mut() {
            playlist.rename_song_id(old, new);
        }

        self.reindex();
        self.save();
        true
    }

    pub fn delete(&mut self, id: PlaylistId) -> bool {
        let Some(playlist) = self.playlists.remove(&id) else {
            return false;
        };
        for song in playlist.songs() {
            if let Some(entry) = self.membership.get_mut(song) {
                entry.retain(|&other| other != id);
                if entry.is_empty() {
                    self.membership.remove(song);
                }
            }
        }
        self.reindex();
        self.mark_dirty();
        true
    }

    fn reindex(&mut self) {
        self.sorted_ids.clear();
        self.sorted_ids.extend(self.playlists.keys().copied());
        self.sorted_ids.sort_by_key(|id| (self.playlists[id].name().to_lowercase(), *id));

        self.membership.clear();
        for id in &self.sorted_ids {
            for &song in self.playlists[id].songs() {
                let entry = self.membership.entry(song).or_default();
                if !entry.contains(id) {
                    entry.push(*id);
                }
            }
        }

        self.revision += 1;
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.dirty_since.get_or_insert_with(Instant::now);
    }

    pub fn flush_if_due(&mut self) {
        if self.dirty && self.dirty_since.is_some_and(|since| since.elapsed() >= FLUSH_AFTER) {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        self.save();
        self.dirty = false;
        self.dirty_since = None;
    }

    fn save(&mut self) {
        let all: Vec<&Playlist> = self.sorted_ids.iter().filter_map(|id| self.playlists.get(id)).collect();
        let Ok(json) = serde_json::to_vec_pretty(&all) else { return };

        if let Err(e) = crate::atomic::write(&self.path, &json) {
            eprintln!("warning: failed to save playlists to {}: {e}", self.path.display());
        }
    }
}

impl Drop for PlaylistStore {
    fn drop(&mut self) {
        self.flush();
    }
}
