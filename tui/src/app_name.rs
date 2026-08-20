#![allow(dead_code)]

pub const APP_NAME: &str = "Lyre";

pub fn snake_case() -> String {
    slug('_')
}

pub fn kebab_case() -> String {
    slug('-')
}

fn slug(separator: char) -> String {
    APP_NAME.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>().join(&separator.to_string())
}

pub fn cache_file_name() -> String {
    format!(".{}-cache.json", kebab_case())
}

pub fn playlists_file_name() -> String {
    format!(".{}-playlists.json", kebab_case())
}
