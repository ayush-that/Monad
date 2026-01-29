//! Audio service connecting UI to the audio engine.

use crate::state::player::PlaybackStatus;
use crate::state::AppState;
use dioxus::prelude::*;
use monad_audio::{AudioEngine, EngineCommand, EngineEvent, PlaybackState as EnginePlaybackState};
use monad_core::Track;
use monad_extractor::Extractor;
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Audio service that manages the connection between UI and audio playback.
#[derive(Clone)]
pub struct AudioService {
    engine: Arc<Mutex<Option<AudioEngine>>>,
    extractor: Arc<Extractor>,
}

impl AudioService {
    /// Create a new audio service.
    pub fn new() -> Self {
        let engine = match AudioEngine::new() {
            Ok(engine) => {
                info!("Audio engine initialized successfully");
                Some(engine)
            }
            Err(e) => {
                error!("Failed to initialize audio engine: {e}");
                None
            }
        };

        let extractor = Extractor::new();
        // Keep disk cache - instant playback for previously played songs
        info!("Extractor initialized with disk caching");

        Self {
            engine: Arc::new(Mutex::new(engine)),
            extractor: Arc::new(extractor),
        }
    }

    /// Play a track by downloading audio with yt-dlp and sending to audio engine.
    pub async fn play_track(&self, track: &Track) {
        info!("Playing track: {} - {}", track.title, track.artist_name());

        // Download audio directly using yt-dlp (avoids session-bound URL issues)
        match self.extractor.extract(&track.id).await {
            Ok(audio) => {
                info!(
                    "Downloaded audio for track: {} ({} bytes, {})",
                    track.id,
                    audio.data.len(),
                    audio.mime_type
                );
                // Send raw audio data directly to the engine
                self.send_command(EngineCommand::LoadData(audio.data, Some(audio.mime_type)));
            }
            Err(e) => {
                error!("Failed to download audio for track {}: {}", track.id, e);
            }
        }
    }

    /// Send a command to the audio engine.
    pub fn send_command(&self, command: EngineCommand) {
        if let Some(engine) = self.engine.lock().as_ref() {
            if let Err(e) = engine.send_command(command) {
                error!("Failed to send command to audio engine: {e}");
            }
        } else {
            warn!("Audio engine not available");
        }
    }

    /// Play/resume playback.
    pub fn play(&self) {
        self.send_command(EngineCommand::Play);
    }

    /// Pause playback.
    pub fn pause(&self) {
        self.send_command(EngineCommand::Pause);
    }

    /// Try to receive an event from the audio engine.
    pub fn try_recv_event(&self) -> Option<EngineEvent> {
        self.engine.lock().as_ref()?.try_recv_event()
    }
}

impl Default for AudioService {
    fn default() -> Self {
        Self::new()
    }
}

/// Hook to initialize and use the audio service.
pub fn use_audio_service() -> Signal<AudioService> {
    use_context_provider(|| Signal::new(AudioService::new()))
}

/// Hook to sync audio engine events with app state.
/// This should be called in the App component.
pub fn use_audio_event_sync(audio: Signal<AudioService>, app_state: AppState) {
    // Copy signals (they're Copy)
    let mut player_status = app_state.player.status;
    let mut player_position = app_state.player.position;
    let mut player_duration = app_state.player.duration;
    let mut queue = app_state.queue;
    let mut player_current_track = app_state.player.current_track;

    use_future(move || async move {
        loop {
            // Poll for events from the audio engine
            let service = audio.read();
            while let Some(event) = service.try_recv_event() {
                match event {
                    EngineEvent::StateChanged(state) => {
                        debug!("Playback state changed: {:?}", state);
                        let status = match state {
                            EnginePlaybackState::Stopped => PlaybackStatus::Stopped,
                            EnginePlaybackState::Playing => PlaybackStatus::Playing,
                            EnginePlaybackState::Paused => PlaybackStatus::Paused,
                            EnginePlaybackState::Buffering => PlaybackStatus::Buffering,
                        };
                        *player_status.write() = status;
                    }
                    EngineEvent::PositionUpdate(pos) => {
                        *player_position.write() = pos;
                    }
                    EngineEvent::DurationUpdate(dur) => {
                        *player_duration.write() = dur;
                    }
                    EngineEvent::BufferingProgress(progress) => {
                        debug!("Buffering: {:.0}%", progress * 100.0);
                    }
                    EngineEvent::TrackLoaded => {
                        debug!("Track loaded, starting playback");
                        // Auto-play when track is loaded
                        service.play();
                    }
                    EngineEvent::PlaybackFinished => {
                        info!("Playback finished, advancing to next track");
                        // Auto-advance to next track
                        if let Some(item) = queue.write().advance() {
                            let track = item.track.clone();
                            *player_current_track.write() = Some(track.clone());
                            *player_status.write() = PlaybackStatus::Buffering;
                            // Play the next track
                            let audio_clone = audio;
                            spawn(async move {
                                audio_clone.read().play_track(&track).await;
                            });
                        } else {
                            *player_status.write() = PlaybackStatus::Stopped;
                        }
                    }
                    EngineEvent::Error(err) => {
                        error!("Playback error: {err}");
                    }
                }
            }
            drop(service);

            // Small delay to prevent busy loop
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    });
}
