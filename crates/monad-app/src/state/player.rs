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
    /// Volume level (0.0 to 1.0).
    pub volume: Signal<f32>,
    /// Whether the player is muted.
    pub muted: Signal<bool>,
    /// Shuffle mode enabled.
    pub shuffle: Signal<bool>,
    /// Repeat mode.
    pub repeat: Signal<RepeatMode>,
}

/// Repeat mode options.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    /// Cycle to the next repeat mode.
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

impl PlayerState {
    /// Create a new player state.
    pub fn new() -> Self {
        Self {
            current_track: Signal::new(None),
            status: Signal::new(PlaybackStatus::Stopped),
            position: Signal::new(0.0),
            duration: Signal::new(0.0),
            volume: Signal::new(1.0),
            muted: Signal::new(false),
            shuffle: Signal::new(false),
            repeat: Signal::new(RepeatMode::Off),
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

    /// Toggle play/pause.
    pub fn toggle_play(&mut self) {
        let current = *self.status.read();
        match current {
            PlaybackStatus::Playing => self.pause(),
            PlaybackStatus::Paused | PlaybackStatus::Stopped => self.play(),
            PlaybackStatus::Buffering => {}
        }
    }

    /// Stop playback.
    pub fn stop(&mut self) {
        *self.status.write() = PlaybackStatus::Stopped;
        *self.position.write() = 0.0;
    }

    /// Seek to a position.
    pub fn seek(&mut self, position: f64) {
        *self.position.write() = position;
    }

    /// Set the volume.
    pub fn set_volume(&mut self, volume: f32) {
        *self.volume.write() = volume.clamp(0.0, 1.0);
        if volume > 0.0 {
            *self.muted.write() = false;
        }
    }

    /// Toggle mute.
    pub fn toggle_mute(&mut self) {
        let muted = *self.muted.read();
        *self.muted.write() = !muted;
    }

    /// Toggle shuffle mode.
    pub fn toggle_shuffle(&mut self) {
        let shuffle = *self.shuffle.read();
        *self.shuffle.write() = !shuffle;
    }

    /// Cycle repeat mode.
    pub fn cycle_repeat(&mut self) {
        let current = *self.repeat.read();
        *self.repeat.write() = current.next();
    }

    /// Get the effective volume (considering mute).
    pub fn effective_volume(&self) -> f32 {
        if *self.muted.read() {
            0.0
        } else {
            *self.volume.read()
        }
    }

    /// Check if currently playing.
    pub fn is_playing(&self) -> bool {
        *self.status.read() == PlaybackStatus::Playing
    }

    /// Format the current position as mm:ss.
    pub fn position_formatted(&self) -> String {
        format_time(*self.position.read())
    }

    /// Format the duration as mm:ss.
    pub fn duration_formatted(&self) -> String {
        format_time(*self.duration.read())
    }

    /// Get progress as a percentage (0.0 to 100.0).
    pub fn progress_percent(&self) -> f64 {
        let duration = *self.duration.read();
        if duration > 0.0 {
            (*self.position.read() / duration) * 100.0
        } else {
            0.0
        }
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Format seconds as mm:ss or hh:mm:ss.
fn format_time(seconds: f64) -> String {
    let total_secs = seconds as u64;
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins}:{secs:02}")
    }
}
