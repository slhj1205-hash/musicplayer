use std::{env, path::PathBuf};

use color_eyre::Result;

use lyre_core::{Library, PlaylistStore};

use lyre_tui::{app::App, config, Backend};

fn main() -> Result<()> {
    color_eyre::install()?;

    let dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .or_else(config::load_last_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    let cache_path = config::scan_cache_path(&dir);

    let library = match Library::scan(&dir, &cache_path) {
        Ok((library, stats)) => {
            if stats.skipped() > 0 {
                eprintln!("warning: {} file(s) could not be loaded during scan", stats.skipped());
            }
            library
        }
        Err(e) => {
            eprintln!("failed to scan {}: {e}", dir.display());
            std::process::exit(1);
        }
    };

    config::save_last_dir(library.root());

    let playlists_dir = config::data_dir()
        .map(|dir| dir.join("playlists"))
        .unwrap_or_else(|| library.root().join("playlists"));
    let (playlists, prune_stats) = PlaylistStore::load(playlists_dir, &library);
    if prune_stats.songs_removed > 0 {
        eprintln!(
            "warning: removed {} missing song(s) across {} playlist(s)",
            prune_stats.songs_removed, prune_stats.playlists_loaded
        );
    }

    let app = App::new(library, playlists, Backend::detect());

    ratatui::run(|terminal| app.run(terminal))
}
