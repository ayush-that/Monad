//! Application state management.

pub mod battery;
pub mod ipod;
pub mod player;

pub use player::PlayerState;

use dioxus::prelude::*;
use monad_core::{Queue, QueueItem, Track};

/// Global application state.
#[derive(Clone)]
pub struct AppState {
    /// Player state.
    pub player: PlayerState,
    /// Playback queue.
    pub queue: Signal<Queue>,
}

impl AppState {
    /// Create a new application state.
    pub fn new() -> Self {
        Self {
            player: PlayerState::new(),
            queue: Signal::new(Queue::new()),
        }
    }

    /// Add a track to the queue and optionally start playback.
    pub fn play_track(&mut self, track: Track, play_now: bool) {
        self.queue
            .write()
            .push(QueueItem::from_track(track.clone()));

        if play_now {
            // Jump to the newly added track
            let idx = self.queue.read().len().saturating_sub(1);
            self.queue.write().jump_to(idx);
            self.player.set_track(Some(track));
            self.player.play();
        }
    }

    /// Play the next track in queue.
    pub fn next_track(&mut self) {
        if let Some(item) = self.queue.write().advance() {
            let track = item.track.clone();
            self.player.set_track(Some(track));
            self.player.play();
        }
    }

    /// Play the previous track.
    pub fn previous_track(&mut self) {
        if let Some(item) = self.queue.write().previous() {
            let track = item.track.clone();
            self.player.set_track(Some(track));
            self.player.play();
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
