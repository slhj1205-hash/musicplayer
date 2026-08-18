use std::path::{Path, PathBuf};

use crate::app_name;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Config {
    last_dir: Option<PathBuf>,
}

fn config_path() -> Option<PathBuf> {
    let dir_name = app_name::kebab_case();
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join(&dir_name).join("config.json"));
        }
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home).join(".config").join(&dir_name).join("config.json"));
    }
    None
}

/// Base directory for application data (e.g. playlists), following the
/// XDG Base Directory spec with a `$HOME/.local/share/<app>` fallback.
pub fn data_dir() -> Option<PathBuf> {
    let dir_name = app_name::kebab_case();
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
        && !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join(&dir_name));
        }
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home).join(".local").join("share").join(&dir_name));
    }
    None
}

/// Base directory for cached data (e.g. the scan cache), following the
/// XDG Base Directory spec with a `$HOME/.cache/<app>` fallback.
pub fn cache_dir() -> Option<PathBuf> {
    let dir_name = app_name::kebab_case();
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME")
        && !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join(&dir_name));
        }
    if let Ok(home) = std::env::var("HOME") {
        return Some(PathBuf::from(home).join(".cache").join(&dir_name));
    }
    None
}

/// Path to the scan cache file for a given library root. Lives under
/// `cache_dir()`, keyed by a hash of the canonicalized root so different
/// library directories don't collide. Falls back to a file inside the
/// root itself if no cache dir can be determined (no `$HOME`).
pub fn scan_cache_path(root: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};

    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    canonical.hash(&mut hasher);
    let filename = format!("{:016x}.json", hasher.finish());

    match cache_dir() {
        Some(dir) => dir.join(filename),
        None => root.join(app_name::cache_file_name()),
    }
}

pub fn load_last_dir() -> Option<PathBuf> {
    let path = config_path()?;
    let contents = std::fs::read_to_string(path).ok()?;
    let config: Config = serde_json::from_str(&contents).ok()?;
    config.last_dir.filter(|dir| dir.is_dir())
}

pub fn save_last_dir(dir: &Path) {
    let Some(path) = config_path() else {
        return;
    };
    let config = Config { last_dir: Some(dir.to_path_buf()) };
    let Ok(json) = serde_json::to_string_pretty(&config) else {
        return;
    };
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("warning: failed to create config directory: {e}");
            return;
        }
    if let Err(e) = std::fs::write(path, json) {
        eprintln!("warning: failed to persist config: {e}");
    }
}
