use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lyre_core::{Library, MetadataEdits, PlaylistStore};
use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

use lyre_tui::{
    app::{App, Category, ChooseActionField, MetadataField, Panel, PlaylistView, Row, SidePanel, Sort},
    config,
    keymap::{self, Action},
    ui::{marquee_window, sort_title},
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

fn youtube_fields(url: &str, focused: lyre_tui::app::YoutubeField) -> lyre_tui::app::YoutubeFieldsModal {
    lyre_tui::app::YoutubeFieldsModal {
        url: url.to_string(),
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        title_sort: String::new(),
        artist_sort: String::new(),
        directory: String::new(),
        file_name: String::new(),
        file_name_overridden: false,
        focused,
        error: None,
        fetch_status: lyre_tui::app::FetchStatus::Pending,
        download_status: lyre_tui::app::DownloadStatus::Pending,
    }
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

    let visible = MetadataField::visible(&h.app.modal.metadata_modal.as_ref().unwrap().edits);
    for expected in &visible {
        assert_eq!(h.app.modal.metadata_modal.as_ref().unwrap().focused, *expected);
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
        *visible.last().unwrap(),
        "shift-tab from the first field must wrap back to the last"
    );
}

#[test]
fn song_modal_tab_and_shift_tab_move_focus_like_j_and_k() {
    let mut h = harness();
    h.app.playlists.create("First");
    h.app.on_key(key('g'));
    h.app.on_key(special(KeyCode::Char('A')));
    assert_eq!(h.app.modal.song_modal.as_ref().unwrap().selected, ChooseActionField::AddToPlaylist);

    h.app.on_key(special(KeyCode::Tab));
    assert_eq!(
        h.app.modal.song_modal.as_ref().unwrap().selected,
        ChooseActionField::CreatePlaylist,
        "Tab must move focus in the song actions modal, matching the metadata modal"
    );

    h.app.on_key(special(KeyCode::BackTab));
    assert_eq!(
        h.app.modal.song_modal.as_ref().unwrap().selected,
        ChooseActionField::AddToPlaylist,
        "Shift+Tab must move focus back, matching the metadata modal"
    );
}

#[test]
fn song_modal_side_panel_tab_and_shift_tab_move_selection_like_j_and_k() {
    let mut h = harness();
    h.app.playlists.create("First");
    h.app.playlists.create("Second");
    h.app.on_key(key('g'));
    h.app.on_key(special(KeyCode::Char('A')));
    h.app.on_key(special(KeyCode::Enter));

    let SidePanel::AddToPlaylist { list_state, .. } = h.app.modal.song_modal.as_ref().unwrap().side.as_ref().unwrap();
    let start = list_state.selected();

    h.app.on_key(special(KeyCode::Tab));
    let SidePanel::AddToPlaylist { list_state, .. } = h.app.modal.song_modal.as_ref().unwrap().side.as_ref().unwrap();
    let after_tab = list_state.selected();
    assert_ne!(after_tab, start, "Tab must move the side panel selection, matching j/Down");

    h.app.on_key(special(KeyCode::BackTab));
    let SidePanel::AddToPlaylist { list_state, .. } = h.app.modal.song_modal.as_ref().unwrap().side.as_ref().unwrap();
    assert_eq!(list_state.selected(), start, "Shift+Tab must move the side panel selection back, matching k/Up");
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
    let edits = h.app.modal.metadata_modal.as_ref().unwrap().edits.clone();
    for _ in 0..MetadataField::visible(&edits).iter().position(|&f| f == MetadataField::Track).unwrap() {
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
fn playlists_path_lives_under_the_data_dir() {
    let home = tempfile::tempdir().unwrap();
    let library_root = tempfile::tempdir().unwrap();

    let path = unsafe {
        let prev_home = std::env::var("HOME").ok();
        let prev_xdg = std::env::var("XDG_DATA_HOME").ok();
        std::env::set_var("HOME", home.path());
        std::env::remove_var("XDG_DATA_HOME");

        let path = config::playlists_path(library_root.path());

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
fn playlists_path_is_stable_for_the_same_root_and_differs_across_roots() {
    let one_root = tempfile::tempdir().unwrap();
    let another_root = tempfile::tempdir().unwrap();

    let (first, second, from_another_root) = unsafe {
        let prev_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", tempfile::tempdir().unwrap().path());

        let first = config::playlists_path(one_root.path());
        let second = config::playlists_path(one_root.path());
        let from_another_root = config::playlists_path(another_root.path());

        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }

        (first, second, from_another_root)
    };

    assert_eq!(first, second, "the same library root should always map to the same playlists file");
    assert_ne!(
        first, from_another_root,
        "each library root must get its own playlists file, so switching directories can't silently \
         prune or overwrite another library's playlists"
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

#[test]
fn ctrl_d_pages_down_instead_of_changing_directory() {
    assert_eq!(
        keymap::lookup(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        Some(Action::PageDown),
        "Ctrl+d must resolve to PageDown, not be shadowed by plain d's ChangeDirectory binding"
    );
    assert_eq!(
        keymap::lookup(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)),
        Some(Action::ChangeDirectory)
    );
}

#[test]
fn ctrl_u_pages_up_instead_of_unshuffling() {
    assert_eq!(
        keymap::lookup(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL)),
        Some(Action::PageUp),
        "Ctrl+u must resolve to PageUp, not be shadowed by plain u's Unshuffle binding"
    );
    assert_eq!(
        keymap::lookup(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE)),
        Some(Action::Unshuffle)
    );
}

#[test]
fn marquee_window_returns_short_ascii_text_unchanged() {
    assert_eq!(marquee_window("Beacon", 20), "Beacon");
}

#[test]
fn marquee_window_never_exceeds_the_visible_width_for_wide_characters() {
    for text in ["初音ミク", "こんにちは世界", "안녕하세요", "Beacon 灯台 Artist"] {
        for width in 1..12 {
            let windowed = marquee_window(text, width);
            assert!(
                lyre_tui::ui::display_width(&windowed) <= width,
                "marquee_window({text:?}, {width}) returned {windowed:?} with display width \
                 {}, exceeding the {width}-column budget",
                lyre_tui::ui::display_width(&windowed)
            );
        }
    }
}

#[test]
fn marquee_window_zero_width_returns_empty() {
    assert_eq!(marquee_window("Beacon", 0), "");
}

#[test]
fn lowercase_y_opens_the_youtube_modal_entering_url() {
    let mut h = harness();
    h.app.on_key(key('y'));

    let modal = h.app.modal.youtube_modal.as_ref().expect("<y> must open the youtube modal");
    assert!(matches!(modal, lyre_tui::app::YoutubeModal::EnteringUrl { url_input, error, restore } if url_input.is_empty() && error.is_none() && restore.is_none()));
}

#[cfg(not(feature = "youtube"))]
#[test]
fn without_the_youtube_feature_submitting_a_url_fails_gracefully_instead_of_hanging() {
    let mut h = harness();
    h.app.on_key(key('y'));
    for c in "https://example.com/watch?v=x".chars() {
        h.app.on_key(key(c));
    }
    h.app.on_key(special(KeyCode::Enter));

    assert_eq!(
        h.app.drain_youtube_events_for_test(),
        lyre_tui::app::EventsChanged::Changed,
        "the stub must report an event, not silently do nothing"
    );

    match h.app.modal.youtube_modal.as_ref().expect("a failure must not silently close the modal") {
        lyre_tui::app::YoutubeModal::EnteringUrl { url_input, error, .. } => {
            assert_eq!(url_input, "https://example.com/watch?v=x");
            assert_eq!(error.as_deref(), Some("YouTube support was not built into this binary"));
        }
        _ => panic!("a failure must bounce the user back to the URL screen"),
    }
}

#[test]
fn youtube_modal_entering_url_accumulates_typed_characters_and_esc_cancels() {
    let mut h = harness();
    h.app.on_key(key('y'));
    h.app.on_key(key('h'));
    h.app.on_key(key('i'));

    match h.app.modal.youtube_modal.as_ref().unwrap() {
        lyre_tui::app::YoutubeModal::EnteringUrl { url_input, .. } => assert_eq!(url_input, "hi"),
        _ => panic!("expected EnteringUrl"),
    }

    h.app.on_key(special(KeyCode::Esc));
    assert!(h.app.modal.youtube_modal.is_none());
}

#[test]
fn youtube_modal_rejects_a_directory_that_escapes_the_library_root() {
    let mut h = harness();
    h.app.modal.youtube_modal = Some(lyre_tui::app::YoutubeModal::EditingFields(lyre_tui::app::YoutubeFieldsModal {
        title: "Some Title".to_string(),
        artist: "Some Artist".to_string(),
        directory: "../escape".to_string(),
        file_name: "SomeArtist-SomeTitle.mp3".to_string(),
        file_name_overridden: true,
        focused: lyre_tui::app::YoutubeField::Directory,
        ..youtube_fields("https://example.com/watch?v=x", lyre_tui::app::YoutubeField::Directory)
    }));

    h.app.on_key(special(KeyCode::Enter));

    match h.app.modal.youtube_modal.as_ref().expect("modal should stay open on validation error") {
        lyre_tui::app::YoutubeModal::EditingFields(fields) => {
            assert!(fields.error.as_deref().unwrap_or_default().contains(".."));
        }
        _ => panic!("expected to stay on EditingFields"),
    }
}

#[test]
fn youtube_modal_auto_generates_the_file_name_from_title_and_artist_until_overridden() {
    let mut h = harness();
    h.app.modal.youtube_modal = Some(lyre_tui::app::YoutubeModal::EditingFields(youtube_fields(
        "https://example.com/watch?v=x",
        lyre_tui::app::YoutubeField::Title,
    )));

    for c in "Bush".chars() {
        h.app.on_key(key(c));
    }
    h.app.on_key(special(KeyCode::Tab));
    for c in "Kate".chars() {
        h.app.on_key(key(c));
    }

    match h.app.modal.youtube_modal.as_ref().unwrap() {
        lyre_tui::app::YoutubeModal::EditingFields(fields) => {
            assert_eq!(fields.file_name, "Kate-Bush.mp3");
            assert!(!fields.file_name_overridden);
        }
        _ => panic!("expected EditingFields"),
    }
}

#[test]
fn metadata_field_visible_hides_the_romanized_fields_by_default() {
    let edits = MetadataEdits::default();
    let visible = MetadataField::visible(&edits);

    assert!(!visible.contains(&MetadataField::TitleSort));
    assert!(!visible.contains(&MetadataField::ArtistSort));
}

#[test]
fn metadata_field_visible_shows_title_sort_only_when_the_title_needs_romanization() {
    let edits = MetadataEdits { title: "夜明け".to_string(), ..MetadataEdits::default() };
    let visible = MetadataField::visible(&edits);

    assert!(visible.contains(&MetadataField::TitleSort));
    assert!(!visible.contains(&MetadataField::ArtistSort));
}

#[test]
fn metadata_modal_typing_a_non_ascii_title_reveals_the_title_sort_field_in_the_tab_order() {
    let mut h = harness();
    h.app.on_key(key('g'));
    h.app.on_key(special(KeyCode::Char('E')));

    for c in "夜明け".chars() {
        h.app.on_key(key(c));
    }

    let modal = h.app.modal.metadata_modal.as_ref().unwrap();
    let visible = MetadataField::visible(&modal.edits);
    assert!(visible.contains(&MetadataField::TitleSort), "a non-ASCII title must reveal the romanized field");

    h.app.on_key(special(KeyCode::Tab));
    assert_eq!(h.app.modal.metadata_modal.as_ref().unwrap().focused, MetadataField::TitleSort);
}

fn open_metadata_modal_for_selected_song(h: &mut Harness) {
    h.app.on_key(special(KeyCode::Char('E')));
}

#[test]
fn saving_a_new_romanized_artist_prompts_to_apply_it_to_sibling_songs() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let Some(Row::Song(id, _)) = h.app.selected_row() else { panic!("expected a song row") };
    assert_eq!(h.app.library.get(id).unwrap().artist(), "Alpha", "sanity check: selected an Alpha song");

    open_metadata_modal_for_selected_song(&mut h);
    h.app.modal.metadata_modal.as_mut().unwrap().edits.artist_sort = "Arufa".to_string();
    h.app.on_key(special(KeyCode::Enter));

    let confirm = h.app.modal.romanized_artist_confirm.as_ref().expect("must prompt when a sibling song shares the artist");
    assert_eq!(confirm.artist_display, "Alpha");
    assert_eq!(confirm.value, "Arufa");
    assert_eq!(confirm.count, 1, "only one other Alpha song exists in the fixture");
}

#[test]
fn saving_a_romanized_artist_with_no_siblings_does_not_prompt() {
    let mut h = harness();
    h.app.on_key(key('g'));

    open_metadata_modal_for_selected_song(&mut h);
    {
        let modal = h.app.modal.metadata_modal.as_mut().unwrap();
        modal.edits.artist = "CompletelyUniqueArtist".to_string();
        modal.edits.artist_sort = "Yunikuu".to_string();
    }
    h.app.on_key(special(KeyCode::Enter));

    assert!(
        h.app.modal.romanized_artist_confirm.is_none(),
        "an artist with no other songs must not trigger the confirmation"
    );
}

#[test]
fn declining_the_romanized_artist_prompt_leaves_the_library_untouched() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let songs_before = h.app.library.len();

    open_metadata_modal_for_selected_song(&mut h);
    h.app.modal.metadata_modal.as_mut().unwrap().edits.artist_sort = "Arufa".to_string();
    h.app.on_key(special(KeyCode::Enter));
    assert!(h.app.modal.romanized_artist_confirm.is_some());

    h.app.on_key(key('n'));

    assert!(h.app.modal.romanized_artist_confirm.is_none());
    assert_eq!(h.app.library.len(), songs_before, "declining must not change how many songs exist");
}

#[test]
fn accepting_the_romanized_artist_prompt_applies_it_and_closes_the_modal() {
    let mut h = harness();
    h.app.on_key(key('g'));
    let songs_before = h.app.library.len();

    open_metadata_modal_for_selected_song(&mut h);
    h.app.modal.metadata_modal.as_mut().unwrap().edits.artist_sort = "Arufa".to_string();
    h.app.on_key(special(KeyCode::Enter));
    assert!(h.app.modal.romanized_artist_confirm.is_some());

    h.app.on_key(key('y'));

    assert!(h.app.modal.romanized_artist_confirm.is_none());
    assert_eq!(h.app.library.len(), songs_before, "applying must not add or remove songs, only re-tag them");
}

#[test]
fn youtube_field_visible_hides_the_romanized_fields_by_default() {
    let fields = youtube_fields("https://example.com/watch?v=x", lyre_tui::app::YoutubeField::Title);
    let visible = lyre_tui::app::YoutubeField::visible(&fields);

    assert!(!visible.contains(&lyre_tui::app::YoutubeField::TitleSort));
    assert!(!visible.contains(&lyre_tui::app::YoutubeField::ArtistSort));
}

#[test]
fn youtube_modal_typing_a_non_ascii_artist_reveals_the_artist_sort_field_in_the_tab_order() {
    let mut h = harness();
    h.app.modal.youtube_modal = Some(lyre_tui::app::YoutubeModal::EditingFields(youtube_fields(
        "https://example.com/watch?v=x",
        lyre_tui::app::YoutubeField::Artist,
    )));

    for c in "夜明けバンド".chars() {
        h.app.on_key(key(c));
    }

    match h.app.modal.youtube_modal.as_ref().unwrap() {
        lyre_tui::app::YoutubeModal::EditingFields(fields) => {
            let visible = lyre_tui::app::YoutubeField::visible(fields);
            assert!(
                visible.contains(&lyre_tui::app::YoutubeField::ArtistSort),
                "a non-ASCII artist must reveal the romanized field"
            );
        }
        _ => panic!("expected EditingFields"),
    }

    h.app.on_key(special(KeyCode::Tab));
    match h.app.modal.youtube_modal.as_ref().unwrap() {
        lyre_tui::app::YoutubeModal::EditingFields(fields) => {
            assert_eq!(fields.focused, lyre_tui::app::YoutubeField::ArtistSort);
        }
        _ => panic!("expected EditingFields"),
    }
}

#[test]
fn start_youtube_fields_with_no_restore_defaults_the_directory_to_the_library_root() {
    let fields = lyre_tui::app::start_youtube_fields("https://example.com/watch?v=x".to_string(), None);

    assert_eq!(fields.directory, "./");
    assert!(matches!(fields.fetch_status, lyre_tui::app::FetchStatus::Pending));
    assert!(matches!(fields.download_status, lyre_tui::app::DownloadStatus::Pending));
    assert_eq!(fields.focused, lyre_tui::app::YoutubeField::Title);
}

#[test]
fn start_youtube_fields_with_a_restore_snapshot_keeps_everything_but_the_url_and_resets_status() {
    let mut previous = youtube_fields("https://old-url.example.com", lyre_tui::app::YoutubeField::Album);
    previous.title = "Yoake".to_string();
    previous.artist = "Some Band".to_string();
    previous.directory = "custom/subdir".to_string();
    previous.error = Some("a stale error".to_string());

    let fields = lyre_tui::app::start_youtube_fields("https://new-url.example.com".to_string(), Some(previous));

    assert_eq!(fields.url, "https://new-url.example.com");
    assert_eq!(fields.title, "Yoake");
    assert_eq!(fields.artist, "Some Band");
    assert_eq!(fields.directory, "custom/subdir", "a retried attempt must keep the user's edits");
    assert!(fields.error.is_none(), "the error from the previous attempt must be cleared");
    assert_eq!(fields.focused, lyre_tui::app::YoutubeField::Title, "focus always resets to Title on retry");
    assert!(matches!(fields.fetch_status, lyre_tui::app::FetchStatus::Pending));
    assert!(matches!(fields.download_status, lyre_tui::app::DownloadStatus::Pending));
}

#[test]
fn a_download_failure_interrupts_the_user_and_preserves_their_fields_for_retry() {
    let mut h = harness();
    let mut fields = youtube_fields("https://example.com/watch?v=x", lyre_tui::app::YoutubeField::Album);
    fields.title = "Yoake".to_string();
    fields.artist = "Some Band".to_string();
    fields.directory = "custom/subdir".to_string();
    h.app.modal.youtube_modal = Some(lyre_tui::app::YoutubeModal::EditingFields(fields));

    h.app.handle_youtube_event_for_test(lyre_tui::app::DownloadEvent::Failed("network error".to_string()));

    match h.app.modal.youtube_modal.as_ref().expect("a failure must not silently close the modal") {
        lyre_tui::app::YoutubeModal::EnteringUrl { url_input, error, restore } => {
            assert_eq!(url_input, "https://example.com/watch?v=x");
            assert_eq!(error.as_deref(), Some("network error"));
            let restore = restore.as_ref().expect("the typed fields must be preserved for a retry");
            assert_eq!(restore.title, "Yoake");
            assert_eq!(restore.artist, "Some Band");
            assert_eq!(restore.directory, "custom/subdir");
        }
        _ => panic!("a failure must bounce the user back to the URL screen"),
    }
}

#[test]
fn a_fetch_failure_while_still_downloading_also_interrupts_and_preserves_fields() {
    let mut h = harness();
    let fields = youtube_fields("https://example.com/watch?v=x", lyre_tui::app::YoutubeField::Title);
    h.app.modal.youtube_modal = Some(lyre_tui::app::YoutubeModal::Downloading {
        file_name: "song.mp3".to_string(),
        dest_path: h.app.library.root().join("song.mp3"),
        fields,
    });

    h.app.handle_youtube_event_for_test(lyre_tui::app::DownloadEvent::Failed("this video is a live stream".to_string()));

    match h.app.modal.youtube_modal.as_ref().unwrap() {
        lyre_tui::app::YoutubeModal::EnteringUrl { error, restore, .. } => {
            assert_eq!(error.as_deref(), Some("this video is a live stream"));
            assert!(restore.is_some());
        }
        _ => panic!("a failure while waiting on the download must also interrupt"),
    }
}

#[test]
fn closing_the_modal_after_a_failure_discards_the_saved_fields() {
    let mut h = harness();
    h.app.modal.youtube_modal = Some(lyre_tui::app::YoutubeModal::EditingFields(youtube_fields(
        "https://example.com/watch?v=x",
        lyre_tui::app::YoutubeField::Title,
    )));
    h.app.handle_youtube_event_for_test(lyre_tui::app::DownloadEvent::Failed("network error".to_string()));
    assert!(h.app.modal.youtube_modal.is_some());

    h.app.on_key(special(KeyCode::Esc));

    assert!(h.app.modal.youtube_modal.is_none(), "exiting the whole modal must not keep anything around");
}

#[test]
fn a_download_finishing_while_still_editing_fields_is_remembered_without_leaving_the_screen() {
    let mut h = harness();
    h.app.modal.youtube_modal = Some(lyre_tui::app::YoutubeModal::EditingFields(youtube_fields(
        "https://example.com/watch?v=x",
        lyre_tui::app::YoutubeField::Title,
    )));

    let temp_path = std::env::temp_dir().join("lyre-test-download.mp3");
    h.app.handle_youtube_event_for_test(lyre_tui::app::DownloadEvent::DownloadReady(temp_path.clone()));

    match h.app.modal.youtube_modal.as_ref().unwrap() {
        lyre_tui::app::YoutubeModal::EditingFields(fields) => {
            assert!(
                matches!(&fields.download_status, lyre_tui::app::DownloadStatus::Ready(p) if p == &temp_path),
                "the finished download must be recorded without forcing the user off the fields screen"
            );
        }
        _ => panic!("the user must stay on EditingFields while still typing"),
    }
}

#[test]
fn info_ready_while_editing_fields_updates_the_inline_status_without_touching_typed_fields() {
    let mut h = harness();
    let mut fields = youtube_fields("https://example.com/watch?v=x", lyre_tui::app::YoutubeField::Title);
    fields.title = "user typed this".to_string();
    h.app.modal.youtube_modal = Some(lyre_tui::app::YoutubeModal::EditingFields(fields));

    h.app.handle_youtube_event_for_test(lyre_tui::app::DownloadEvent::InfoReady {
        title: "Fetched Video Title".to_string(),
        uploader: Some("Some Uploader".to_string()),
    });

    match h.app.modal.youtube_modal.as_ref().unwrap() {
        lyre_tui::app::YoutubeModal::EditingFields(fields) => {
            assert_eq!(fields.title, "user typed this", "fetched info must never overwrite what the user typed");
            match &fields.fetch_status {
                lyre_tui::app::FetchStatus::Ready { title, uploader } => {
                    assert_eq!(title, "Fetched Video Title");
                    assert_eq!(uploader.as_deref(), Some("Some Uploader"));
                }
                lyre_tui::app::FetchStatus::Pending => panic!("fetch status must update to Ready"),
            }
        }
        _ => panic!("expected EditingFields"),
    }
}
