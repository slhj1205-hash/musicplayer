use ratatui::widgets::ListState;

use lyre_core::{PlaylistId, SongId};

use crate::keymap::Direction;

use super::state::{Panel, PlaylistView, QueueSource, Row, StatusKind};
use super::App;

impl App {
    pub fn queue_source(&self) -> QueueSource {
        self.queue_source
    }

    pub(super) fn active_list_state_mut(&mut self) -> &mut ListState {
        match self.panel {
            Panel::Library => &mut self.library_panel.list_state,
            Panel::Playlists => &mut self.playlist_panel.list_state,
        }
    }

    pub(super) fn active_list_state(&self) -> &ListState {
        match self.panel {
            Panel::Library => &self.library_panel.list_state,
            Panel::Playlists => &self.playlist_panel.list_state,
        }
    }

    pub(super) fn active_page_height(&self) -> usize {
        match self.panel {
            Panel::Library => self.library_panel.page_height,
            Panel::Playlists => self.playlist_panel.page_height,
        }
    }

    pub fn visible_playlist_ids(&self) -> Vec<PlaylistId> {
        let needle = self.playlist_panel.search_query.to_lowercase();
        self.playlists
            .ids_sorted_by_name()
            .iter()
            .copied()
            .filter(|&id| {
                needle.is_empty()
                    || self.playlists.get(id).map(|p| p.name().to_lowercase().contains(&needle)).unwrap_or(false)
            })
            .collect()
    }

    pub(super) fn move_selection(&mut self, delta: isize) {
        if self.panel == Panel::Playlists && self.playlist_panel.view == PlaylistView::Browsing {
            self.move_playlist_browse_selection(delta);
            return;
        }

        let len = self.visible_rows().len();
        if len == 0 {
            self.active_list_state_mut().select(None);
            return;
        }

        let start = self.active_list_state().selected().unwrap_or(0);
        let rows = self.rows_slice();
        let target = wrapping_selectable_index(start, delta, len, |i| matches!(rows[i], Row::Song(_, _)));
        self.active_list_state_mut().select(Some(target));
    }

    pub(super) fn move_playlist_browse_selection(&mut self, delta: isize) {
        let len = self.visible_playlist_ids().len();
        if len == 0 {
            self.playlist_panel.list_state.select(None);
            return;
        }
        let start = self.playlist_panel.list_state.selected().unwrap_or(0) as isize;
        let idx = (start + delta).rem_euclid(len as isize);
        self.playlist_panel.list_state.select(Some(idx as usize));
    }

    pub(super) fn jump_page(&mut self, direction: Direction) {
        if self.panel == Panel::Playlists && self.playlist_panel.view == PlaylistView::Browsing {
            self.jump_playlist_browse_page(direction);
            return;
        }

        let len = self.visible_rows().len();
        if len == 0 {
            self.active_list_state_mut().select(None);
            return;
        }
        let height = self.active_page_height();
        let offset = self.active_list_state().offset();

        let (new_offset, target) = {
            let rows = self.rows_slice();
            let is_selectable = |i: usize| matches!(rows[i], Row::Song(_, _));
            compute_jump(offset, len, height, direction, is_selectable)
        };

        let state = self.active_list_state_mut();
        *state.offset_mut() = new_offset;
        state.select(Some(target));
    }

    fn jump_playlist_browse_page(&mut self, direction: Direction) {
        let len = self.visible_playlist_ids().len();
        if len == 0 {
            self.playlist_panel.list_state.select(None);
            return;
        }
        let height = self.playlist_panel.page_height;
        let offset = self.playlist_panel.list_state.offset();

        let (new_offset, target) = compute_jump(offset, len, height, direction, |_| true);

        *self.playlist_panel.list_state.offset_mut() = new_offset;
        self.playlist_panel.list_state.select(Some(target));
    }

    pub(super) fn select_first_row(&mut self) {
        if self.panel == Panel::Playlists && self.playlist_panel.view == PlaylistView::Browsing {
            let len = self.visible_playlist_ids().len();
            self.playlist_panel.list_state.select(if len == 0 { None } else { Some(0) });
            return;
        }
        let target = self.visible_rows().iter().position(|r| matches!(r, Row::Song(_, _)));
        self.active_list_state_mut().select(target);
    }

    pub(super) fn select_last_row(&mut self) {
        if self.panel == Panel::Playlists && self.playlist_panel.view == PlaylistView::Browsing {
            let last = self.visible_playlist_ids().len().checked_sub(1);
            self.playlist_panel.list_state.select(last);
            return;
        }
        let target = self.visible_rows().iter().rposition(|r| matches!(r, Row::Song(_, _)));
        self.active_list_state_mut().select(target);
    }

    pub fn selected_row(&mut self) -> Option<Row> {
        let i = self.active_list_state().selected()?;
        self.visible_rows().get(i).cloned()
    }

    fn rows_slice(&self) -> &[Row] {
        self.rows.rows_unchecked()
    }

    pub(super) fn selected_playlist_id(&self) -> Option<PlaylistId> {
        let ids = self.visible_playlist_ids();
        let i = self.playlist_panel.list_state.selected()?;
        ids.get(i).copied()
    }

    pub(super) fn reset_playlist_browse_selection(&mut self) {
        self.playlist_panel.list_state = ListState::default();
        if !self.visible_playlist_ids().is_empty() {
            self.playlist_panel.list_state.select(Some(0));
        }
    }

    pub(super) fn sync_playlist_browse_selection(&mut self) {
        let len = self.visible_playlist_ids().len();
        if len == 0 {
            self.playlist_panel.list_state.select(None);
            return;
        }
        let start = match self.playlist_panel.list_state.selected() {
            Some(i) if i < len => i,
            _ => 0,
        };
        self.playlist_panel.list_state.select(Some(start));
    }

    pub(super) fn sync_playlist_selection(&mut self) {
        match self.playlist_panel.view {
            PlaylistView::Browsing => self.sync_playlist_browse_selection(),
            PlaylistView::Viewing(_) => self.sync_selection_to_rows(),
        }
    }

    pub(super) fn jump_to_current(&mut self) {
        let Some(current) = self.queue.current_id() else {
            self.set_status("nothing is playing", StatusKind::Info);
            return;
        };

        if !self.select_song_by_id(current) {
            self.set_status(
                "now playing isn't in the current view -- clear the search to find it",
                StatusKind::Info,
            );
        }
    }

    pub(super) fn select_song_by_id(&mut self, id: SongId) -> bool {
        match self.visible_rows().iter().position(|row| matches!(row, Row::Song(row_id, _) if *row_id == id)) {
            Some(i) => {
                self.active_list_state_mut().select(Some(i));
                true
            }
            None => false,
        }
    }

    pub(super) fn sync_selection_to_rows(&mut self) {
        let len = self.visible_rows().len();
        if len == 0 {
            self.active_list_state_mut().select(None);
            return;
        }

        let start = match self.active_list_state().selected() {
            Some(i) if i < len => i,
            _ => 0,
        };

        let landing = nearest_song_row(self.rows_slice(), start);
        self.active_list_state_mut().select(Some(landing));
    }

    pub(super) fn cycle_category(&mut self, direction: Direction) {
        match self.panel {
            Panel::Library => {
                self.library_panel.category =
                    match direction {
                        Direction::Forwards => self.library_panel.category.next(),
                        Direction::Backwards => self.library_panel.category.prev(),
                    };
                self.sync_selection_to_rows();
                self.set_status(format!("grouped by {}", self.library_panel.category.label()), StatusKind::Info);
            }
            Panel::Playlists => {
                if matches!(self.playlist_panel.view, PlaylistView::Viewing(_)) {
                    self.playlist_panel.category =
                        match direction {
                            Direction::Forwards => self.playlist_panel.category.next(),
                            Direction::Backwards => self.playlist_panel.category.prev(),
                        };
                    self.sync_selection_to_rows();
                    self.set_status(
                        format!("grouped by {}", self.playlist_panel.category.label()),
                        StatusKind::Info,
                    );
                }
            }
        }
    }

    pub(super) fn cycle_sort(&mut self, direction: Direction) {
        match self.panel {
            Panel::Library => {
                self.library_panel.sort =
                    match direction {
                        Direction::Forwards => self.library_panel.sort.next(),
                        Direction::Backwards => self.library_panel.sort.prev(),
                    };
                self.sync_selection_to_rows();
                self.set_status(format!("sorted by {}", self.library_panel.sort.label()), StatusKind::Info);
            }
            Panel::Playlists => {
                if matches!(self.playlist_panel.view, PlaylistView::Viewing(_)) {
                    self.playlist_panel.sort =
                        match direction {
                            Direction::Forwards => self.playlist_panel.sort.next(),
                            Direction::Backwards => self.playlist_panel.sort.prev(),
                        };
                    self.sync_selection_to_rows();
                    self.set_status(format!("sorted by {}", self.playlist_panel.sort.label()), StatusKind::Info);
                }
            }
        }
    }

    pub(super) fn cycle_library_playlist_mode(&mut self) {
        if self.panel != Panel::Library {
            return;
        }
        self.library_panel.playlist_mode = self.library_panel.playlist_mode.cycle();
        self.set_status(format!("playlists: {}", self.library_panel.playlist_mode.label()), StatusKind::Info);
    }
}

fn nearest_song_row(rows: &[Row], start: usize) -> usize {
    if matches!(rows[start], Row::Song(_, _)) {
        return start;
    }
    rows.iter()
        .enumerate()
        .skip(start)
        .find(|(_, r)| matches!(r, Row::Song(_, _)))
        .or_else(|| rows.iter().enumerate().find(|(_, r)| matches!(r, Row::Song(_, _))))
        .map(|(i, _)| i)
        .unwrap_or(start)
}

fn wrapping_selectable_index(start: usize, delta: isize, len: usize, is_selectable: impl Fn(usize) -> bool) -> usize {
    let len = len as isize;
    let start = start as isize;
    let mut idx = start;
    loop {
        idx = (idx + delta).rem_euclid(len);
        if idx == start || is_selectable(idx as usize) {
            return idx as usize;
        }
    }
}

pub(super) fn move_wrapping(state: &mut ListState, len: usize, delta: isize) {
    if len == 0 {
        return;
    }
    let start = state.selected().unwrap_or(0) as isize;
    let idx = (start + delta).rem_euclid(len as isize);
    state.select(Some(idx as usize));
}

fn compute_jump(offset: usize, len: usize, height: usize, direction: Direction, is_selectable: impl Fn(usize) -> bool) -> (usize, usize) {
    let height = height.max(1);
    let offset = offset.min(len.saturating_sub(1));

    match direction {
        Direction::Forwards => {
            let last_visible = (offset + height - 1).min(len - 1);
            if last_visible > offset {

                let mid = (last_visible + height / 2).min(len - 1);
                let target = nearest_selectable(mid, len, &is_selectable).unwrap_or(mid);
                (last_visible, target)
            } else {

                let target = last_selectable(len, &is_selectable).unwrap_or(len - 1);
                (len.saturating_sub(height), target)
            }
        },
        Direction::Backwards => {
            if offset > 0 {
                let new_offset = offset.saturating_sub(height - 1);
                let mid = (new_offset + height / 2).min(len - 1);
                let target = nearest_selectable(mid, len, &is_selectable).unwrap_or(mid);
                (new_offset, target)
            } else {
                let target = first_selectable(len, &is_selectable).unwrap_or(0);
                (0, target)
            }
        },
    }
}

fn nearest_selectable(idx: usize, len: usize, is_selectable: &impl Fn(usize) -> bool) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let idx = idx.min(len - 1);
    for radius in 0..len {
        if idx + radius < len && is_selectable(idx + radius) {
            return Some(idx + radius);
        }
        if radius <= idx && is_selectable(idx - radius) {
            return Some(idx - radius);
        }
    }
    None
}

fn first_selectable(len: usize, is_selectable: &impl Fn(usize) -> bool) -> Option<usize> {
    (0..len).find(|&i| is_selectable(i))
}

fn last_selectable(len: usize, is_selectable: &impl Fn(usize) -> bool) -> Option<usize> {
    (0..len).rev().find(|&i| is_selectable(i))
}
