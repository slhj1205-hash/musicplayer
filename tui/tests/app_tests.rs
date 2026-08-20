use crossterm::event::{KeyCode, KeyEvent};
use lyre_core::{Library, PlaylistStore};
use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use lyre_tui::{
    app::{App, Category, MetadataField, Panel, PlaylistView, Row, Sort},
    config, ui::sort_title,
    Backend,
};

fn wav(title: &str, artist: &str, samples: usize) -> Vec<u8> {
    let mut info = b"INFO".to_vec();
    for (key, value) in [(b"INAM", title), (b"IART", artist)] {
        let mut data = value.as_bytes().to_vec();
        data.push(0);
        if data.len() % 2 == 1 {
            data.push(0);
        }
        info.extend_from_slice(key);
        info.extend_from_slice(&(data.len() as u32).to_le_bytes());
        info.extend_from_slice(&data);
    }
    let mut list = b"LIST".to_vec();
    list.extend_from_slice(&(info.len() as u32).to_le_bytes());
    list.extend_from_slice(&info);

    let mut fmt = Vec::new();
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&1u16.to_le_bytes());
    fmt.extend_from_slice(&8000u32.to_le_bytes());
    fmt.extend_from_slice(&16000u32.to_le_bytes());
    fmt.extend_from_slice(&2u16.to_le_bytes());
    fmt.extend_from_slice(&16u16.to_le_bytes());
    let mut fmt_chunk = b"fmt ".to_vec();
    fmt_chunk.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    fmt_chunk.extend_from_slice(&fmt);

    let pcm = vec![0u8; samples * 2];
    let mut data_chunk = b"data".to_vec();
    data_chunk.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    data_chunk.extend_from_slice(&pcm);

    let mut body = b"WAVE".to_vec();
    body.extend_from_slice(&fmt_chunk);
    body.extend_from_slice(&list);
    body.extend_from_slice(&data_chunk);

    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

struct Harness {
    _dir: tempfile::TempDir,
    app: App,
}

fn harness() -> Harness {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().join("music");

    for (artist, tracks) in [
        ("Alpha", [("Anchor", 4000usize), ("Azure", 400usize)]),
        ("Beta", [("Beacon", 4000usize), ("Bright", 400usize)]),
        ("Gamma", [("Glimmer", 4000usize), ("Grove", 400usize)]),
    ] {
        let d = root.join(artist);
        std::fs::create_dir_all(&d).unwrap();
        for (i, (title, samples)) in tracks.iter().enumerate() {
            std::fs::write(d.join(format!("{i}-{title}.wav")), wav(title, artist, *samples)).unwrap();
        }
    }

    let (library, _) = Library::scan(&root, dir.path().join("cache.bin")).unwrap();
    let (playlists, _) = PlaylistStore::load(dir.path().join("playlists"), &library);
    let app = App::new(library, playlists, Backend::null());

    Harness { _dir: dir, app }
}

fn empty_harness() -> Harness {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path().join("music");
    std::fs::create_dir_all(&root).unwrap();

    let (library, _) = Library::scan(&root, dir.path().join("cache.bin")).unwrap();
    let (playlists, _) = PlaylistStore::load(dir.path().join("playlists"), &library);
    let app = App::new(library, playlists, Backend::null());

    Harness { _dir: dir, app }
}

fn key(c: char) -> KeyEvent {
    KeyEvent::from(KeyCode::Char(c))
}

fn special(code: KeyCode) -> KeyEvent {
    KeyEvent::from(code)
}

fn song_titles(app: &mut App) -> Vec<String> {
    app.visible_rows()
        .to_vec()
        .iter()
        .filter_map(|r| match r {
            Row::Song(id, _) => Some(app.library.get(*id).unwrap().title().to_string()),
            Row::Header(_) => None,
        })
        .collect()
}

fn header_names(app: &mut App) -> Vec<String> {
    app.visible_rows()
        .iter()
        .filter_map(|r| match r {
            Row::Header(name) => Some(name.trim().to_string()),
            Row::Song(_, _) => None,
        })
        .collect()
}

fn render(app: &mut App, width: u16, height: u16) -> Buffer {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    app.render(area, &mut buf);
    buf
}

fn buffer_text(buf: &Buffer) -> String {
    let area = *buf.area();
    (0..area.height)
        .map(|y| (0..area.width).map(|x| buf[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn default_category_is_none_and_lists_every_song_with_no_headers() {
    let mut h = harness();
    assert_eq!(header_names(&mut h.app).len(), 0);
    assert_eq!(song_titles(&mut h.app).len(), 6);
}

#[test]
fn default_sort_is_title_and_orders_songs_alphabetically() {
    let mut h = harness();
    let titles = song_titles(&mut h.app);
    let mut sorted = titles.clone();
    sorted.sort_by_key(|t| t.to_lowercase());
    assert_eq!(titles, sorted);
}

#[test]
fn category_artist_creates_one_header_per_artist() {
    let mut h = harness();
    h.app.library_panel.category = Category::Artist;

    let headers = header_names(&mut h.app);
    assert_eq!(headers, vec!["Alpha", "Beta", "Gamma"]);
}

#[test]
fn category_path_creates_one_header_per_directory() {
    let mut h = harness();
    h.app.library_panel.category = Category::Path;

    let headers = header_names(&mut h.app);
    assert_eq!(headers, vec!["Alpha", "Beta", "Gamma"]);
}

#[test]
fn category_and_sort_apply_independently() {
    let mut h = harness();
    h.app.library_panel.category = Category::Artist;
    h.app.library_panel.sort = Sort::Duration;

    let rows: Vec<Option<String>> = h
        .app
        .visible_rows()
        .to_vec()
        .iter()
        .map(|r| match r {
            Row::Header(name) => Some(format!("#{}", name.trim())),
            Row::Song(id, _) => Some(h.app.library.get(*id).unwrap().title().to_string()),
        })
        .collect();

    assert_eq!(
        rows,
        vec![
            Some("#Alpha".into()),
            Some("Azure".into()),
            Some("Anchor".into()),
            Some("#Beta".into()),
            Some("Bright".into()),
            Some("Beacon".into()),
            Some("#Gamma".into()),
            Some("Grove".into()),
            Some("Glimmer".into()),
        ],
        "grouping must stay by artist while the shorter (lower duration) song leads within each group"
    );
}

#[test]
fn switching_category_back_to_none_removes_all_headers() {
    let mut h = harness();
    h.app.library_panel.category = Category::Artist;
    assert!(!header_names(&mut h.app).is_empty());

    h.app.library_panel.category = Category::None;
    assert!(header_names(&mut h.app).is_empty());
}

#[test]
fn search_query_filters_rows_and_clearing_restores_them() {
    let mut h = harness();
    assert_eq!(song_titles(&mut h.app).len(), 6);

    h.app.library_panel.search_query = "azure".into();
    assert_eq!(song_titles(&mut h.app), vec!["Azure"]);

    h.app.library_panel.search_query.clear();
    assert_eq!(song_titles(&mut h.app).len(), 6);
}

#[test]
fn repeated_visible_rows_calls_are_stable_when_nothing_changed() {
    let mut h = harness();
    let first = song_titles(&mut h.app);
    let second = song_titles(&mut h.app);
    assert_eq!(first, second);
}

#[test]
fn switching_panels_changes_the_visible_rows() {
    let mut h = harness();
    assert_eq!(song_titles(&mut h.app).len(), 6);

    h.app.on_key(special(KeyCode::Tab));
    assert_eq!(h.app.panel, Panel::Playlists);
    assert!(h.app.visible_rows().is_empty(), "no playlist is open yet");

    h.app.on_key(special(KeyCode::Tab));
    assert_eq!(h.app.panel, Panel::Library);
    assert_eq!(song_titles(&mut h.app).len(), 6);
}

#[test]
fn viewing_a_playlist_shows_only_its_songs() {
    let mut h = harness();
    let song = h.app.library.ids_by_path()[0];
    let id = h.app.playlists.create("Mix");

    h.app.panel = Panel::Playlists;
    h.app.playlist_panel.view = PlaylistView::Viewing(id);
    assert_eq!(song_titles(&mut h.app).len(), 0);

    h.app.playlists.add_song(id, song);
    assert_eq!(song_titles(&mut h.app).len(), 1);

    h.app.playlists.remove_song(id, song);
    assert_eq!(song_titles(&mut h.app).len(), 0);
}

#[test]
fn moving_the_selection_wraps_and_never_lands_on_a_header() {
    let mut h = harness();
    h.app.library_panel.category = Category::Artist;
    h.app.on_key(key('g'));

    for _ in 0..12 {
        let row = h.app.selected_row();
        assert!(matches!(row, Some(Row::Song(_, _))), "a header must never be selected");
        h.app.on_key(key('j'));
    }
}

#[test]
fn shift_g_selects_the_last_song_and_g_selects_the_first() {
    let mut h = harness();
    h.app.on_key(special(KeyCode::Char('G')));
    let last = h.app.library_panel.list_state.selected();

    h.app.on_key(key('g'));
    let first = h.app.library_panel.list_state.selected();

    assert_eq!(first, Some(0));
    assert_eq!(last, Some(5));
}

#[test]
fn lowercase_o_cycles_category_and_shows_a_status_message() {
    let mut h = harness();
    assert!(header_names(&mut h.app).is_empty());

    h.app.on_key(key('o'));
    assert!(!header_names(&mut h.app).is_empty(), "category should have advanced past None");
    assert!(h.app.status.text.contains("grouped by"));
}

#[test]
fn lowercase_p_cycles_sort_and_shows_a_status_message() {
    let mut h = harness();
    let before = song_titles(&mut h.app);

    h.app.on_key(key('p'));
    let after = song_titles(&mut h.app);

    assert_ne!(before, after, "sort must have changed the order");
    assert!(h.app.status.text.contains("sorted by"));
}

#[test]
fn shift_a_opens_the_song_actions_modal_not_lowercase_p() {
    let mut h = harness();
    h.app.on_key(key('g'));

    h.app.on_key(key('p'));
    assert!(h.app.modal.song_modal.is_none(), "'p' must cycle sort, not open the song modal");

    h.app.on_key(special(KeyCode::Char('A')));
    assert!(h.app.modal.song_modal.is_some(), "Shift+A must open the song actions modal");
}

#[test]
fn shift_e_opens_the_metadata_modal_prefilled_with_the_selected_songs_tags() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let Some(Row::Song(id, _)) = h.app.selected_row() else { panic!("expected a song row") };
    let expected_title = h.app.library.get(id).unwrap().title().to_string();

    h.app.on_key(special(KeyCode::Char('E')));

    let modal = h.app.modal.metadata_modal.as_ref().expect("Shift+E must open the metadata edit modal");
    assert_eq!(modal.song, id);
    assert_eq!(modal.edits.title, expected_title);
    assert_eq!(modal.focused, MetadataField::Title);
    assert!(modal.error.is_none());
}

#[test]
fn metadata_modal_tab_and_shift_tab_cycle_through_every_field_and_wrap() {
    let mut h = harness();
    h.app.on_key(key('g'));
    h.app.on_key(special(KeyCode::Char('E')));

    for &expected in MetadataField::ALL {
        assert_eq!(h.app.modal.metadata_modal.as_ref().unwrap().focused, expected);
        h.app.on_key(special(KeyCode::Tab));
    }
    assert_eq!(
        h.app.modal.metadata_modal.as_ref().unwrap().focused,
        MetadataField::Title,
        "tabbing past the last field must wrap back to the first"
    );

    h.app.on_key(special(KeyCode::BackTab));
    assert_eq!(
        h.app.modal.metadata_modal.as_ref().unwrap().focused,
        *MetadataField::ALL.last().unwrap(),
        "shift-tab from the first field must wrap back to the last"
    );
}

#[test]
fn metadata_modal_esc_cancels_without_changing_the_library() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let Some(Row::Song(id, _)) = h.app.selected_row() else { panic!("expected a song row") };
    let before = h.app.library.get(id).unwrap().title().to_string();

    h.app.on_key(special(KeyCode::Char('E')));
    h.app.on_key(key('x'));
    h.app.on_key(special(KeyCode::Esc));

    assert!(h.app.modal.metadata_modal.is_none());
    assert_eq!(h.app.library.get(id).unwrap().title(), before);
}

#[test]
fn metadata_modal_editing_the_title_and_saving_updates_the_library() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let Some(Row::Song(old_id, _)) = h.app.selected_row() else { panic!("expected a song row") };

    h.app.on_key(special(KeyCode::Char('E')));
    for _ in 0..32 {
        h.app.on_key(special(KeyCode::Backspace));
    }
    for c in "Retitled".chars() {
        h.app.on_key(key(c));
    }
    h.app.on_key(special(KeyCode::Enter));

    assert!(h.app.modal.metadata_modal.is_none(), "a successful save must close the modal");
    assert!(!h.app.library.contains(old_id), "the old song id must no longer resolve");

    let new_song = h.app.library.songs_by_path().find(|s| s.title() == "Retitled");
    assert!(new_song.is_some(), "the library must contain a song with the new title");
    assert!(h.app.status.text.contains("updated metadata"));
}

#[test]
fn metadata_modal_saving_a_non_numeric_track_keeps_the_modal_open_with_an_error() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let Some(Row::Song(old_id, _)) = h.app.selected_row() else { panic!("expected a song row") };

    h.app.on_key(special(KeyCode::Char('E')));
    for _ in 0..MetadataField::ALL.iter().position(|&f| f == MetadataField::Track).unwrap() {
        h.app.on_key(special(KeyCode::Tab));
    }
    for c in "not-a-number".chars() {
        h.app.on_key(key(c));
    }
    h.app.on_key(special(KeyCode::Enter));

    let modal = h.app.modal.metadata_modal.as_ref().expect("a failed save must keep the modal open");
    assert!(modal.error.is_some());
    assert_eq!(modal.song, old_id, "the failed edit must be preserved so the user can fix it");
    assert!(h.app.library.contains(old_id), "the library must be untouched by a failed write");
}

#[test]
fn metadata_modal_save_carries_the_now_playing_song_to_its_new_id() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let Some(Row::Song(old_id, _)) = h.app.selected_row() else { panic!("expected a song row") };

    h.app.on_key(special(KeyCode::Enter));
    assert_eq!(h.app.queue.current_id(), Some(old_id), "sanity check: the song is now playing");

    h.app.on_key(special(KeyCode::Char('E')));
    for _ in 0..32 {
        h.app.on_key(special(KeyCode::Backspace));
    }
    for c in "Retitled".chars() {
        h.app.on_key(key(c));
    }
    h.app.on_key(special(KeyCode::Enter));

    let new_id = h.app.library.songs_by_path().find(|s| s.title() == "Retitled").unwrap().id();
    assert_eq!(
        h.app.queue.current_id(),
        Some(new_id),
        "the currently-playing song must follow the id change, not silently stop tracking it"
    );
}

#[test]
fn enter_plays_the_selected_song() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let selected = h.app.selected_row();
    let Some(Row::Song(id, _)) = selected else { panic!("expected a song row") };

    h.app.on_key(special(KeyCode::Enter));

    assert_eq!(h.app.queue.current_id(), Some(id));
}

#[test]
fn n_advances_and_b_goes_back() {
    let mut h = harness();
    h.app.on_key(key('n'));
    let first = h.app.queue.current_id();

    h.app.on_key(key('n'));
    let second = h.app.queue.current_id();
    assert_ne!(first, second);

    h.app.on_key(key('b'));
    assert_eq!(h.app.queue.current_id(), first);
}

#[test]
fn a_queues_the_selected_song_next() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let Some(Row::Song(id, _)) = h.app.selected_row() else { panic!("expected a song row") };

    h.app.on_key(key('a'));

    assert_eq!(h.app.queue.priority_queue().front(), Some(&id));
}

#[test]
fn b_with_no_previous_track_shows_a_status_message() {
    let mut h = empty_harness();
    h.app.on_key(key('b'));
    assert!(h.app.status.text.contains("no previous track"));
}

#[test]
fn enter_with_no_song_selected_shows_a_status_message() {
    let mut h = harness();
    h.app.library_panel.list_state.select(None);
    h.app.on_key(special(KeyCode::Enter));
    assert!(h.app.status.text.contains("select a song first"));
}

#[test]
fn lowercase_a_with_no_song_selected_shows_a_status_message() {
    let mut h = harness();
    h.app.library_panel.list_state.select(None);
    h.app.on_key(key('a'));
    assert!(h.app.status.text.contains("select a song first"));
}

#[test]
fn shift_a_with_no_song_selected_shows_a_status_message() {
    let mut h = harness();
    h.app.library_panel.list_state.select(None);
    h.app.on_key(key('A'));
    assert!(h.app.status.text.contains("select a song first"));
}

#[test]
fn entering_a_playlist_shows_a_status_message() {
    let mut h = harness();
    let id = h.app.playlists.create("Mix");

    h.app.panel = Panel::Playlists;
    h.app.playlist_panel.list_state.select(Some(0));
    h.app.on_key(special(KeyCode::Enter));

    assert_eq!(h.app.playlist_panel.view, PlaylistView::Viewing(id));
    assert!(h.app.status.text.contains("viewing \"Mix\""));
}

#[test]
fn q_asks_for_confirmation_before_quitting() {
    let mut h = harness();
    h.app.on_key(key('q'));
    assert!(h.app.modal.confirming_quit);

    h.app.on_key(key('x'));
    assert!(!h.app.modal.confirming_quit, "any key other than y/Y/Enter must cancel the quit confirmation");
}

#[test]
fn question_mark_toggles_the_help_overlay() {
    let mut h = harness();
    assert!(!h.app.modal.showing_help);

    h.app.on_key(key('?'));
    assert!(h.app.modal.showing_help);

    h.app.on_key(key('x'));
    assert!(!h.app.modal.showing_help, "any key closes the help overlay");
}

#[test]
fn a_rendered_frame_shows_the_selected_song_and_panel_title() {
    let mut h = harness();
    h.app.on_key(key('g'));

    let buf = render(&mut h.app, 120, 30);
    let text = buffer_text(&buf);
    assert!(text.contains("Anchor"), "the first song should be visible:\n{text}");
    assert!(text.contains("Library"));
}

#[test]
fn sort_title_width_is_stable_across_every_category_and_sort_combination() {
    let widths: Vec<usize> = Category::ALL
        .iter()
        .flat_map(|category| {
            Sort::ALL.iter().map(|sort| {
                sort_title(category.label(), sort.label(), Style::default()).width()
            })
        })
        .collect();

    let first = widths[0];
    for (i, width) in widths.iter().enumerate() {
        assert_eq!(
            *width, first,
            "combination #{i} has a different rendered width than the others -- the header would jump"
        );
    }
}

#[test]
fn scan_cache_path_lives_under_the_cache_dir_not_the_library_root() {
    let home = tempfile::tempdir().unwrap();
    let library_root = tempfile::tempdir().unwrap();

    let (home_path, root_path) = unsafe {
        let prev_home = std::env::var("HOME").ok();
        let prev_xdg = std::env::var("XDG_CACHE_HOME").ok();
        std::env::set_var("HOME", home.path());
        std::env::remove_var("XDG_CACHE_HOME");

        let cache_path = config::scan_cache_path(library_root.path());

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match prev_xdg {
            Some(v) => std::env::set_var("XDG_CACHE_HOME", v),
            None => std::env::remove_var("XDG_CACHE_HOME"),
        }

        (cache_path, library_root.path().to_path_buf())
    };

    assert!(
        home_path.starts_with(home.path().join(".cache").join("lyre")),
        "cache file should live under the XDG cache dir, got {home_path:?}"
    );
    assert!(
        !home_path.starts_with(&root_path),
        "cache file should not be written inside the library root, got {home_path:?}"
    );
}

#[test]
fn scan_cache_path_is_stable_for_the_same_root() {
    let library_root = tempfile::tempdir().unwrap();

    let (first, second) = unsafe {
        let prev_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", tempfile::tempdir().unwrap().path());

        let first = config::scan_cache_path(library_root.path());
        let second = config::scan_cache_path(library_root.path());

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        (first, second)
    };

    assert_eq!(first, second, "the same library root should always map to the same cache file");
}

#[test]
fn playlists_path_lives_under_the_data_dir_regardless_of_library_root() {
    let home = tempfile::tempdir().unwrap();

    let path = unsafe {
        let prev_home = std::env::var("HOME").ok();
        let prev_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("HOME", home.path());
        std::env::remove_var("XDG_DATA_HOME");

        let path = config::playlists_path();

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match prev_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }

        path
    };

    assert!(
        path.starts_with(home.path().join(".local").join("share").join("lyre")),
        "playlists file should live under the XDG data dir, got {path:?}"
    );
}

#[test]
fn playlists_path_does_not_depend_on_which_library_directory_is_open() {
    let home = tempfile::tempdir().unwrap();

    let (from_one_root, from_another_root) = unsafe {
        let prev_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", home.path());

        // playlists_path takes no library root at all -- calling it while "in" two
        // different library directories must resolve to the exact same file, so
        // switching directories can never silently point at a different playlists
        // file than the one the app started with.
        let from_one_root = config::playlists_path();
        let from_another_root = config::playlists_path();

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        (from_one_root, from_another_root)
    };

    assert_eq!(
        from_one_root, from_another_root,
        "there must be exactly one playlists file, independent of the current library root"
    );
}

#[test]
fn scrolling_to_the_end_brings_the_last_song_into_view() {
    let mut h = harness();
    let short = Rect::new(0, 0, 120, 12);

    let mut buf = Buffer::empty(short);
    h.app.render(short, &mut buf);
    assert!(buffer_text(&buf).contains("Anchor"));

    h.app.on_key(special(KeyCode::Char('G')));
    let mut buf = Buffer::empty(short);
    h.app.render(short, &mut buf);
    assert!(buffer_text(&buf).contains("Grove"), "the last song should scroll into view");
}
