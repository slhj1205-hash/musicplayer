clippy lint deny panics

# Code review findings (2026-08-19)

Full pass over both crates (~6,800 lines). Overall the codebase is in good shape —
clippy is nearly silent, there is no `.unwrap()` anywhere in production code, and
errors are modeled consistently with `thiserror`. Findings below, worst first.

## Fixed already

- [x] **Panic risk in `Metadata::write`** (`core/src/song.rs`). The code assumed
  that if a tag didn't exist, inserting one and then calling `first_tag_mut()`
  would always succeed, backed by `.expect("inserted a tag above if none
  existed")`. But `TaggedFile::insert_tag` silently no-ops if the file format
  doesn't support that tag type at all (checked via `tag_support().is_readable()`)
  — it doesn't insert anything. For a format lofty can *read* but has no writable
  tag support for, this would panic and crash the whole TUI instead of showing an
  error. Fixed to return `Error::Unwritable` instead. Verified the fix compiles
  and the full suite (87 tests) still passes. No regression test added — would
  need a fixture format with zero tag support, which isn't practical to
  synthesize.

## Real bugs

- [ ] **Playlists load from two different paths.** `src/main.rs:34-37` loads
  playlists from `config::data_dir()` (XDG data dir, e.g.
  `~/.local/share/lyre/playlists`) on startup, with a fallback to
  `library.root().join("playlists")` if no data dir is available. But
  `finish_dir_scan` in `tui/src/app/mod.rs:165-166` (triggered by `<d>` to
  change directory) always loads/saves from `library.root().join("playlists")`,
  ignoring `data_dir()` entirely. Concretely: start the app normally, playlists
  come from the XDG dir. Press `<d>` and re-enter the *same* directory, and the
  app silently switches to a different (likely empty) playlists file — any
  playlists created from then on save somewhere different from where they
  started. Looks like `finish_dir_scan` predates the XDG data-dir fallback added
  to `main.rs` and was never updated to match. Fix: factor the
  `data_dir()`-aware path resolution out of `main.rs` into `config.rs` and use it
  in both places.

## Design risk worth discussing

- [ ] **`SongId` is derived from `std::collections::hash_map::DefaultHasher`**
  (`core/src/song.rs`), used as a *persisted, cross-run, cross-machine* identity
  — it's the join key for `scan_cache.json` and `playlists.json`. The stdlib
  explicitly does not guarantee `DefaultHasher`'s algorithm stays the same across
  Rust versions (only that it's deterministic *within* a given version). A
  future `rustc`/std upgrade that changes this algorithm would silently
  regenerate different `SongId`s for every song on the next scan — playlists
  would get pruned as if every song had vanished, no error, just an empty
  playlist next launch. Consider pinning to an explicit, versioned hash (e.g. a
  small `siphash`/`fnv`/`blake3` dependency with a documented, stable algorithm)
  instead of relying on a standard-library implementation detail. Needs a short
  design discussion since it touches the on-disk `SongId` representation — likely
  wants a one-time migration for anyone already using the app.

## Cleanup, not urgent

- [ ] **Booleans instead of enums for direction.** `Action::CycleCategory(bool)`,
  `Action::CycleSort(bool)` (`tui/src/keymap.rs`), `jump_page(forward: bool)`,
  `compute_jump(..., forward: bool, ...)` (`tui/src/app/navigation.rs`). Call
  sites like `Action::CycleCategory(true)` / `Action::CycleCategory(false)` read
  fine once you know the convention but require checking the definition to
  confirm `true` means forward. A small `enum Direction { Forward, Backward }`
  would make every call site self-documenting.
- [ ] **`render_song_list_panel` takes 13 parameters**
  (`tui/src/ui/rows.rs:217`), with `#[allow(clippy::too_many_arguments)]`
  explicitly suppressing the lint. Worth collapsing the panel-specific
  parameters (`title_prefix`, `category_label`, `sort_label`, `searching`,
  `query`, `playlist_info`) into a `PanelConfig` struct — `library.rs` and
  `playlists.rs` already assemble these from panel state right before calling
  it, so it'd be a mechanical change.
- [ ] **Dead public API**: `Playlist::from_songs` and
  `Queue::clear_priority_queue` are `pub` but never called from the TUI or
  exercised by any test. Either remove them, or — if `clear_priority_queue` is a
  half-wired feature (no keybinding exists to trigger it) — decide whether to
  finish wiring it up.
- [ ] Pre-existing `clippy::collapsible_if` in `tui/src/keymap.rs:151`
  (unrelated to anything above). Cosmetic only.
