use std::path::{Path, PathBuf};

use fnv::FnvHasher;

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

pub fn scan_cache_path(root: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};

    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut hasher = FnvHasher::default();
    canonical.hash(&mut hasher);
    let filename = format!("{:016x}.json", hasher.finish());

    match cache_dir() {
        Some(dir) => dir.join(filename),
        None => root.join(app_name::cache_file_name()),
    }
}

pub fn playlists_path() -> PathBuf {
    match data_dir() {
        Some(dir) => dir.join("playlists"),
        None => PathBuf::from(format!(".{}-playlists", app_name::kebab_case())),
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
