//! Audio playback engine coordinating decode, resample, and output.

use crate::buffer::{shared_ring_buffer, SharedRingBuffer};
use crate::ffmpeg_decode::FfmpegDecoder;
use crate::output::AudioOutput;
use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use monad_core::{Error, Result};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, trace, warn};

/// Size of the ring buffer in samples (about 2 seconds at 48kHz stereo).
const RING_BUFFER_SIZE: usize = 48000 * 2 * 4;

/// Minimum buffer fill before starting playback (in samples).
const MIN_BUFFER_FILL: usize = 8192;

/// Playback state of the audio engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
    Buffering,
}

/// Commands to control the audio engine.
#[derive(Debug, Clone)]
pub enum EngineCommand {
    /// Play the current track.
    Play,
    /// Pause playback.
    Pause,
    /// Stop playback and reset position.
    Stop,
    /// Seek to a position in seconds.
    Seek(f64),
    /// Set volume (0.0 to 1.0).
    SetVolume(f32),
    /// Load a new track from URL with optional HTTP headers.
    LoadUrl(String, Option<HashMap<String, String>>),
    /// Load audio data directly.
    LoadData(Vec<u8>, Option<String>),
    /// Shutdown the engine.
    Shutdown,
}

/// Events emitted by the audio engine.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Playback state changed.
    StateChanged(PlaybackState),
    /// Position updated (in seconds).
    PositionUpdate(f64),
    /// Duration determined (in seconds).
    DurationUpdate(f64),
    /// Buffering progress (0.0 to 1.0).
    BufferingProgress(f32),
    /// Track loaded successfully.
    TrackLoaded,
    /// Playback finished.
    PlaybackFinished,
    /// Error occurred.
    Error(String),
}

/// High-performance audio playback engine.
pub struct AudioEngine {
    /// Current playback state.
    state: Arc<RwLock<PlaybackState>>,
    /// Current volume (0.0 to 1.0).
    volume: Arc<Mutex<f32>>,
    /// Current position in seconds.
    position: Arc<RwLock<f64>>,
    /// Total duration in seconds.
    duration: Arc<RwLock<Option<f64>>>,
    /// Command sender.
    command_tx: Sender<EngineCommand>,
    /// Event receiver.
    event_rx: Receiver<EngineEvent>,
    /// Ring buffer shared with audio output.
    ring_buffer: SharedRingBuffer,
}

impl AudioEngine {
    /// Create a new audio engine.
    pub fn new() -> Result<Self> {
        let (command_tx, command_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();

        let state = Arc::new(RwLock::new(PlaybackState::Stopped));
        let volume = Arc::new(Mutex::new(0.85f32)); // Slightly below max for headroom
        let position = Arc::new(RwLock::new(0.0f64));
        let duration = Arc::new(RwLock::new(None));
        let ring_buffer = shared_ring_buffer(RING_BUFFER_SIZE);

        // Spawn the engine worker thread - it will create the audio output
        let state_clone = state.clone();
        let volume_clone = volume.clone();
        let position_clone = position.clone();
        let duration_clone = duration.clone();
        let ring_buffer_clone = ring_buffer.clone();

        std::thread::Builder::new()
            .name("audio-engine".to_string())
            .spawn(move || {
                // Create audio output inside the worker thread (cpal::Stream is not Send)
                match AudioOutput::new(
                    ring_buffer_clone.clone(),
                    volume_clone.clone(),
                    state_clone.clone(),
                ) {
                    Ok(output) => {
                        let output_sample_rate = output.sample_rate();
                        let output_channels = output.channels();

                        info!(
                            "Audio output initialized: {} Hz, {} channels, device: {}",
                            output_sample_rate,
                            output_channels,
                            output.device_name()
                        );

                        let worker = EngineWorker::new(
                            command_rx,
                            event_tx.clone(),
                            state_clone,
                            volume_clone,
                            position_clone,
                            duration_clone,
                            ring_buffer_clone,
                            output,
                            output_sample_rate,
                            output_channels,
                        );
                        worker.run();
                    }
                    Err(e) => {
                        error!("Failed to initialize audio output: {e}");
                        let _ = event_tx.send(EngineEvent::Error(format!(
                            "Failed to initialize audio: {e}"
                        )));
                    }
                }
            })
            .map_err(|e| Error::AudioOutput(format!("Failed to spawn engine thread: {e}")))?;

        Ok(Self {
            state,
            volume,
            position,
            duration,
            command_tx,
            event_rx,
            ring_buffer,
        })
    }

    /// Get the current playback state.
    pub fn state(&self) -> PlaybackState {
        *self.state.read()
    }

    /// Get the current volume.
    pub fn volume(&self) -> f32 {
        *self.volume.lock()
    }

    /// Get the ring buffer fill level (0.0 to 1.0).
    #[allow(clippy::cast_precision_loss)]
    pub fn buffer_fill(&self) -> f32 {
        let available = self.ring_buffer.available();
        let capacity = self.ring_buffer.capacity();
        available as f32 / capacity as f32
    }

    /// Get the current position in seconds.
    pub fn position(&self) -> f64 {
        *self.position.read()
    }

    /// Get the total duration in seconds.
    pub fn duration(&self) -> Option<f64> {
        *self.duration.read()
    }

    /// Send a command to the engine.
    pub fn send_command(&self, command: EngineCommand) -> Result<()> {
        self.command_tx
            .send(command)
            .map_err(|e| Error::AudioOutput(format!("Failed to send command: {e}")))
    }

    /// Play the current track.
    pub fn play(&self) -> Result<()> {
        self.send_command(EngineCommand::Play)
    }

    /// Pause playback.
    pub fn pause(&self) -> Result<()> {
        self.send_command(EngineCommand::Pause)
    }

    /// Stop playback.
    pub fn stop(&self) -> Result<()> {
        self.send_command(EngineCommand::Stop)
    }

    /// Seek to a position in seconds.
    pub fn seek(&self, position: f64) -> Result<()> {
        self.send_command(EngineCommand::Seek(position))
    }

    /// Set the volume (0.0 to 1.0).
    pub fn set_volume(&self, volume: f32) -> Result<()> {
        self.send_command(EngineCommand::SetVolume(volume.clamp(0.0, 1.0)))
    }

    /// Load a track from a URL.
    pub fn load_url(&self, url: impl Into<String>) -> Result<()> {
        self.send_command(EngineCommand::LoadUrl(url.into(), None))
    }

    /// Load a track from a URL with custom HTTP headers.
    pub fn load_url_with_headers(
        &self,
        url: impl Into<String>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<()> {
        self.send_command(EngineCommand::LoadUrl(url.into(), headers))
    }

    /// Load audio data directly.
    pub fn load_data(&self, data: Vec<u8>, mime_hint: Option<&str>) -> Result<()> {
        self.send_command(EngineCommand::LoadData(data, mime_hint.map(String::from)))
    }

    /// Try to receive an event without blocking.
    pub fn try_recv_event(&self) -> Option<EngineEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive events, blocking until one is available.
    pub fn recv_event(&self) -> Option<EngineEvent> {
        self.event_rx.recv().ok()
    }

    /// Shutdown the engine.
    pub fn shutdown(&self) -> Result<()> {
        self.send_command(EngineCommand::Shutdown)
    }
}

impl Default for AudioEngine {
    /// # Panics
    /// Panics if the audio engine cannot be initialized.
    #[allow(clippy::expect_used)]
    fn default() -> Self {
        Self::new().expect("Failed to create audio engine")
    }
}

/// Internal worker that runs the audio processing loop.
struct EngineWorker {
    command_rx: Receiver<EngineCommand>,
    event_tx: Sender<EngineEvent>,
    state: Arc<RwLock<PlaybackState>>,
    volume: Arc<Mutex<f32>>,
    position: Arc<RwLock<f64>>,
    duration: Arc<RwLock<Option<f64>>>,
    ring_buffer: SharedRingBuffer,
    /// Keep output alive for the duration of the worker.
    _output: AudioOutput,
    #[allow(dead_code)] // Kept for potential future output configuration
    output_sample_rate: u32,
    #[allow(dead_code)] // Kept for potential future output configuration
    output_channels: u16,
    /// Current decoder (FFmpeg-based for reliable timing).
    decoder: Option<FfmpegDecoder>,
    /// Samples written since start (for position tracking).
    samples_written: u64,
}

impl EngineWorker {
    #[allow(clippy::too_many_arguments)]
    const fn new(
        command_rx: Receiver<EngineCommand>,
        event_tx: Sender<EngineEvent>,
        state: Arc<RwLock<PlaybackState>>,
        volume: Arc<Mutex<f32>>,
        position: Arc<RwLock<f64>>,
        duration: Arc<RwLock<Option<f64>>>,
        ring_buffer: SharedRingBuffer,
        output: AudioOutput,
        output_sample_rate: u32,
        output_channels: u16,
    ) -> Self {
        Self {
            command_rx,
            event_tx,
            state,
            volume,
            position,
            duration,
            ring_buffer,
            _output: output,
            output_sample_rate,
            output_channels,
            decoder: None,
            samples_written: 0,
        }
    }

    fn run(mut self) {
        info!("Audio engine worker started");

        let mut last_position_update = Instant::now();
        let position_update_interval = Duration::from_millis(100);

        loop {
            // Check for commands (non-blocking when playing)
            let command = if *self.state.read() == PlaybackState::Playing {
                match self.command_rx.try_recv() {
                    Ok(cmd) => Some(cmd),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => {
                        debug!("Command channel closed, shutting down");
                        break;
                    }
                }
            } else {
                // Block when not playing
                match self.command_rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(cmd) => Some(cmd),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => None,
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                        debug!("Command channel closed, shutting down");
                        break;
                    }
                }
            };

            if let Some(cmd) = command {
                if matches!(cmd, EngineCommand::Shutdown) {
                    info!("Audio engine shutting down");
                    break;
                }
                self.handle_command(cmd);
            }

            // Process audio if playing
            if *self.state.read() == PlaybackState::Playing {
                self.process_audio();

                // Update position periodically
                if last_position_update.elapsed() >= position_update_interval {
                    self.update_position();
                    last_position_update = Instant::now();
                }
            }

            // Small sleep to prevent busy-waiting
            if self.ring_buffer.free() < 1024 {
                std::thread::sleep(Duration::from_micros(500));
            }
        }
    }

    fn handle_command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::Play => {
                if self.decoder.is_some() {
                    self.set_state(PlaybackState::Playing);
                } else {
                    warn!("Cannot play: no track loaded");
                    let _ = self
                        .event_tx
                        .send(EngineEvent::Error("No track loaded".to_string()));
                }
            }
            EngineCommand::Pause => {
                self.set_state(PlaybackState::Paused);
            }
            EngineCommand::Stop => {
                self.set_state(PlaybackState::Stopped);
                self.ring_buffer.clear();
                self.samples_written = 0;
                *self.position.write() = 0.0;
                let _ = self.event_tx.send(EngineEvent::PositionUpdate(0.0));
            }
            EngineCommand::Seek(pos) => {
                self.seek_to(pos);
            }
            EngineCommand::SetVolume(vol) => {
                *self.volume.lock() = vol;
            }
            EngineCommand::LoadUrl(url, headers) => {
                self.load_url_internal(&url, headers.as_ref());
            }
            EngineCommand::LoadData(data, mime_hint) => {
                self.load_data(data, mime_hint.as_deref());
            }
            EngineCommand::Shutdown => {
                // Handled in the main loop
            }
        }
    }

    fn load_url_internal(&mut self, url: &str, headers: Option<&HashMap<String, String>>) {
        debug!("Loading URL: {url}");
        self.set_state(PlaybackState::Buffering);

        match self.fetch_url(url, headers) {
            Ok((data, mime_type)) => {
                self.load_data(data, mime_type.as_deref());
            }
            Err(e) => {
                error!("Failed to fetch URL: {e}");
                let _ = self
                    .event_tx
                    .send(EngineEvent::Error(format!("Failed to fetch: {e}")));
                self.set_state(PlaybackState::Stopped);
            }
        }
    }

    fn fetch_url(
        &self,
        url: &str,
        headers: Option<&HashMap<String, String>>,
    ) -> Result<(Vec<u8>, Option<String>)> {
        let mut request = ureq::get(url);

        // Use provided headers if available (from yt-dlp), otherwise use defaults
        if let Some(h) = headers {
            debug!("Using {} custom headers from yt-dlp", h.len());
            for (key, value) in h {
                request = request.header(key, value);
            }
        } else {
            // Default headers for YouTube
            request = request
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36")
                .header("Accept", "*/*")
                .header("Accept-Language", "en-US,en;q=0.9")
                .header("Origin", "https://www.youtube.com")
                .header("Referer", "https://www.youtube.com/");
        }

        let mut body = request
            .call()
            .map_err(|e| Error::Network(format!("HTTP request failed: {e}")))?
            .into_body();

        let mime_type = body.mime_type().map(String::from);
        let data = body
            .read_to_vec()
            .map_err(|e| Error::Network(format!("Failed to read response: {e}")))?;

        debug!("Fetched {} bytes, mime: {:?}", data.len(), mime_type);
        Ok((data, mime_type))
    }

    fn load_data(&mut self, data: Vec<u8>, mime_hint: Option<&str>) {
        debug!("Loading {} bytes of audio data", data.len());
        self.set_state(PlaybackState::Buffering);
        let _ = self.event_tx.send(EngineEvent::BufferingProgress(0.1));

        // Reset state
        self.ring_buffer.clear();
        self.samples_written = 0;
        self.decoder = None;
        *self.position.write() = 0.0;
        *self.duration.write() = None;

        // Create FFmpeg decoder (always outputs 48kHz stereo f32le)
        match FfmpegDecoder::from_bytes(data, mime_hint) {
            Ok(decoder) => {
                debug!(
                    "FFmpeg decoder created: {} Hz, {} channels",
                    decoder.sample_rate(),
                    decoder.channels()
                );

                // Get duration
                if let Some(dur) = decoder.duration() {
                    *self.duration.write() = Some(dur);
                    let _ = self.event_tx.send(EngineEvent::DurationUpdate(dur));
                    debug!("Track duration: {:.2} seconds", dur);
                }

                self.decoder = Some(decoder);

                // Pre-fill buffer before playback
                self.prefill_buffer();

                let _ = self.event_tx.send(EngineEvent::BufferingProgress(1.0));
                let _ = self.event_tx.send(EngineEvent::TrackLoaded);
                self.set_state(PlaybackState::Stopped);

                info!("Track loaded successfully");
            }
            Err(e) => {
                error!("Failed to create decoder: {e}");
                let _ = self
                    .event_tx
                    .send(EngineEvent::Error(format!("Failed to decode: {e}")));
                self.set_state(PlaybackState::Stopped);
            }
        }
    }

    fn prefill_buffer(&mut self) {
        debug!("Pre-filling buffer...");

        let target_fill = MIN_BUFFER_FILL * 2;
        let mut filled = 0;

        while filled < target_fill {
            if !self.decode_and_write() {
                break;
            }
            filled = self.ring_buffer.available();
        }

        debug!("Pre-filled {} samples", filled);
    }

    fn process_audio(&mut self) {
        // Keep the buffer reasonably full
        let free_space = self.ring_buffer.free();
        if free_space < 2048 {
            return;
        }

        // Decode and write samples
        if !self.decode_and_write() {
            // End of stream
            if self.ring_buffer.is_empty() {
                info!("Playback finished");
                self.set_state(PlaybackState::Stopped);
                let _ = self.event_tx.send(EngineEvent::PlaybackFinished);
            }
        }
    }

    fn decode_and_write(&mut self) -> bool {
        let Some(decoder) = &mut self.decoder else {
            return false;
        };

        match decoder.decode_next() {
            Ok(Some(samples)) => {
                // FFmpeg already outputs 48kHz stereo, write directly
                if !samples.is_empty() {
                    let written = self.ring_buffer.write(&samples);
                    self.samples_written += written as u64;
                    trace!("Wrote {} samples to ring buffer", written);
                }
                true
            }
            Ok(None) => {
                // End of stream
                false
            }
            Err(e) => {
                error!("Decode error: {e}");
                false
            }
        }
    }

    fn seek_to(&mut self, position_secs: f64) {
        debug!("Seeking to {:.2} seconds", position_secs);

        if let Some(decoder) = &mut self.decoder {
            // Clear buffer
            self.ring_buffer.clear();

            // Seek decoder
            if let Err(e) = decoder.seek(position_secs) {
                warn!("Seek failed: {e}");
                let _ = self
                    .event_tx
                    .send(EngineEvent::Error(format!("Seek failed: {e}")));
                return;
            }

            // Update position tracking (48kHz stereo = 2 samples per frame)
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let samples_at_position = (position_secs * 48000.0 * 2.0) as u64;
            self.samples_written = samples_at_position;

            *self.position.write() = position_secs;
            let _ = self
                .event_tx
                .send(EngineEvent::PositionUpdate(position_secs));

            // Pre-fill buffer after seek
            self.prefill_buffer();
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn update_position(&self) {
        // Calculate position from samples written and consumed
        let samples_in_buffer = self.ring_buffer.available() as u64;
        let samples_consumed = self.samples_written.saturating_sub(samples_in_buffer);

        // Convert to seconds (FFmpeg outputs 48kHz stereo = 96000 samples/sec)
        let position_secs = samples_consumed as f64 / (48000.0 * 2.0);

        *self.position.write() = position_secs;
        let _ = self
            .event_tx
            .send(EngineEvent::PositionUpdate(position_secs));
    }

    fn set_state(&self, new_state: PlaybackState) {
        let old_state = {
            let mut state = self.state.write();
            let old = *state;
            *state = new_state;
            old
        };

        if old_state != new_state {
            debug!("State changed: {:?} -> {:?}", old_state, new_state);
            let _ = self.event_tx.send(EngineEvent::StateChanged(new_state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playback_state_default() {
        assert_eq!(PlaybackState::default(), PlaybackState::Stopped);
    }

    // Note: Engine creation test requires audio hardware
    // and may fail in CI environments without audio devices
}
