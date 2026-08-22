use std::{cmp::Ordering, path::Path};

use lyre_core::{PlaylistId, Song};

use super::state::{is_filtering, Category, Panel, PlaylistView, Row, Sort};
use super::App;

#[derive(Default)]
pub struct RowCache {
    rows: Vec<Row>,
    key: Option<RowsKey>,
}

#[derive(Clone, PartialEq, Eq)]
struct RowsKey {
    panel: Panel,
    view: PlaylistView,
    category: Category,
    sort: Sort,
    query: String,
    library_revision: u64,
    playlists_revision: u64,
}

impl RowCache {

    pub fn invalidate(&mut self) {
        self.key = None;
    }
}

impl App {
    fn rows_key(&self) -> RowsKey {
        let (view, category, sort, query) = match self.panel {
            Panel::Library => (
                PlaylistView::Browsing,
                self.library_panel.category,
                self.library_panel.sort,
                &self.library_panel.search_query,
            ),
            Panel::Playlists => (
                self.playlist_panel.view,
                self.playlist_panel.category,
                self.playlist_panel.sort,
                &self.playlist_panel.search_query,
            ),
        };

        RowsKey {
            panel: self.panel,
            view,
            category,
            sort,
            query: query.clone(),
            library_revision: self.library_revision,
            playlists_revision: self.playlists.revision(),
        }
    }

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
        self.visible_rows().iter().filter(|r| matches!(r, Row::Song(_, _))).count()
    }

    fn build_rows_into(&self, out: &mut Vec<Row>) {
        match self.panel {
            Panel::Library => {
                if !is_filtering(&self.library_panel.search_query) {
                    let songs: Vec<&Song> = self.library.songs_by_path().collect();
                    build_rows(songs, self.library_panel.category, self.library_panel.sort, self.library.root(), out);
                } else {
                    let query = self.library_panel.search_query.to_lowercase();
                    let terms: Vec<&str> = query.split_whitespace().collect();
                    let songs = fuzzy_filter_and_sort(self.library.songs_by_path(), &terms);
                    build_relevance_rows(songs, out);
                }
            }
            Panel::Playlists => match self.playlist_panel.view {
                PlaylistView::Browsing => {}
                PlaylistView::Viewing(id) => self.build_playlist_rows_into(id, out),
            },
        }
    }

    fn build_playlist_rows_into(&self, id: PlaylistId, out: &mut Vec<Row>) {
        let Some(playlist) = self.playlists.get(id) else { return };
        let songs_iter = playlist.songs().iter().filter_map(|&id| self.library.get(id));

        if !is_filtering(&self.playlist_panel.search_query) {
            let songs: Vec<&Song> = songs_iter.collect();
            build_rows(songs, self.playlist_panel.category, self.playlist_panel.sort, self.library.root(), out);
        } else {
            let query = self.playlist_panel.search_query.to_lowercase();
            let terms: Vec<&str> = query.split_whitespace().collect();
            let songs = fuzzy_filter_and_sort(songs_iter, &terms);
            build_relevance_rows(songs, out);
        }
    }
}

fn fuzzy_filter_and_sort<'a>(songs: impl Iterator<Item = &'a Song>, terms: &[&str]) -> Vec<&'a Song> {
    let mut scored: Vec<(&Song, u32)> =
        songs.filter_map(|song| song.fuzzy_score(terms).map(|score| (song, score))).collect();
    scored.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.sort_title().cmp(b.0.sort_title())));
    scored.into_iter().map(|(song, _)| song).collect()
}

fn build_relevance_rows(songs: Vec<&Song>, rows: &mut Vec<Row>) {
    rows.reserve(songs.len());
    rows.extend(songs.into_iter().map(|s| Row::Song(s.id(), 0)));
}

fn sort_comparator(sort: Sort) -> impl Fn(&Song, &Song) -> Ordering {
    move |a, b| match sort {
        Sort::Title => a.sort_title().cmp(b.sort_title()),
        Sort::Duration => {
            a.metadata().duration.cmp(&b.metadata().duration).then_with(|| a.sort_title().cmp(b.sort_title()))
        }
        Sort::Artist => a.sort_artist().cmp(b.sort_artist()).then_with(|| a.sort_title().cmp(b.sort_title())),
        Sort::Path => a.path().cmp(b.path()),
        Sort::DateModified => b.modified().cmp(&a.modified()).then_with(|| a.path().cmp(b.path())),
    }
}

fn relative_parent<'a>(song: &'a Song, root: &Path) -> &'a Path {
    let rel = song.path().strip_prefix(root).unwrap_or(song.path());
    rel.parent().unwrap_or(Path::new(""))
}

fn build_rows(mut songs: Vec<&Song>, category: Category, sort: Sort, root: &Path, rows: &mut Vec<Row>) {
    rows.reserve(songs.len());
    let within_group = sort_comparator(sort);

    match category {
        Category::None => {
            songs.sort_unstable_by(|a, b| within_group(a, b));
            rows.extend(songs.into_iter().map(|s| Row::Song(s.id(), 0)));
        }
        Category::Artist => {
            songs.sort_unstable_by(|a, b| a.sort_artist().cmp(b.sort_artist()).then_with(|| within_group(a, b)));

            let mut last_artist: Option<&str> = None;
            for song in songs {
                if last_artist != Some(song.artist()) {
                    rows.push(Row::Header(song.artist().to_string()));
                    last_artist = Some(song.artist());
                }
                rows.push(Row::Song(song.id(), 0));
            }
        }
        Category::Path => {
            songs.sort_unstable_by(|a, b| relative_parent(a, root).cmp(relative_parent(b, root)).then_with(|| within_group(a, b)));

            let mut last_dirs: Vec<String> = Vec::new();
            for song in songs {
                let comps: Vec<_> = relative_parent(song, root).components().collect();

                let shared_depth = comps
                    .iter()
                    .zip(last_dirs.iter())
                    .take_while(|(c, s)| c.as_os_str().to_str() == Some(s.as_str()))
                    .count();

                last_dirs.truncate(shared_depth);

                for (depth, comp) in comps.iter().enumerate().skip(shared_depth) {
                    let name = comp.as_os_str().to_string_lossy().into_owned();
                    let mut header = String::with_capacity(depth * 2 + name.len());
                    for _ in 0..depth {
                        header.push_str("  ");
                    }
                    header.push_str(&name);
                    rows.push(Row::Header(header));
                    last_dirs.push(name);
                }

                rows.push(Row::Song(song.id(), comps.len()));
            }
        }
    }
}

impl RowCache {

    pub fn rows_unchecked(&self) -> &[Row] {
        &self.rows
    }
}
