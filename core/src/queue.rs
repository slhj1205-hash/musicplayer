use std::collections::VecDeque;
use std::cmp::Ordering;

use rand::seq::SliceRandom;

use crate::{library::Library, playlist::Playlist, song::SongId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NowPlaying {
    Queued,
    Priority(SongId),
}

pub struct Queue {
    songs: Vec<SongId>,

    order: Vec<usize>,

    cursor: Option<usize>,
    playing: Option<NowPlaying>,
    priority: VecDeque<SongId>,
}

impl Queue {
    pub fn new(songs: Vec<SongId>) -> Queue {
        let order: Vec<usize> = (0..songs.len()).collect();

        Queue { songs, order, cursor: None, playing: None, priority: VecDeque::new() }
    }

    pub fn from_playlist(playlist: &Playlist) -> Queue {
        Queue::new(playlist.songs().to_vec())
    }

    pub fn len(&self) -> usize {
        self.songs.len()
    }
    pub fn is_empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn current_id(&self) -> Option<SongId> {
        match self.playing? {
            NowPlaying::Priority(id) => Some(id),
            NowPlaying::Queued => self.song_at(self.cursor?),
        }
    }

    pub fn clear_current(&mut self) {
        self.cursor = None;
        self.playing = None;
    }

    pub fn current<'a>(&self, library: &'a Library) -> Option<&'a crate::song::Song> {
        self.current_id().and_then(|id| library.get(id))
    }

    pub fn priority_queue(&self) -> &VecDeque<SongId> {
        &self.priority
    }

    pub fn queue_next(&mut self, song: SongId) {
        self.priority.push_back(song);
    }

    pub fn clear_priority_queue(&mut self) {
        self.priority.clear();
    }

    #[inline]
    fn song_at(&self, position: usize) -> Option<SongId> {
        self.order.get(position).map(|&i| self.songs[i])
    }

    fn reindex(&mut self, previous_song_index: Option<usize>) {
        self.cursor = previous_song_index.and_then(|i| self.order.iter().position(|&song_index| song_index == i));
    }

    fn current_song_index(&self) -> Option<usize> {
        self.cursor.map(|p| self.order[p])
    }

    pub fn sort_by(&mut self, mut compare: impl FnMut(SongId, SongId) -> Ordering) {
        let current = self.current_song_index();
        let songs = &self.songs;
        self.order.sort_by(|&a, &b| compare(songs[a], songs[b]));
        self.reindex(current);
    }

    pub fn sort_by_key<K: Ord>(&mut self, mut key_fn: impl FnMut(SongId) -> K) {
        let current = self.current_song_index();
        let songs = &self.songs;
        self.order.sort_by_key(|&i| key_fn(songs[i]));
        self.reindex(current);
    }

    pub fn shuffle(&mut self) {
        let current = self.current_song_index();
        self.order.shuffle(&mut rand::rng());
        self.reindex(current);
    }

    pub fn unshuffle(&mut self) {
        let current = self.current_song_index();
        self.order.clear();
        self.order.extend(0..self.songs.len());
        self.reindex(current);
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<SongId> {
        if let Some(song) = self.priority.pop_front() {
            self.playing = Some(NowPlaying::Priority(song));
            return Some(song);
        }

        let len = self.order.len();
        if len == 0 {
            self.cursor = None;
            self.playing = None;
            return None;
        }

        self.cursor = Some(match self.cursor {
            Some(p) => (p + 1) % len,
            None => 0,
        });
        self.playing = Some(NowPlaying::Queued);

        self.current_id()
    }

    pub fn previous(&mut self) -> Option<SongId> {
        let len = self.order.len();
        if len == 0 {
            self.cursor = None;
            self.playing = None;
            return None;
        }

        self.cursor = Some(match self.cursor {
            Some(0) | None => len - 1,
            Some(p) => p - 1,
        });
        self.playing = Some(NowPlaying::Queued);

        self.current_id()
    }

    pub fn play_at(&mut self, order_position: usize) -> Option<SongId> {
        if order_position >= self.order.len() {
            return None;
        }
        self.cursor = Some(order_position);
        self.playing = Some(NowPlaying::Queued);
        self.current_id()
    }

    pub fn play_id(&mut self, id: SongId) -> Option<SongId> {
        let len = self.order.len();
        if len == 0 {
            return None;
        }

        let start = self.cursor.map(|p| (p + 1) % len).unwrap_or(0);

        let target = self
            .order
            .iter()
            .enumerate()
            .filter(|&(_, &song_index)| self.songs[song_index] == id)
            .map(|(position, _)| position)
            .min_by_key(|&position| (position + len - start) % len)?;

        self.play_at(target)
    }

    pub fn play_upcoming(&mut self, n: usize) -> Option<SongId> {
        if n == 0 {
            return None;
        }
        let mut result = None;
        for _ in 0..n {
            result = self.next();
        }
        result
    }

    pub fn upcoming(&self, n: usize) -> Vec<SongId> {
        let mut out: Vec<SongId> = self.priority.iter().copied().take(n).collect();
        if out.len() >= n || self.order.is_empty() || self.playing.is_none() {
            out.truncate(n);
            return out;
        }

        let len = self.order.len();
        let mut position = self.cursor;
        while out.len() < n {
            let next = match position {
                Some(p) => (p + 1) % len,
                None => 0,
            };
            out.push(self.songs[self.order[next]]);
            position = Some(next);
        }

        out
    }

    pub fn ordered_ids(&self) -> impl Iterator<Item = SongId> + '_ {
        self.order.iter().map(move |&i| self.songs[i])
    }

    pub fn contains(&self, id: SongId) -> bool {
        self.songs.contains(&id)
    }

    pub fn current_position(&self) -> Option<usize> {
        matches!(self.playing, Some(NowPlaying::Queued)).then_some(self.cursor).flatten()
    }
}
