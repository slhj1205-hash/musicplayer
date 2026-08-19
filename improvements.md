# Lyre Music Player — Code Review & Improvement Opportunities

## 🔴 Critical Bugs

### 1. `Queue::upcoming()` double-counts priority-queued songs

The priority queue uses a bump-at-front approach: songs added via `queue_next()` get pushed to the front of `self.order`. But `upcoming()` returns them from the priority queue **and** from the main queue, so they appear twice in the "Up Next" view.

**Fix:** Remove priority songs from the main queue when they're promoted, or track them separately so they're only emitted once.

---

### 2. `SongId::from_path` is broken

```rust
let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
```

On error, it uses the *original* (possibly relative) path in the hash, not the canonical one. Two copies of the same file in different locations get different IDs.

**Fix:** Handle the canonicalization error more explicitly — either return `None` or use a fallback that's consistent.

---

### 3. Cursor invalidation after shuffle

`Queue::shuffle()` reindexes `self.order` but `self.cursor` is only a position index into `self.order`. After shuffle, the cursor points to a different logical position — "jump to current" breaks after any shuffle.

**Fix:** After `shuffle()` or `unshuffle()`, invalidate the cursor:

```rust
pub fn shuffle(&mut self) {
    let current = self.current_song_index();
    self.order.shuffle(&mut rand::rng());
    self.cursor = None;  // ← invalidate cursor
    self.reindex(None);
}
```

---

### 4. `RowCache` key is incomplete

The cache key includes `library_revision` and `playlists_revision`, but these are only updated on **save**, not on every structural change:

- Adding a song to a playlist → `playlists.revision` doesn't change until `save()`
- Shuffling the queue → no cache invalidation
- Changing category/sort → handled via `RowsKey`, but intermediate states aren't invalidated

The cache becomes stale silently.

**Fix:** Add invalidation hooks:

```rust
pub struct RowCache {
    rows: Vec<Row>,
    key: Option<RowsKey>,
    invalidated: bool,  // ← track invalidation separately
}

impl RowCache {
    pub fn invalidate(&mut self) {
        self.key = None;
        self.invalidated = true;
    }
}
```

Then call `invalidate()` from every mutation point, and use the `invalidated` flag as the cache miss condition.

---

## 🟡 Performance Issues

### 5. `visible_song_count()` scans all rows every frame

```rust
pub fn visible_song_count(&mut self) -> usize {
    self.visible_rows().iter().filter(|r| matches!(r, Row::Song(_, _))).count()
}
```

Called every render frame. With 10k songs, this is O(n) per frame.

**Fix:** Track the count incrementally in `visible_rows()`:

```rust
pub fn visible_rows(&mut self) -> &[Row] {
    let key = self.rows_key();
    if self.rows.key.as_ref() != Some(&key) {
        let mut buffer = std::mem::take(&mut self.rows.rows);
        buffer.clear();
        self.build_rows_into(&mut buffer);
        self.rows.rows = buffer;
        self.rows.key = Some(key);
    }
    &self.rows.rows
}

pub fn visible_song_count(&mut self) -> usize {
    self.rows.rows.iter().filter(|r| matches!(r, Row::Song(_, _))).count()
}
```

---

### 6. `Sync_selection_to_rows()` double-calls `visible_rows()`

```rust
pub(super) fn sync_selection_to_rows(&mut self) {
    let len = self.visible_rows().len();  // ← rebuilds rows
    // ...
    let rows = self.rows_slice();
    let landing = nearest_song_row(self.rows_slice(), start);  // ← calls it again
```

Redundant rebuild + scan.

**Fix:** Pass the rows slice directly:

```rust
pub(super) fn sync_selection_to_rows(&mut self, rows: &[Row]) {
    let len = rows.len();
    // ...
    let landing = nearest_song_row(rows, start);
```

---

### 7. `PlaylistStore::reindex()` is O(n·m) per mutation

Every `add_song`/`remove_song`/`delete` calls `save()` → `reindex()`:

- Rebuilds sorted IDs: O(n log n)
- Rebuilds membership map from scratch: O(n·m)
- Writes entire file

For large playlists, this is a bottleneck.

**Fix:** Update membership incrementally:

```rust
pub fn add_song(&mut self, id: PlaylistId, song: SongId) -> bool {
    // ...
    self.playlists.get_mut(&id).expect("checked above").add(song);
    self.membership.entry(song).or_default().push(id);
    self.revision += 1;
    // Don't save yet — batch saves or use a separate "dirty" flag
    true
}

pub fn save(&mut self) {
    if self.dirty {
        // ...
        self.dirty = false;
    }
}
```

---

### 8. `Metadata::probe` called three times per song load

- `Song::load()` → `Metadata::probe()`
- `Song::fingerprint()` → `fs::metadata()` (redundant)
- `Song::load_with_stat()` → `Metadata::probe()` → `fs::metadata()` inside

Triple stat + triple probe.

**Fix:** Load metadata once and reuse it:

```rust
pub fn load(path: impl AsRef<Path>) -> Result<Song, Error> {
    let path = path.as_ref();
    let metadata = Metadata::probe(path)?;
    let mtime = fs::metadata(path).map(|m| mtime_secs(&m)).unwrap_or(0);
    Ok(Song::assemble(SongId::from_path(path), Arc::from(path), metadata, mtime))
}

pub fn load_with_stat(path: impl AsRef<Path>, len: u64, modified_secs: u64) -> Result<Song, Error> {
    let path = path.as_ref();
    let metadata = Metadata::probe(path)?;  // ← only one probe
    Ok(Song::assemble(SongId::compute(path, len, modified_secs), Arc::from(path), metadata, modified_secs))
}
```

Also, `SongId::from_path` already calls `fs::metadata` internally — pass the stat from the caller instead.

---

### 9. `ScanCache::save()` sorts all entries on every save

```rust
pub fn save(&self, path: &Path) {
    let mut ordered: Vec<(&PathBuf, &Entry)> = self.entries.iter().collect();
    ordered.sort_unstable_by(|a, b| a.0.cmp(b.0));
    // ...
}
```

Unnecessary — a flat file with stable insertion order is fine. The file just gets appended to each time.

**Fix:** Remove the sort:

```rust
pub fn save(&self, path: &Path) {
    let ordered: Vec<_> = self.entries.iter().collect();  // ← no sort
    let Ok(json) = serde_json::to_vec_pretty(&ordered) else { return };
    // ...
}
```

---

## 🟢 UX / Design Issues

### 10. `handle_key` drops unknown keys silently

```rust
if key.code != KeyCode::Char('n') && !is_digit {
    self.pending_number.clear();
}
```

Pressing 'h' (not a defined key) clears the pending number. The key is dropped without any status message.

**Fix:** Either accept all keys and ignore unknown ones, or emit a status message:

```rust
if !known_keys.contains(&key) {
    self.set_status("unknown key", StatusKind::Info);
    return;
}
```

---

### 11. Status messages leak memory

`StatusMessage` uses `Instant` + 4s TTL but never clears the `Vec<StatusMessage>`. Grows unboundedly over a long session.

**Fix:** Track message count and clear when empty:

```rust
pub struct StatusMessage {
    pub text: String,
    pub kind: StatusKind,
    set_at: Instant,
}

pub struct StatusManager {
    messages: Vec<StatusMessage>,
    max_messages: usize,
}

impl StatusManager {
    fn push(&mut self, text: impl Into<String>, kind: StatusKind) {
        self.messages.push(StatusMessage { text: text.into(), kind, set_at: Instant::now() });
        if self.messages.len() > self.max_messages {
            self.messages.clear();
        }
    }
}
```

---

### 12. `SongId` hash can collide

Using a simple `u64` hash of `(path, len, mtime)` means two different files with the same size and mtime collide. The `insert_song` guard only checks path equality, not ID uniqueness.

**Fix:** Use a stronger hash or add a collision check:

```rust
pub fn compute(path: &Path, len: u64, modified_secs: u64) -> SongId {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    len.hash(&mut hasher);
    modified_secs.hash(&mut hasher);
    SongId(hasher.finish())
}

// In insert_song:
if let Some(existing) = songs.get(&song.id()) {
    if existing.path() != song.path() {
        // Collision! Use a secondary check
        if existing.path() == song.path() && existing.fingerprint() == song.fingerprint() {
            // Same file — skip
        } else {
            // Different file with same hash — error/warn
        }
    }
}
```

Or use `uuid` for `SongId` (overkill but eliminates collision risk).

---

### 13. `Queue::play_id` is O(n) linear scan

For jumping to a specific song, it scans the entire `self.order` vector:

```rust
pub fn play_id(&mut self, id: SongId) -> Option<SongId> {
    // O(n) scan through self.order
    self.play_at(target)
}
```

**Fix:** Maintain a reverse index `by_id: HashMap<SongId, usize>` that tracks the position of each song in `self.order`. Update it on every mutation (shuffle, sort, next/prev). Lookup becomes O(1).

---

## Summary by Category

| Category | Count |
|----------|-------|
| **Bugs** | 4 |
| **Performance** | 5 |
| **UX / Design** | 3 |

## Recommended Order

1. **Critical bugs** (1–4) — these cause incorrect behavior
2. **Performance** (5–9) — these affect responsiveness at scale
3. **UX** (10–13) — these improve usability and robustness
