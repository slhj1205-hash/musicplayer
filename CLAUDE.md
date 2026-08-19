# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

Lyre is a terminal music player. Cargo workspace with three crates:

- `core` (`lyre-core`) — library scanning, tags, playback backend, queue, playlists. No UI, no terminal dependency.
- `tui` (`lyre-tui`) — the ratatui interface: rendering, input handling, app state.
- root (`lyre`) — thin binary (`src/main.rs`) that wires the two together.

## Build / test / run

Building requires GStreamer dev headers:
```
sudo apt-get install libgstreamer1.0-dev pkg-config
```
If they're missing, `Backend::detect()` still lets the app run silently via `NullBackend` at runtime — but compiling `gstreamer-sys` still needs the headers present.

```
cargo build                          # whole workspace
cargo test --workspace               # everything — do this before considering any change done
cargo test -p lyre-core              # core only
cargo test -p lyre-tui               # tui only
cargo run -- /path/to/music          # run against a specific library dir
```

Tests live as integration-style suites in `core/tests/core_tests.rs` and `tui/tests/app_tests.rs`, not `#[cfg(test)]` modules scattered through `src/`. Match that: put new tests in those files, name them as full sentences describing the behavior (`shift_a_opens_the_song_actions_modal_not_lowercase_p`, not `test_shift_a`).

`core/examples/scanbench.rs` and `tui/examples/uibench.rs` exist for manual perf checks — run with `cargo run --example scanbench` when a change might affect scan or render cost.

## Performance targets — read before optimizing anything

**Core (`lyre-core`): design for comfortably under 10k songs.** Users are very unlikely to exceed that. This means:
- Plain `HashMap<SongId, Song>` + `Vec<SongId>` for the library, O(n) linear scans in `Queue` (`play_id`, `upcoming`), and O(n log n) sorts are all *fine* — don't reach for indexes, B-trees, disk-backed structures, or incremental diffing to shave time off operations that are already sub-millisecond at 10k items. `Library::scan` already parallelizes tag-probing with rayon; that's the one place scale genuinely mattered, and it's handled.
- Prefer clear, obviously-correct code over cleverness here. If a "faster" approach would add real complexity, it's very likely not worth it at this scale — check with the user before introducing it.

**TUI (`lyre-tui`): UI/UX is the priority, performance is a secondary constraint, not a free pass.** Concretely:
- Don't rebuild the full row list every frame — `RowCache` (`app/row_builder.rs`) exists specifically to avoid that; it recomputes only when its `RowsKey` (panel/view/category/sort/query/revision) changes. Any new state that affects row *contents or ordering* needs to be added to `RowsKey`, or the cache will silently show stale rows.
- Per-row rendering (`ui/rows.rs`) already only touches the visible viewport window, not the full list — keep it that way; don't introduce a per-row operation that scans the whole library.
- Fuzzy search re-scores every song on each keystroke (`row_builder.rs`). That's fine at 10k songs on any reasonable machine — don't add caching/debouncing for it unless a profile actually shows it's a problem.
- None of this means skip UX for performance. If the more polished interaction costs an extra O(n) pass over ≤10k items once per keypress, take it — that's cheap. The bar is "would this be sluggish at 10k songs on modest hardware," not "is this the theoretically fastest way."

## Conventions specific to this codebase

- **No comments**, anywhere, unless explicitly asked for. The existing code is comment-free by convention (names and small functions carry the meaning); match that when adding code, don't introduce explanatory comments.
- **Single source of truth over parallel literals.** This codebase has been bitten twice by the same failure shape: a hardcoded string/number duplicating a fact that lived somewhere else, which drifted out of sync (stale key-hint text in `footer.rs`/`playlists.rs`; a hardcoded header-width constant that didn't account for all `Sort`/`Category` label lengths). When adding UI text or layout logic that depends on enum variants or key bindings, derive it from the existing data rather than writing a second copy:
  - `tui/src/keymap.rs` is the single source for every key binding — its `BINDINGS` table drives both input dispatch (`keymap::lookup`) and every displayed hint (`keymap::display_for`, `keymap::help_rows`, `keymap::FOOTER_HINT`). Add new bindings there, not as a new `match` arm plus a separately-typed hint string.
  - `Category::ALL` / `Sort::ALL` (`app/state.rs`) are the source for cycling (`next`/`prev` are derived from position in `ALL`, not hand-written) and for the header padding width in `ui/style.rs` (`sort_label_width()`, computed from the actual label lengths, not a hardcoded constant). Adding a new `Sort` or `Category` variant should mean: add the variant, add its `label()` arm, add it to `ALL` — nothing else needs manual updating.
  - When you add a new variant to either enum and it's not immediately obvious how `next`/`prev`/width/dispatch pick it up, that's a sign something is still hardcoded elsewhere and should be fixed the same way, not worked around.
- **Name the return, don't just return it.** If a function's name reads as an imperative action (`render_x`, `drain_x`, `insert_x`) and it also returns a value beyond `()` or a bare `Result<(), Error>`, wrap that return in a small named type instead of a raw `usize`/`bool`/`Option<T>` — e.g. `fn render_song_list_panel(...) -> PanelHeight` instead of `-> usize`. For a `bool` that reports an outcome (found it / changed something / mutation applied), prefer a two-variant `enum` with meaningful names over a raw `bool` — `enum EventsChanged { Changed, Unchanged }`, not `-> bool`. The type should live next to the function that returns it, and call sites should match on or destructure it rather than treating it as an opaque primitive. If a function name and return type together are still ambiguous without a comment, that's a signal the function is doing two things and should probably be split, not commented.
- **Regression tests for anything that was ever visibly wrong.** When fixing a bug (stale text, a layout jump, a wrong keybinding), add a test that fails on the old code and passes on the fix — don't just fix it and move on. Confirm the test actually catches the regression (temporarily revert the fix locally and watch it fail) before calling the change done.
- **Match existing formatting exactly** — this project doesn't use a `rustfmt.toml`/`clippy.toml`, so "the existing style in the file you're editing" is the style. Don't reformat unrelated code while making a change.

## Delivering changes

When asked to implement something in this repo, verify the change compiles and `cargo test --workspace` passes before handing it back. Prefer producing a patch file (`git diff` output) the user can `git apply` over describing changes only in prose — that's been the working pattern in this project and it lets the user review and apply on their own machine.
