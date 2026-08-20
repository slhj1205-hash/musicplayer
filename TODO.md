clippy lint deny panics

# Code review findings (2026-08-19)

Full pass over both crates (~6,800 lines). Overall the codebase is in good shape —
clippy is nearly silent, there is no `.unwrap()` anywhere in production code, and
errors are modeled consistently with `thiserror`. Findings below, worst first.

## Fixed already

- [x] **Playlists load from two different paths.** `src/main.rs:34-37` loaded
  playlists from `config::data_dir()` (XDG data dir, e.g.
  `~/.local/share/lyre/playlists`) on startup, with a fallback to
  `library.root().join("playlists")` if no data dir was available. But
  `finish_dir_scan` in `tui/src/app/mod.rs:165-166` (triggered by `<d>` to
  change directory) always loaded/saved from `library.root().join("playlists")`,
  ignoring `data_dir()` entirely. Fixed by adding `config::playlists_path()` --
  a single, library-root-independent function -- and switching both call sites
  (`main.rs` and `finish_dir_scan`) to use it. Playlists now live in exactly one
  place (the XDG data dir, with a `.lyre-playlists` dotfile-in-cwd fallback only
  if `$HOME` is unset) regardless of which library directory is open or how many
  times it's switched.

  Added `playlists_path_lives_under_the_data_dir_regardless_of_library_root` and
  `playlists_path_does_not_depend_on_which_library_directory_is_open` to
  `tui/tests/app_tests.rs`, mirroring the existing `scan_cache_path_*` test
  pattern. **Caveat:** these tests cover `config::playlists_path()` itself, not
  `finish_dir_scan`. I confirmed this by reverting just the `finish_dir_scan`
  call site back to the old buggy path and re-running the suite -- it still
  passed, i.e. the tests do not catch a regression there. `finish_dir_scan` is
  only reachable through the real event loop (`App::run`, which needs a live
  terminal), and `CLAUDE.md` rules out `#[cfg(test)]` unit tests inside `src/`
  as the workaround. So the regression-proofing here is structural rather than
  test-enforced: both call sites now invoke the identical zero-argument
  `config::playlists_path()`, which is a much smaller, more obviously-matching
  surface for a reviewer to check than the two differently-shaped path
  expressions that caused the original bug. Full workspace build + clippy +
  test suite (89 tests) verified green after the change.

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

## Design risk worth discussing

- [x] **`SongId` is derived from `std::collections::hash_map::DefaultHasher`**
  (`core/src/song.rs`), used as a *persisted, cross-run, cross-machine* identity
  — it's the join key for `scan_cache.json` and `playlists.json`. The stdlib
  explicitly does not guarantee `DefaultHasher`'s algorithm stays the same across
  Rust versions (only that it's deterministic *within* a given version). A
  future `rustc`/std upgrade that changes this algorithm would silently
  regenerate different `SongId`s for every song on the next scan — playlists
  would get pruned as if every song had vanished, no error, just an empty
  playlist next launch.

  **Fixed:** Switched `SongId::compute` to `fnv::FnvHasher` — FNV-1a, a
  small, dependency-free, explicitly-versioned crate with a documented,
  stable algorithm, instead of relying on `DefaultHasher`'s unspecified
  implementation. `SongId` isn't used as a `HashMap` key under adversarial
  input, so FNV's weaker collision resistance versus SipHash is not a
  concern here; it's also the fastest option on the short (~30-80 byte)
  path+len+mtime keys `compute` hashes. No migration needed or added — see
  the new "Pre-release status" section above; existing `scan_cache.json`/
  `playlists.json` files will just be treated as stale and regenerated.
  Verified: workspace builds clean, `cargo clippy --workspace --all-targets`
  shows only the pre-existing unrelated `collapsible_if` warning, and the
  full test suite (98 tests) passes.

## Cleanup, not urgent

- [x] **Booleans instead of enums for direction.** `Action::CycleCategory(bool)`,
  `Action::CycleSort(bool)` (`tui/src/keymap.rs`), `jump_page(forward: bool)`,
  `compute_jump(..., forward: bool, ...)` (`tui/src/app/navigation.rs`). Call
  sites like `Action::CycleCategory(true)` / `Action::CycleCategory(false)` read
  fine once you know the convention but require checking the definition to
  confirm `true` means forward. A small `enum Direction { Forward, Backward }`
  would make every call site self-documenting.

  **Fixed:** Replaced all `bool` direction parameters with `enum Direction`.
  The `compute_jump` function now takes `direction: Direction` and uses a
  `match` to dispatch `Forwards` vs `Backwards` behavior.
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

## Ambiguous return types on action-named functions

Same shape as the `render_song_list_panel` issue fixed via `PanelHeight`
(see the "Name the return, don't just return it" convention added to
`CLAUDE.md`): an imperative, verb-phrase function name that reads as
returning nothing, but which returns a `bool`/`Option<T>` the caller
actually depends on. Two fix shapes are on the table — pick one and apply
it consistently across all of these:

**Option A — distinct, specific enums per concept:**
- `Mutated::{Yes, No}` — shared by `PlaylistStore::rename`, `add_song`,
  `remove_song`, `rename_song_id`, `delete` (`core/src/playlist.rs:219,
  229, 242, 264, 278`), all currently `-> bool` for "did the mutation
  happen."
- `EventsChanged::{Changed, Unchanged}` — `App::drain_player_events`
  (`tui/src/app/playback.rs:10`), currently `-> bool`.
- `Selected::{Found, NotFound}` — `App::select_song_by_id`
  (`tui/src/app/navigation.rs:194`), currently `-> bool`.
- `InsertOutcome::{Inserted(SongId), Skipped}` — `insert_song`
  (`core/src/library.rs:278`), currently `-> Option<SongId>`.

**Option B — one generic `Outcome<T = ()>` reused everywhere:**
- `enum Outcome<T = ()> { Applied(T), NoOp }`, used as `-> Outcome` for the
  five `bool`-returning cases above and `-> Outcome<SongId>` for
  `insert_song`. Fewer new types, but more generic naming — less
  self-documenting than Option A at each call site (`Outcome::Applied(id)`
  vs. `InsertOutcome::Inserted(id)`).

Not included: `expire_if_stale` (`tui/src/app/state.rs:278`) — the
`if_stale` in the name already telegraphs a boolean outcome, so wrapping
it would be enum-for-enum's-sake. `PlaylistStore`'s five mutators should
be treated as one shared type regardless of which option is picked —
they're the same concept repeated five times, not five different ones.

