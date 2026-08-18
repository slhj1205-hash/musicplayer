
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent};
use lyre_core::{Library, PlaylistStore};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use lyre_tui::{
    app::{App, Category, PlaylistDisplayMode, Sort},
    Backend,
};

fn bench(name: &str, iters: usize, mut f: impl FnMut()) {
    for _ in 0..3.min(iters) {
        f();
    }
    let mut samples: Vec<Duration> = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t = Instant::now();
        f();
        samples.push(t.elapsed());
    }
    samples.sort();
    let median = samples[samples.len() / 2];
    let total: Duration = samples.iter().sum();
    println!(
        "{name:<44} median {:>10.3?}  mean {:>10.3?}",
        median,
        total / samples.len() as u32
    );
}

fn main() {
    let root = std::env::args().nth(1).unwrap();
    let cache = std::env::args().nth(2).unwrap();
    let (library, _) = Library::scan(&root, &cache).unwrap();
    let n = library.len();

    let pl_dir = std::env::temp_dir().join("uibench-playlists");
    let _ = std::fs::remove_dir_all(&pl_dir);
    let (mut playlists, _) = PlaylistStore::load(&pl_dir, &library);
    let ids: Vec<_> = library.ids().collect();
    for p in 0..12 {
        let id = playlists.create(format!("Playlist {p}"));
        for (i, &song) in ids.iter().enumerate() {
            if i % 13 == p {
                playlists.add_song(id, song);
            }
        }
    }
    println!("library: {n} songs, {} playlists\n", playlists.len());

    let mut app = App::new(library, playlists, Backend::null());
    let area = Rect::new(0, 0, 120, 45);

    for category in [Category::None, Category::Artist, Category::Path] {
        app.library_panel.category = category;
        for sort in [Sort::Title, Sort::Artist, Sort::Path, Sort::Duration] {
            app.library_panel.sort = sort;
            bench(
                &format!("visible_rows [category={} sort={}]", category.label(), sort.label()),
                20,
                || {
                    app.rows.invalidate();
                    std::hint::black_box(app.visible_rows().len());
                },
            );
        }
    }

    app.library_panel.category = Category::None;
    app.library_panel.sort = Sort::Title;
    for pm in [PlaylistDisplayMode::Hidden, PlaylistDisplayMode::Count, PlaylistDisplayMode::Expanded] {
        app.library_panel.playlist_mode = pm;
        let mut buf = Buffer::empty(area);
        bench(&format!("render frame [playlist tags={}]", pm.label()), 20, || {
            (&mut app).render(area, &mut buf);
        });
    }

    app.library_panel.playlist_mode = PlaylistDisplayMode::Hidden;
    let mut buf = Buffer::empty(area);
    bench("keypress: move down (j)", 20, || {
        app.on_key(KeyEvent::from(KeyCode::Char('j')));
    });
    bench("keypress + frame (steady-state scroll)", 20, || {
        app.on_key(KeyEvent::from(KeyCode::Char('j')));
        (&mut app).render(area, &mut buf);
    });

    app.on_key(KeyEvent::from(KeyCode::Char('/')));
    let mut i = 0usize;
    bench("search keystroke + frame", 20, || {
        let c = ['s', 'i', 'l', 'e', 'n', 't'][i % 6];
        i += 1;
        app.on_key(KeyEvent::from(KeyCode::Char(c)));
        app.on_key(KeyEvent::from(KeyCode::Backspace));
        (&mut app).render(area, &mut buf);
    });
}
