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

# YouTube (yt-dlp) download integration — plan (2026-08-20, revised 2026-08-20)

Not started. Lets the user paste a YouTube URL, confirm it resolved to the
right video, fill in tags and a filename by hand, and download the audio
straight into the library. Downloading-then-scanning was chosen over live
streaming: `SongId` is derived from a local path+len+mtime
(`core/src/song.rs`), and `Player::play` canonicalizes the song's path into
a `file://` URI (`core/src/player.rs`) — both assume a real file on disk.
Streaming would mean teaching `AudioBackend` a second source type and giving
up tag-based metadata for no real benefit here, so this plan always lands a
tagged file in the library and lets the existing scan/insert path take it
from there.

Revision note: the section below was checked against the `yt-dlp` crate's
actual source (v2.8.2, github.com/boul2gom/yt-dlp) rather than assumed from
its name. Three corrections came out of that: the info struct's fields
aren't as non-optional as first drafted, "download audio" is genuinely two
API calls not one, and the license needs a decision before this gets built.
Everything else in the original plan held up.

## License — needs a decision before starting

- [ ] The `yt-dlp` crate is **GPL-3.0-only** (`license = "GPL-3.0-only"` in
  its own `Cargo.toml`). This repo currently has no `LICENSE` file and no
  `license` field in any `Cargo.toml`, so there's nothing formally in
  conflict yet, but a GPL-3.0-only dependency has real implications for how
  this project can be licensed and redistributed once it does declare a
  license. Since this is now an unconditional dependency of every build (see
  below), it applies to every distributed binary, not just an opt-in
  variant. Confirm with the user whether that's acceptable before writing
  any code against the crate, rather than discovering it after the fact.

## Dependency

- [ ] Add the `yt-dlp` crate to `core/Cargo.toml` as an **unconditional**
  dependency — no Cargo feature gate. Every default `cargo build` includes
  it; there is no youtube-less build variant.
  ```toml
  [dependencies]
  yt-dlp = { version = "2.8.2", default-features = false, features = ["rustls"] }
  tokio = { version = "1", features = ["rt"] }
  ```
  `default-features = false` still drops the `cache-memory` (moka) default,
  which this one-shot per-download use case gets no benefit from — nothing
  here calls `fetch_video_infos` twice for the same URL within a process
  lifetime — and `rustls` still swaps `reqwest`'s TLS backend to a pure-Rust
  implementation instead of a system OpenSSL dependency. Those two trims are
  worth keeping regardless of the feature-gate decision.
  **Explicitly noting the tradeoff being made here**, since it reverses this
  file's own earlier reasoning: `CLAUDE.md`'s "Conventions" section points
  to `fnv` being chosen over `DefaultHasher` specifically to keep the
  dependency tree small, and the original draft of this plan extended that
  reasoning to gating `yt-dlp` behind a feature. Making it unconditional
  means every `cargo build` — including for people who will never touch
  this feature — now pulls in `tokio`, `reqwest`, `id3`, `mp4ameta`,
  `chrono`, `regex`, `uuid`, `zip`, `tar`, `xz2`, `sha2`, and takes on
  the crate's own reported ~1–2 minute build time, on top of the existing
  `libgstreamer1.0-dev` system requirement. If build time or binary size
  regresses noticeably, a feature gate is the easy fix to revisit — but
  per this instruction, ship unconditional first.

## `core/src/youtube.rs` — new module

- [ ] `fn fetch_info(url: &str, binaries_dir: &Path) -> Result<VideoInfo, Error>`
  — info-only yt-dlp call (no download), via `Downloader::fetch_video_infos`.
  Returns:
  ```rust
  pub struct VideoInfo {
      pub title: String,
      pub uploader: Option<String>,
      pub duration: Option<Duration>,
  }
  ```
  Corrected from the original draft: the crate's `Video` struct
  (`yt_dlp::model::Video`) has `uploader: Option<String>` and
  `duration: Option<i64>` (seconds) — YouTube doesn't guarantee either is
  present (e.g. a channel that's hidden its name, or a livestream/premiere
  with no fixed duration). `title` is the only one of the three that's a
  bare `String` on `Video`. The confirmation screen (`ConfirmingVideo` below)
  should render `Option::None` as an explicit "unknown", not silently
  substitute empty string — same reasoning as `generate_file_name`'s "no
  fallback placeholder text" rule below, applied to display instead of
  filenames.
  This is display-only, to let the user confirm the link resolved to the
  right video. **It is never used to populate the editable Title/Artist/Album
  fields** — those are always typed by hand (see "Metadata fields" below).
- [ ] Reject the video at the `ConfirmingVideo` step (see "Flow" below) if
  `Video::is_live == Some(true)`: a livestream has no fixed end and no
  finished file to download, which conflicts with this feature's entire
  "download once, then insert a stable file" design (see the intro
  paragraph above). Surface this as an `InfoError` variant rather than
  silently letting `Fetching` proceed into a download that can't complete
  normally — `DownloadEvent::InfoError(String)` already has a slot for this,
  no new event variant needed.
- [ ] `fn download_audio(url: &str, binaries_dir: &Path, dest_path: &Path) -> Result<(), Error>`
  — **corrected: this is two `yt-dlp` calls chained, not one.**
  `Downloader::download_audio_stream_to_path` fetches whatever YouTube's
  best *available* audio format actually is — in practice Opus or AAC in a
  WebM/M4A container, never literally MP3, since YouTube does not serve raw
  MP3 streams. Saving that under a `.mp3` filename would produce a file with
  the wrong container/codec despite the extension — lofty (or any strict
  decoder) would either misparse or reject it. Getting a real MP3 requires a
  second, explicit transcode step:
  1. Download the best audio stream to a temporary path in the same
     directory as `dest_path` (`tempfile` is already a workspace
     dependency) via `download_audio_stream_to_path`.
  2. Transcode that temp file to `dest_path` via
     `Downloader::postprocess_video_to_path` with
     `PostProcessConfig::new().with_audio_codec(AudioCodec::MP3)` — despite
     the "video" in its name, this function is a generic FFmpeg argument
     builder (confirmed by reading `build_ffmpeg_command` in the crate
     source: it only adds `-c:v`/video-only flags when a video codec is
     explicitly set in the config) and works correctly on an audio-only
     input when no video options are set.
  3. Delete the temp file (`std::fs::remove_file`, best-effort — log and
     continue on failure, don't fail the whole download over cleanup).
- [ ] Both functions are synchronous on the outside. Internally each builds
  a single-threaded `tokio::runtime::Runtime` and `.block_on`s the `yt_dlp`
  crate's async calls, so nothing outside this module needs to know tokio
  is involved.
- [ ] `binaries_dir` is where the crate caches its self-managed `yt-dlp`/
  `ffmpeg` binaries. Confirmed by reading `Downloader::with_new_binaries` in
  the crate source: it checks whether `yt-dlp`/`ffmpeg` already exist at the
  target paths before downloading anything, so calling it on every
  `fetch_info`/`download_audio` invocation is cheap after the first run —
  the original plan's "downloaded once, reused after" claim holds.
  Resolved by the caller (see "Config" below) and passed in as a parameter,
  same pattern as `Library::scan(root, cache_path)` already uses for its
  cache path, so `core` stays UI-independent and doesn't reach into XDG env
  vars itself.
- [ ] Own `thiserror` `Error` enum, matching the style of `song::Error` /
  `player::Error` (structured variants with `#[source]`, not a string). Wrap
  `yt_dlp::Error` (also a real `thiserror` enum, so this is a clean
  `#[source]` chain, not string-matching) plus a variant for the
  is-live rejection above.

## Filename generation — pure function in `core`

- [ ] `pub fn generate_file_name(artist: &str, title: &str) -> String` in
  `core/src/youtube.rs`. Always produces a `.mp3` filename (mp3-only for
  now, per decision below).
- [ ] Algorithm, fixed by two worked examples:
  - Artist `John leSmith's` + Title `38 cats` → `JohnLeSmiths-38Cats.mp3`
  - Title `Rock-n-Roll` → `RockNRoll` (within whatever field it's in)
  - Rule: **split each field into words on whitespace *and* hyphens** (both
    are word boundaries) — **but not on apostrophes**, which are stripped in
    place rather than treated as a boundary.
  - Capitalize only the first character of each resulting word; leave the
    rest of the word untouched (so `leSmith's` → `LeSmith's`, the internal
    capital `S` is preserved).
  - Strip any remaining non-alphanumeric characters from each word (drops
    the apostrophe: `LeSmith's` → `LeSmiths`).
  - Concatenate all words within one field with no separator
    (`Rock`+`N`+`Roll` → `RockNRoll`; `John`+`LeSmiths` → `JohnLeSmiths`).
  - Join the artist-chunk and title-chunk with a single `-`.
  - Append `.mp3`.
  - Verify against both worked examples above before considering this done.
- [ ] No fallback text for an empty artist/title — an empty or
  partially-empty auto-generated name is fine, because the user can always
  override the Filename field by hand (see modal flow below). Don't invent
  placeholder strings for this.
- [ ] Sentence-named test cases in `core/tests/core_tests.rs`, e.g.:
  - `generate_file_name_capitalizes_each_word_and_strips_apostrophes`
  - `generate_file_name_splits_words_on_hyphens_but_not_apostrophes`
  - `generate_file_name_drops_a_word_made_entirely_of_punctuation`
  - `generate_file_name_handles_an_empty_artist_or_title`
  These need no I/O and no yt-dlp — pure `&str -> String`, so they're cheap
  to get exhaustively right up front rather than fixing edge cases one at a
  time later.

## Background thread + channel (new infrastructure)

- [ ] Nothing in this codebase currently uses a background thread —
  `Library::scan` blocks the calling thread directly, which is fine for a
  local disk scan but not for a multi-second network download. This feature
  introduces the first `std::thread::spawn` + `mpsc` channel in the project.
- [ ] `DownloadEvent` enum sent from the background thread to the TUI:
  ```rust
  pub enum DownloadEvent {
      InfoReady(VideoInfo),
      InfoError(String),
      DownloadComplete(PathBuf),
      DownloadError(String),
  }
  ```
- [ ] The info fetch (step 2 below) and the real download (step 7 below)
  are each spawned as their own thread/job when triggered; the app polls the
  receiving end of the channel once per tick, the same shape
  `Player::poll_events` already uses for backend events — this fits the
  existing event-loop pattern instead of inventing a new one.

## `Library::insert` — new public method (correction from earlier discussion)

`core/src/library.rs:278` already has a **private** free function
`fn insert_song(songs: &mut HashMap<SongId, Song>, song: Song, skipped: &mut usize) -> Option<SongId>`,
used only internally by `scan`. It is not currently callable from outside
`Library`, despite being referenced in the "Ambiguous return types" section
above — that section is about renaming its return type someday, not about
an existing public API.

- [ ] Add:
  ```rust
  pub enum InsertOutcome {
      Inserted(SongId),
      Collision { existing: SongId },
  }

  impl Library {
      pub fn insert(&mut self, song: Song) -> InsertOutcome {
          ...
      }
  }
  ```
  Same collision-check logic as the existing private `insert_song` (warn
  and skip on a path mismatch under the same `SongId`), wrapped as a
  `&mut self` method and made `pub`, rather than reimplemented. Whether the
  private free function ends up delegating to this new method, or is left
  standing alone, is a small implementation-time call, not a planning one.
- [ ] A full rescan after every download was considered and rejected —
  it would re-probe every file in the library (via rayon) just to add one
  song, and would visibly pause the TUI each time. `insert` avoids that.

## Metadata fields — never sourced from yt-dlp

- [ ] Title/Artist/Album are **always typed by hand** by the user. yt-dlp is
  used only for (a) the info-only confirmation step and (b) the actual
  audio download — never to prefill these fields. This is a deliberate
  choice, not a gap to fill in later.
- [ ] Tag writing reuses the exact path the existing metadata-edit modal
  already uses (`tui/src/app/metadata.rs`, `core/src/library.rs`
  `update_metadata`): build a `lyre_core::MetadataEdits` from the modal's
  Title/Artist/Album (Genre/Track/Date left blank — the modal doesn't
  collect them), call `Metadata::write(&downloaded_path, &edits)` directly
  (the same function `update_metadata` calls), then `Song::load(&path)`,
  then the new `Library::insert`. No new tag-writing code.

## Directory field — must stay under `library.root()`

- [ ] The directory input is a **relative** subpath of `library.root()`,
  not a free path.
- [ ] Reject/strip `..` components and reject absolute paths.
- [ ] After joining `library.root()` + the typed subpath, canonicalize and
  confirm the result is still a descendant of `library.root()` before doing
  anything with it — same shape of check you'd want anywhere user-typed text
  becomes a filesystem path.
- [ ] `create_dir_all` the resolved directory at confirm-time if it doesn't
  exist yet.
- [ ] Default: empty (downloads straight into the root), or the last-used
  subdirectory.

## Collision handling

- [ ] If `directory/file_name.mp3` already exists at confirm-time, do not
  silently overwrite or silently rename. Show a dedicated modal state
  (`YoutubeModal::ResolvingCollision`) informing the user of the conflict,
  with two explicit choices:
  - **overwrite** — proceed with the existing path.
  - **rename** — return to the fields modal with focus on the Filename
    field so the user can retype it.
- [ ] Checked once at confirm-time (an `fs::exists` / metadata check), not
  on every keystroke while the filename is being edited.

## TUI modal state (`tui/src/app/state.rs`)

Three-stage modal, following the existing `MetadataEditModal`/
`MetadataField` shape (field enum with `ALL`/`label`/`next`/`prev`/`value`/
`value_mut`, `focused` field on the modal, `Tab` to cycle, `KeyCode::Char`
pushed into the focused field's `String`) rather than inventing a new
pattern:

```rust
pub enum YoutubeModal {
    EnteringUrl { url_input: String, error: Option<String> },
    Fetching { url: String },
    ConfirmingVideo { url: String, info: VideoInfo },
    EditingFields(YoutubeFieldsModal),
    ResolvingCollision { fields: YoutubeFieldsModal, existing_path: PathBuf },
    Downloading { file_name: String },
}

pub struct YoutubeFieldsModal {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub directory: String,
    pub file_name: String,
    pub file_name_overridden: bool,
    pub focused: YoutubeField,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YoutubeField { Title, Artist, Album, Directory, FileName }
```

- [ ] `YoutubeField` gets `ALL`/`label`/`next`/`prev`/`value`/`value_mut`
  exactly like `MetadataField` (`tui/src/app/state.rs`) — reuse the
  existing `cycle()` helper, don't write a second one.
- [ ] `file_name_overridden` starts `false` (filename is auto-derived from
  Title+Artist via `generate_file_name` on every edit to either field).
  Typing directly into the Filename field sets it to `true`, which stops
  the auto-sync until the field is cleared back to empty. Same shape as the
  existing `dir_input`/`editing_dir` toggle in `DirScanState`.
- [ ] Add `pub youtube_modal: Option<YoutubeModal>` (or equivalent) to
  `ModalState` alongside `metadata_modal`.

## Flow (end to end)

1. **Keybind** — new `Action::OpenYoutubeModal` added to `keymap.rs`'s
   `BINDINGS` table (the single source for both dispatch and displayed key
   hints — don't hand-write a second hint string). Opens
   `YoutubeModal::EnteringUrl`.
2. User pastes/types the URL, `Enter` → spawn the background info-only
   fetch (`fetch_info`), state → `Fetching`.
3. Background thread sends `DownloadEvent::InfoReady(VideoInfo)` (or
   `InfoError` — including the is-live rejection from `fetch_info` above).
   App polls this each tick, same as `Player::poll_events`. On success,
   state → `ConfirmingVideo`, rendered read-only: title / uploader /
   duration, rendering `None` on the latter two as "unknown" rather than
   blank, with an explicit "is this the right video?" prompt.
4. `y`/confirm → `EditingFields`, all of Title/Artist/Album/Directory/
   FileName start **empty** — none of them are seeded from step 3.
   `n`/`Esc` → back to `EnteringUrl` with the input cleared.
5. User fills in fields, `Tab` cycling focus. Filename auto-regenerates
   from Title+Artist while `file_name_overridden == false`.
6. Confirm → validate `directory` stays under `library.root()` (see
   above) → check if the resolved file already exists.
   - If it exists → `ResolvingCollision` (see above), looping back into
     `EditingFields` on "rename" or proceeding on "overwrite".
   - If not → proceed directly.
7. Spawn `download_audio` on a background thread with the final resolved
   path. State → `Downloading`.
8. `DownloadEvent::DownloadComplete(path)` (or `DownloadError`) polled each
   tick. On success: `Metadata::write(&path, &edits)` →
   `Song::load(&path)` → `Library::insert(song)` (see above) → close the
   modal, select the newly inserted song.

## Config (`tui/src/config.rs`)

- [ ] New functions alongside the existing `data_dir()`/`cache_dir()`/
  `scan_cache_path()`/`playlists_path()`: something like
  `youtube_binaries_dir() -> Option<PathBuf>` (under `cache_dir()`) for
  where `yt-dlp`/`ffmpeg` get cached. Resolved here in `tui`, passed into
  `core::youtube::fetch_info`/`download_audio` as parameters — `core` stays
  UI-independent, matching the existing split (same reasoning as
  `Library::scan(root, cache_path)` taking its cache path as an argument).

## Decisions already made (do not re-litigate without asking)

- MP3 only, for now — no format choice in the modal. Concretely this means
  a download-then-transcode (not download-then-rename) pipeline, since
  YouTube never serves audio already in an MP3 container — see
  `download_audio` above.
- Filename generation always has a working override via the Filename
  field; no fallback placeholder text needed for empty inputs.
- Word-splitting boundary for filename generation is whitespace **and**
  hyphens, not apostrophes — see the two worked examples above.
- Info-first-then-download-second: the user must see and confirm the
  resolved video before any download starts.
- Title/Artist/Album are always hand-typed, never pulled from yt-dlp.
- The directory field is a hard constraint (must resolve under
  `library.root()`), not just a default suggestion.
- On a filename collision, ask the user (overwrite vs. rename) — never
  silently pick one.
- Livestreams (`Video::is_live == Some(true)`) are rejected at the
  confirmation step, not attempted.

## Testing

- [ ] `generate_file_name` — sentence-named pure-function tests in
  `core/tests/core_tests.rs`, no network needed (see above).
- [ ] `Library::insert` — collision behavior (`Collision` vs `Inserted`),
  mirroring the existing `insert_song` scan-time tests if any exist.
- [ ] Directory validation — path-escape rejection (`..`, absolute paths)
  as pure-function tests, no filesystem needed beyond a tempdir.
- [ ] Real yt-dlp network calls (`fetch_info`, `download_audio`) are not
  suitable for the normal `cargo test --workspace` run — mark them
  `#[ignore]` for manual/local verification only. For `download_audio`
  specifically, verify the *output* is actually a valid MP3 (e.g. `lofty`
  can open it and report an MP3 file type), not just that the call
  succeeded — the two-step download-then-transcode above is exactly the
  kind of thing that can silently degrade to "produces a file" without
  producing the *right kind* of file.
- [ ] Per `CLAUDE.md`: full `cargo test --workspace` must stay green before
  this is considered done. No separate feature-gated build to check — the
  default `cargo build` and `cargo test --workspace` runs are the only
  configuration, since `yt-dlp` is an unconditional dependency now.
