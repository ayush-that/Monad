//! Player state management.

use dioxus::prelude::*;
use monad_core::Track;

/// Playback state.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
    Buffering,
}

/// Player state for the UI.
#[derive(Clone)]
pub struct PlayerState {
    /// Current track being played.
    pub current_track: Signal<Option<Track>>,
    /// Playback status.
    pub status: Signal<PlaybackStatus>,
    /// Current position in seconds.
    pub position: Signal<f64>,
    /// Total duration in seconds.
    pub duration: Signal<f64>,
}

impl PlayerState {
    /// Create a new player state.
    pub fn new() -> Self {
        Self {
            current_track: Signal::new(None),
            status: Signal::new(PlaybackStatus::Stopped),
            position: Signal::new(0.0),
            duration: Signal::new(0.0),
        }
    }

    /// Set the current track.
    pub fn set_track(&mut self, track: Option<Track>) {
        *self.current_track.write() = track;
        *self.position.write() = 0.0;
    }

    /// Start or resume playback.
    pub fn play(&mut self) {
        *self.status.write() = PlaybackStatus::Playing;
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        *self.status.write() = PlaybackStatus::Paused;
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::new()
    }
}
