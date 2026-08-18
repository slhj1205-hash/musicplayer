mod fixtures;

use std::{path::Path, time::Duration};

use fixtures::write_song;
use lyre_core::{
    library::Library,
    player::{AudioBackend, PlaybackState},
    playlist::PlaylistStore,
    queue::Queue,
    scan_cache::{Entry, Probed, ScanCache},
    song::{is_supported_audio, SongId},
    NullBackend, Player,
};
use tempfile::TempDir;

#[test]
fn is_supported_audio_accepts_known_extensions_case_insensitively() {
    for name in ["track.mp3", "track.MP3", "track.Flac", "track.OGG", "track.wav", "track.opus"] {
        assert!(is_supported_audio(Path::new(name)), "{name} should be recognised as audio");
    }
}

#[test]
fn is_supported_audio_rejects_non_audio_and_malformed_names() {
    for name in ["cover.jpg", "readme.txt", "playlist.m3u", "no_extension", "track.mp3.bak", "."] {
        assert!(!is_supported_audio(Path::new(name)), "{name} should not be recognised as audio");
    }
}

#[test]
fn library_scan_finds_only_supported_audio_files() {
    let dir = TempDir::new().unwrap();
    write_song(dir.path(), "one.wav", "One", "Artist", "Album");
    write_song(dir.path(), "two.wav", "Two", "Artist", "Album");
    std::fs::write(dir.path().join("cover.jpg"), b"not audio").unwrap();
    std::fs::write(dir.path().join("notes.txt"), b"not audio").unwrap();

    let (library, stats) = Library::scan(dir.path(), dir.path().join("cache.bin")).unwrap();

    assert_eq!(library.len(), 2);
    assert_eq!(stats.files_considered, 2);
}

#[test]
fn library_scan_recurses_into_subdirectories() {
    let dir = TempDir::new().unwrap();
    write_song(&dir.path().join("Artist A"), "one.wav", "One", "Artist A", "Album");
    write_song(&dir.path().join("Artist B").join("Album B"), "two.wav", "Two", "Artist B", "Album B");

    let (library, _) = Library::scan(dir.path(), dir.path().join("cache.bin")).unwrap();

    assert_eq!(library.len(), 2);
}

#[test]
fn library_scan_on_a_missing_path_is_an_error() {
    let dir = TempDir::new().unwrap();
    let missing = dir.path().join("does-not-exist");

    let result = Library::scan(&missing, dir.path().join("cache.bin"));

    assert!(result.is_err());
}

#[test]
fn library_scan_on_a_file_rather_than_a_directory_is_an_error() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("not-a-dir.txt");
    std::fs::write(&file, b"hello").unwrap();

    let result = Library::scan(&file, dir.path().join("cache.bin"));

    assert!(result.is_err());
}

#[test]
fn library_get_and_contains_agree_with_each_other() {
    let dir = TempDir::new().unwrap();
    write_song(dir.path(), "one.wav", "One", "Artist", "Album");
    let (library, _) = Library::scan(dir.path(), dir.path().join("cache.bin")).unwrap();

    let id = library.ids_by_path()[0];
    assert!(library.contains(id));
    assert!(library.get(id).is_some());

    let bogus = SongId::compute(Path::new("/nowhere"), 0, 0);
    assert!(!library.contains(bogus));
    assert!(library.get(bogus).is_none());
}

#[test]
fn library_ids_by_path_are_sorted_by_file_path() {
    let dir = TempDir::new().unwrap();
    write_song(dir.path(), "z.wav", "Z Song", "Artist", "Album");
    write_song(dir.path(), "a.wav", "A Song", "Artist", "Album");
    write_song(dir.path(), "m.wav", "M Song", "Artist", "Album");
    let (library, _) = Library::scan(dir.path(), dir.path().join("cache.bin")).unwrap();

    let titles: Vec<&str> = library.songs_by_path().map(|s| s.title()).collect();
    assert_eq!(titles, vec!["A Song", "M Song", "Z Song"]);
}

#[test]
fn library_scan_is_served_from_cache_on_the_second_pass() {
    let dir = TempDir::new().unwrap();
    write_song(dir.path(), "one.wav", "One", "Artist", "Album");
    let cache_path = dir.path().join("cache.bin");

    let (_, first) = Library::scan(dir.path(), &cache_path).unwrap();
    assert_eq!(first.reprobed, 1);
    assert_eq!(first.cache_hits, 0);

    let (_, second) = Library::scan(dir.path(), &cache_path).unwrap();
    assert_eq!(second.reprobed, 0);
    assert_eq!(second.cache_hits, 1);
}

#[test]
fn library_reprobes_a_file_after_it_changes() {
    let dir = TempDir::new().unwrap();
    let path = write_song(dir.path(), "one.wav", "One", "Artist", "Album");
    let cache_path = dir.path().join("cache.bin");
    Library::scan(dir.path(), &cache_path).unwrap();

    std::thread::sleep(Duration::from_millis(1100));
    std::fs::write(&path, fixtures::wav("One (Remaster)", "Artist", "Album", 400)).unwrap();

    let (library, stats) = Library::scan(dir.path(), &cache_path).unwrap();
    assert_eq!(stats.reprobed, 1);
    assert_eq!(library.songs_by_path().next().unwrap().title(), "One (Remaster)");
}

#[test]
fn library_drops_songs_that_are_deleted_between_scans() {
    let dir = TempDir::new().unwrap();
    let path = write_song(dir.path(), "one.wav", "One", "Artist", "Album");
    let cache_path = dir.path().join("cache.bin");
    let (library, _) = Library::scan(dir.path(), &cache_path).unwrap();
    assert_eq!(library.len(), 1);

    std::fs::remove_file(&path).unwrap();
    let (library, _) = Library::scan(dir.path(), &cache_path).unwrap();
    assert_eq!(library.len(), 0);
}

#[test]
fn scan_cache_round_trips_through_disk() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("cache.bin");

    let mut cache = ScanCache::new();
    cache.insert(
        Path::new("song.mp3").to_path_buf(),
        Entry { size: 123, mtime: 456, probed: Probed::Unreadable },
    );
    cache.save(&path);

    let loaded = ScanCache::load(&path);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.get_fresh(Path::new("song.mp3"), 123, 456), Some(&Probed::Unreadable));
}

#[test]
fn scan_cache_get_fresh_rejects_a_stale_fingerprint() {
    let mut cache = ScanCache::new();
    cache.insert(Path::new("song.mp3").to_path_buf(), Entry { size: 100, mtime: 200, probed: Probed::Unreadable });

    assert!(cache.get_fresh(Path::new("song.mp3"), 999, 200).is_none());
    assert!(cache.get_fresh(Path::new("song.mp3"), 100, 999).is_none());
    assert!(cache.get_fresh(Path::new("song.mp3"), 100, 200).is_some());
}

#[test]
fn scan_cache_treats_a_missing_or_corrupt_file_as_empty() {
    let dir = TempDir::new().unwrap();

    let missing = ScanCache::load(&dir.path().join("does-not-exist.bin"));
    assert!(missing.is_empty());

    let corrupt_path = dir.path().join("corrupt.bin");
    std::fs::write(&corrupt_path, b"not a valid cache file").unwrap();
    let corrupt = ScanCache::load(&corrupt_path);
    assert!(corrupt.is_empty());
}

#[test]
fn playlist_store_create_rename_and_delete_round_trip() {
    let dir = TempDir::new().unwrap();
    let mut store = PlaylistStore::empty(dir.path().join("playlists"));

    let id = store.create("Road Trip");
    assert_eq!(store.get(id).unwrap().name(), "Road Trip");

    assert!(store.rename(id, "Summer Trip"));
    assert_eq!(store.get(id).unwrap().name(), "Summer Trip");

    assert!(store.delete(id));
    assert!(store.get(id).is_none());
}

#[test]
fn playlist_store_persists_across_a_reload() {
    let dir = TempDir::new().unwrap();
    let playlists_dir = dir.path().join("playlists");
    write_song(dir.path(), "song.wav", "Song", "Artist", "Album");
    let (library, _) = Library::scan(dir.path(), dir.path().join("cache.bin")).unwrap();
    let song = library.ids_by_path()[0];

    let id = {
        let mut store = PlaylistStore::empty(&playlists_dir);
        let id = store.create("Favourites");
        store.add_song(id, song);
        id
    };

    let (reloaded, _) = PlaylistStore::load(&playlists_dir, &library);
    let playlist = reloaded.get(id).expect("playlist should have been persisted to disk");
    assert_eq!(playlist.name(), "Favourites");
    assert_eq!(playlist.songs(), &[song]);
}

#[test]
fn playlist_store_prunes_songs_that_are_no_longer_in_the_library() {
    let dir = TempDir::new().unwrap();
    let playlists_dir = dir.path().join("playlists");
    let missing_song = SongId::compute(Path::new("missing.mp3"), 1, 1);

    {
        let mut store = PlaylistStore::empty(&playlists_dir);
        let id = store.create("Mix");
        store.add_song(id, missing_song);
    }

    let empty_library = Library::empty(dir.path());
    let (store, stats) = PlaylistStore::load(&playlists_dir, &empty_library);

    assert_eq!(stats.songs_removed, 1);
    let id = store.ids_sorted_by_name()[0];
    assert!(store.get(id).unwrap().is_empty());
}

#[test]
fn playlist_store_add_song_is_idempotent() {
    let dir = TempDir::new().unwrap();
    let mut store = PlaylistStore::empty(dir.path().join("playlists"));
    let id = store.create("Mix");
    let song = SongId::compute(Path::new("song.mp3"), 1, 1);

    assert!(store.add_song(id, song));
    assert!(!store.add_song(id, song), "adding the same song twice should be a no-op");
    assert_eq!(store.get(id).unwrap().len(), 1);
}

#[test]
fn playlist_store_remove_song_updates_membership() {
    let dir = TempDir::new().unwrap();
    let mut store = PlaylistStore::empty(dir.path().join("playlists"));
    let id = store.create("Mix");
    let song = SongId::compute(Path::new("song.mp3"), 1, 1);
    store.add_song(id, song);

    assert!(store.contains(id, song));
    assert!(store.remove_song(id, song));
    assert!(!store.contains(id, song));
    assert!(store.containing(song).is_empty());
}

#[test]
fn playlist_store_containing_lists_every_playlist_holding_a_song() {
    let dir = TempDir::new().unwrap();
    let mut store = PlaylistStore::empty(dir.path().join("playlists"));
    let song = SongId::compute(Path::new("song.mp3"), 1, 1);

    let a = store.create("A");
    let b = store.create("B");
    store.add_song(a, song);
    store.add_song(b, song);

    let membership = store.containing(song);
    assert_eq!(membership.len(), 2);
    assert!(membership.contains(&a));
    assert!(membership.contains(&b));
}

#[test]
fn playlist_store_ids_sorted_by_name_are_case_insensitive() {
    let dir = TempDir::new().unwrap();
    let mut store = PlaylistStore::empty(dir.path().join("playlists"));
    store.create("banana");
    store.create("Apple");
    store.create("cherry");

    let names: Vec<&str> =
        store.ids_sorted_by_name().iter().map(|&id| store.get(id).unwrap().name()).collect();
    assert_eq!(names, vec!["Apple", "banana", "cherry"]);
}

#[test]
fn playlist_store_revision_advances_on_mutation_but_not_on_reads() {
    let dir = TempDir::new().unwrap();
    let mut store = PlaylistStore::empty(dir.path().join("playlists"));
    let before = store.revision();

    let id = store.create("Mix");
    assert!(store.revision() > before);

    let after_create = store.revision();
    let _ = store.get(id);
    let _ = store.ids_sorted_by_name();
    assert_eq!(store.revision(), after_create, "read-only calls must not bump the revision");
}

#[test]
fn queue_next_walks_forward_and_wraps_to_the_start() {
    let ids: Vec<SongId> = (0..3).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let mut queue = Queue::new(ids.clone());

    assert_eq!(queue.next(), Some(ids[0]));
    assert_eq!(queue.next(), Some(ids[1]));
    assert_eq!(queue.next(), Some(ids[2]));
    assert_eq!(queue.next(), Some(ids[0]), "past the end must wrap to the start");
}

#[test]
fn queue_previous_walks_backward_and_wraps_to_the_end() {
    let ids: Vec<SongId> = (0..3).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let mut queue = Queue::new(ids.clone());

    assert_eq!(queue.previous(), Some(ids[2]), "previous with nothing playing must land on the last song");
    assert_eq!(queue.previous(), Some(ids[1]));
}

#[test]
fn queue_on_an_empty_queue_returns_nothing() {
    let mut queue = Queue::new(Vec::new());
    assert_eq!(queue.next(), None);
    assert_eq!(queue.previous(), None);
    assert_eq!(queue.current_id(), None);
}

#[test]
fn queue_play_id_jumps_to_a_specific_song() {
    let ids: Vec<SongId> = (0..3).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let mut queue = Queue::new(ids.clone());

    assert_eq!(queue.play_id(ids[2]), Some(ids[2]));
    assert_eq!(queue.current_id(), Some(ids[2]));
}

#[test]
fn queue_play_id_prefers_the_nearest_occurrence_after_the_cursor() {
    let a = SongId::compute(Path::new("a.mp3"), 1, 1);
    let b = SongId::compute(Path::new("b.mp3"), 2, 2);

    let mut queue = Queue::new(vec![a, b, a, b]);
    queue.play_at(1);

    queue.play_id(a);
    assert_eq!(queue.current_position(), Some(2), "should land on the closer occurrence of `a` at index 2");
}

#[test]
fn queue_play_upcoming_jumps_forward_by_n() {
    let ids: Vec<SongId> = (0..5).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let mut queue = Queue::new(ids.clone());

    assert_eq!(queue.play_upcoming(3), Some(ids[2]));
}

#[test]
fn queue_play_upcoming_zero_does_nothing() {
    let ids: Vec<SongId> = (0..3).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let mut queue = Queue::new(ids);

    assert_eq!(queue.play_upcoming(0), None);
    assert_eq!(queue.current_id(), None);
}

#[test]
fn queue_priority_songs_play_before_the_regular_queue() {
    let ids: Vec<SongId> = (0..3).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let priority = SongId::compute(Path::new("priority.mp3"), 9, 9);

    let mut queue = Queue::new(ids.clone());
    queue.queue_next(priority);

    assert_eq!(queue.next(), Some(priority));
    assert_eq!(queue.next(), Some(ids[0]), "after the priority song, playback resumes at the queue's start");
}

#[test]
fn queue_upcoming_lists_priority_songs_then_the_regular_queue() {
    let ids: Vec<SongId> = (0..3).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let priority = SongId::compute(Path::new("priority.mp3"), 9, 9);

    let mut queue = Queue::new(ids.clone());
    queue.play_at(0);
    queue.queue_next(priority);

    assert_eq!(queue.upcoming(3), vec![priority, ids[1], ids[2]]);
}

#[test]
fn queue_shuffle_preserves_the_currently_playing_song() {
    let ids: Vec<SongId> = (0..20).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let mut queue = Queue::new(ids.clone());
    queue.play_at(5);
    let current = queue.current_id();

    queue.shuffle();

    assert_eq!(queue.current_id(), current, "shuffling must not change which song is playing");
}

#[test]
fn queue_unshuffle_restores_original_order() {
    let ids: Vec<SongId> = (0..5).map(|i| SongId::compute(Path::new(&format!("{i}.mp3")), i, i)).collect();
    let mut queue = Queue::new(ids.clone());

    queue.shuffle();
    queue.unshuffle();

    assert_eq!(queue.ordered_ids().collect::<Vec<_>>(), ids);
}

#[test]
fn queue_contains_reflects_membership() {
    let a = SongId::compute(Path::new("a.mp3"), 1, 1);
    let b = SongId::compute(Path::new("b.mp3"), 2, 2);
    let queue = Queue::new(vec![a]);

    assert!(queue.contains(a));
    assert!(!queue.contains(b));
}

#[test]
fn player_toggle_moves_between_playing_and_paused() {
    let mut player = Player::new(NullBackend::new());
    let dir = TempDir::new().unwrap();
    let path = write_song(dir.path(), "song.wav", "Song", "Artist", "Album");
    let song = lyre_core::song::Song::load(&path).unwrap();

    player.play(&song).unwrap();
    assert_eq!(player.state(), PlaybackState::Playing);

    player.toggle().unwrap();
    assert_eq!(player.state(), PlaybackState::Paused);

    player.toggle().unwrap();
    assert_eq!(player.state(), PlaybackState::Playing);
}

#[test]
fn player_stop_returns_to_idle_and_is_a_no_op_when_already_idle() {
    let mut player = Player::new(NullBackend::new());
    assert_eq!(player.state(), PlaybackState::Idle);

    assert!(player.stop().is_ok());
    assert_eq!(player.state(), PlaybackState::Idle);
}

#[test]
fn player_volume_is_clamped_between_zero_and_one() {
    let mut player = Player::new(NullBackend::new());

    player.set_volume(5.0);
    assert_eq!(player.volume(), 1.0);

    player.set_volume(-5.0);
    assert_eq!(player.volume(), 0.0);
}

#[test]
fn player_adjust_volume_clamps_at_the_bounds() {
    let mut player = Player::new(NullBackend::new());
    player.set_volume(0.05);

    player.adjust_volume(-1.0);
    assert_eq!(player.volume(), 0.0);

    player.adjust_volume(1.0);
    assert_eq!(player.volume(), 1.0);
}

#[test]
fn null_backend_reports_no_position_until_something_is_loaded() {
    let backend = NullBackend::new();
    assert_eq!(backend.position(), None);
}

#[test]
fn null_backend_position_advances_after_play_uri() {
    let mut backend = NullBackend::new();
    backend.play_uri("file:///tmp/whatever.mp3").unwrap();
    assert!(backend.position().is_some());

    std::thread::sleep(Duration::from_millis(20));
    assert!(backend.position().unwrap() >= Duration::from_millis(20));
}

#[test]
fn null_backend_stop_clears_the_loaded_track() {
    let mut backend = NullBackend::new();
    backend.play_uri("file:///tmp/whatever.mp3").unwrap();
    assert!(backend.position().is_some());

    backend.stop().unwrap();
    assert_eq!(backend.position(), None);
}

#[test]
fn atomic_write_round_trips_bytes_and_leaves_no_temp_file_behind() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("data.bin");

    lyre_core::atomic::write(&path, b"hello world").unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"hello world");
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "no temp files should remain after a successful write");
}

#[test]
fn song_falls_back_to_the_file_stem_when_no_title_tag_is_present() {
    let dir = TempDir::new().unwrap();
    let path = write_song(dir.path(), "untitled_track.wav", "", "", "");
    let song = lyre_core::song::Song::load(&path).unwrap();

    assert_eq!(song.title(), "untitled_track");
    assert_eq!(song.artist(), "Unknown Artist");
    assert_eq!(song.album(), "Unknown Album");
}

#[test]
fn song_sort_keys_are_lowercase() {
    let dir = TempDir::new().unwrap();
    let path = write_song(dir.path(), "song.wav", "Song Title", "Song Artist", "Song Album");
    let song = lyre_core::song::Song::load(&path).unwrap();

    assert_eq!(song.sort_title(), "song title");
    assert_eq!(song.sort_artist(), "song artist");
}

#[test]
fn song_fuzzy_term_score_requires_every_term_to_match() {
    let dir = TempDir::new().unwrap();
    let path = write_song(dir.path(), "song.wav", "Neon Skyline", "Static Prairie", "Wide Fields");
    let song = lyre_core::song::Song::load(&path).unwrap();

    assert!(song.fuzzy_score(&["neon"]).is_some());
    assert!(song.fuzzy_score(&["neon", "prairie"]).is_some());
    assert!(song.fuzzy_score(&["neon", "zzz"]).is_none(), "a term that matches nothing should fail the whole query");
}

#[test]
fn song_fuzzy_term_score_expects_an_already_lowercased_term() {
    let dir = TempDir::new().unwrap();
    let path = write_song(dir.path(), "song.wav", "Neon Skyline", "Artist", "Album");
    let song = lyre_core::song::Song::load(&path).unwrap();

    assert!(song.fuzzy_score(&["neon"]).is_some(), "callers are expected to lowercase the query before matching");
}

#[test]
fn song_fuzzy_term_score_rewards_a_match_at_a_word_boundary() {
    let dir = TempDir::new().unwrap();
    let boundary = write_song(dir.path(), "boundary.wav", "Blue Sky", "Artist", "Album");
    let midword = write_song(dir.path(), "midword.wav", "Ruby Sky", "Artist", "Album");
    let boundary_song = lyre_core::song::Song::load(&boundary).unwrap();
    let midword_song = lyre_core::song::Song::load(&midword).unwrap();

    let boundary_score = boundary_song.fuzzy_term_score("b").unwrap();
    let midword_score = midword_song.fuzzy_term_score("b").unwrap();

    assert!(boundary_score > midword_score, "a match at a word boundary should score higher than mid-word");
}

#[test]
fn song_fuzzy_term_score_of_an_empty_term_matches_everything_with_zero_score() {
    let dir = TempDir::new().unwrap();
    let path = write_song(dir.path(), "song.wav", "Anything", "Artist", "Album");
    let song = lyre_core::song::Song::load(&path).unwrap();

    assert_eq!(song.fuzzy_term_score(""), Some(0));
}
