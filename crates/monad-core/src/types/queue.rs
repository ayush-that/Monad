//! Queue management types.

#![allow(clippy::unwrap_used)] // Tests use unwrap for brevity

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Track;

/// A single item in the playback queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueueItem {
    /// Unique identifier for this queue item.
    pub id: Uuid,
    /// The track to play.
    pub track: Track,
    /// Source of this queue item.
    pub source: QueueSource,
}

impl QueueItem {
    pub fn new(track: Track, source: QueueSource) -> Self {
        Self {
            id: Uuid::new_v4(),
            track,
            source,
        }
    }

    pub fn from_track(track: Track) -> Self {
        Self::new(track, QueueSource::Manual)
    }
}

/// Source of how an item was added to the queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QueueSource {
    /// Manually added by user.
    Manual,
    /// From playing an album.
    Album { id: String, name: String },
    /// From playing a playlist.
    Playlist { id: String, name: String },
    /// From playing an artist's songs.
    Artist { id: String, name: String },
    /// From search results.
    Search { query: String },
    /// Auto-generated recommendations.
    AutoPlay,
}

/// The playback queue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Queue {
    /// All items in the queue.
    items: Vec<QueueItem>,
    /// Current playback index.
    current_index: Option<usize>,
    /// Repeat mode.
    repeat_mode: RepeatMode,
    /// Shuffle enabled.
    shuffle: bool,
    /// Shuffle order (indices into items).
    shuffle_order: Vec<usize>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all items in the queue.
    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    /// Get the current track.
    pub fn current(&self) -> Option<&QueueItem> {
        self.current_index.and_then(|i| self.items.get(i))
    }

    /// Get the current index.
    pub const fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Get the number of items in the queue.
    pub const fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the queue is empty.
    pub const fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add a track to the end of the queue.
    pub fn push(&mut self, item: QueueItem) {
        let index = self.items.len();
        self.items.push(item);
        self.shuffle_order.push(index);

        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
    }

    /// Insert a track at a specific position.
    pub fn insert(&mut self, index: usize, item: QueueItem) {
        let index = index.min(self.items.len());
        self.items.insert(index, item);

        // Update current index if necessary
        if let Some(current) = self.current_index {
            if index <= current {
                self.current_index = Some(current + 1);
            }
        } else {
            self.current_index = Some(0);
        }

        // Rebuild shuffle order
        self.rebuild_shuffle_order();
    }

    /// Remove an item by its UUID.
    pub fn remove(&mut self, id: Uuid) -> Option<QueueItem> {
        let index = self.items.iter().position(|item| item.id == id)?;
        self.remove_at(index)
    }

    /// Remove an item at a specific index.
    pub fn remove_at(&mut self, index: usize) -> Option<QueueItem> {
        if index >= self.items.len() {
            return None;
        }

        let item = self.items.remove(index);

        // Update current index
        if let Some(current) = self.current_index {
            if self.items.is_empty() {
                self.current_index = None;
            } else if index < current {
                self.current_index = Some(current - 1);
            } else if index == current && current >= self.items.len() {
                self.current_index = Some(self.items.len() - 1);
            }
        }

        // Rebuild shuffle order
        self.rebuild_shuffle_order();

        Some(item)
    }

    /// Clear the entire queue.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
        self.shuffle_order.clear();
    }

    /// Set the queue contents, replacing everything.
    pub fn set(&mut self, items: Vec<QueueItem>, start_index: usize) {
        self.items = items;
        self.current_index = if self.items.is_empty() {
            None
        } else {
            Some(start_index.min(self.items.len() - 1))
        };
        self.rebuild_shuffle_order();
    }

    /// Move to the next track.
    #[allow(clippy::should_implement_trait)] // Not implementing Iterator
    pub fn advance(&mut self) -> Option<&QueueItem> {
        if self.items.is_empty() {
            return None;
        }

        let next_index = if self.shuffle {
            self.next_shuffle_index()
        } else {
            self.next_sequential_index()
        };

        if let Some(index) = next_index {
            self.current_index = Some(index);
            self.items.get(index)
        } else {
            None
        }
    }

    /// Move to the previous track.
    pub fn previous(&mut self) -> Option<&QueueItem> {
        if self.items.is_empty() {
            return None;
        }

        let prev_index = if self.shuffle {
            self.prev_shuffle_index()
        } else {
            self.prev_sequential_index()
        };

        if let Some(index) = prev_index {
            self.current_index = Some(index);
            self.items.get(index)
        } else {
            None
        }
    }

    /// Jump to a specific index.
    pub fn jump_to(&mut self, index: usize) -> Option<&QueueItem> {
        if index < self.items.len() {
            self.current_index = Some(index);
            self.items.get(index)
        } else {
            None
        }
    }

    /// Get repeat mode.
    pub const fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    /// Set repeat mode.
    pub const fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat_mode = mode;
    }

    /// Cycle through repeat modes.
    pub const fn cycle_repeat(&mut self) -> RepeatMode {
        self.repeat_mode = match self.repeat_mode {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        };
        self.repeat_mode
    }

    /// Check if shuffle is enabled.
    pub const fn is_shuffle(&self) -> bool {
        self.shuffle
    }

    /// Set shuffle mode.
    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.shuffle = shuffle;
        if shuffle {
            self.rebuild_shuffle_order();
        }
    }

    /// Toggle shuffle mode.
    pub fn toggle_shuffle(&mut self) -> bool {
        self.set_shuffle(!self.shuffle);
        self.shuffle
    }

    fn next_sequential_index(&self) -> Option<usize> {
        let current = self.current_index.unwrap_or(0);

        match self.repeat_mode {
            RepeatMode::One => Some(current),
            RepeatMode::All => Some((current + 1) % self.items.len()),
            RepeatMode::Off => {
                if current + 1 < self.items.len() {
                    Some(current + 1)
                } else {
                    None
                }
            }
        }
    }

    fn prev_sequential_index(&self) -> Option<usize> {
        let current = self.current_index.unwrap_or(0);

        match self.repeat_mode {
            RepeatMode::One => Some(current),
            RepeatMode::All => Some(if current == 0 {
                self.items.len() - 1
            } else {
                current - 1
            }),
            RepeatMode::Off => {
                if current > 0 {
                    Some(current - 1)
                } else {
                    None
                }
            }
        }
    }

    fn next_shuffle_index(&self) -> Option<usize> {
        if self.shuffle_order.is_empty() {
            return None;
        }

        let current = self.current_index.unwrap_or(0);
        let shuffle_pos = self
            .shuffle_order
            .iter()
            .position(|&i| i == current)
            .unwrap_or(0);

        match self.repeat_mode {
            RepeatMode::One => Some(current),
            RepeatMode::All => {
                let next_pos = (shuffle_pos + 1) % self.shuffle_order.len();
                Some(self.shuffle_order[next_pos])
            }
            RepeatMode::Off => {
                if shuffle_pos + 1 < self.shuffle_order.len() {
                    Some(self.shuffle_order[shuffle_pos + 1])
                } else {
                    None
                }
            }
        }
    }

    fn prev_shuffle_index(&self) -> Option<usize> {
        if self.shuffle_order.is_empty() {
            return None;
        }

        let current = self.current_index.unwrap_or(0);
        let shuffle_pos = self
            .shuffle_order
            .iter()
            .position(|&i| i == current)
            .unwrap_or(0);

        match self.repeat_mode {
            RepeatMode::One => Some(current),
            RepeatMode::All => {
                let prev_pos = if shuffle_pos == 0 {
                    self.shuffle_order.len() - 1
                } else {
                    shuffle_pos - 1
                };
                Some(self.shuffle_order[prev_pos])
            }
            RepeatMode::Off => {
                if shuffle_pos > 0 {
                    Some(self.shuffle_order[shuffle_pos - 1])
                } else {
                    None
                }
            }
        }
    }

    fn rebuild_shuffle_order(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        self.shuffle_order = (0..self.items.len()).collect();

        if self.shuffle && self.shuffle_order.len() > 1 {
            // Simple deterministic shuffle using item IDs as seed
            let mut hasher = DefaultHasher::new();
            for item in &self.items {
                item.id.hash(&mut hasher);
            }
            let seed = hasher.finish();

            // Fisher-Yates shuffle with deterministic random
            let mut rng_state = seed;
            for i in (1..self.shuffle_order.len()).rev() {
                rng_state = rng_state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                #[allow(clippy::cast_possible_truncation)]
                let j = (rng_state as usize) % (i + 1);
                self.shuffle_order.swap(i, j);
            }
        }
    }
}

/// Repeat mode for playback.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    /// No repeat.
    #[default]
    Off,
    /// Repeat the entire queue.
    All,
    /// Repeat the current track.
    One,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Track;

    fn make_track(id: &str) -> Track {
        Track::new(id, format!("Track {id}"))
    }

    #[test]
    fn test_queue_push_and_current() {
        let mut queue = Queue::new();
        assert!(queue.is_empty());
        assert!(queue.current().is_none());

        queue.push(QueueItem::from_track(make_track("1")));
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.current().unwrap().track.id, "1");
    }

    #[test]
    fn test_queue_next() {
        let mut queue = Queue::new();
        queue.push(QueueItem::from_track(make_track("1")));
        queue.push(QueueItem::from_track(make_track("2")));
        queue.push(QueueItem::from_track(make_track("3")));

        assert_eq!(queue.current().unwrap().track.id, "1");
        assert_eq!(queue.advance().unwrap().track.id, "2");
        assert_eq!(queue.advance().unwrap().track.id, "3");
        assert!(queue.advance().is_none()); // No repeat
    }

    #[test]
    fn test_queue_repeat_all() {
        let mut queue = Queue::new();
        queue.push(QueueItem::from_track(make_track("1")));
        queue.push(QueueItem::from_track(make_track("2")));
        queue.set_repeat_mode(RepeatMode::All);

        queue.advance(); // -> 2
        assert_eq!(queue.advance().unwrap().track.id, "1"); // wraps around
    }

    #[test]
    fn test_queue_remove() {
        let mut queue = Queue::new();
        let item1 = QueueItem::from_track(make_track("1"));
        let id1 = item1.id;
        queue.push(item1);
        queue.push(QueueItem::from_track(make_track("2")));

        assert_eq!(queue.len(), 2);
        queue.remove(id1);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.current().unwrap().track.id, "2");
    }
}
