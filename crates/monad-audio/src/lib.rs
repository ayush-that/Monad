//! # monad-audio
//!
//! High-performance audio playback engine for Monad.
//!
//! Features:
//! - Lock-free ring buffer for decode→output communication
//! - FFmpeg-based decoding for maximum compatibility
//! - Low-latency cpal output

pub mod buffer;
pub mod decode;
pub mod engine;
pub mod ffmpeg_decode;
pub mod output;
pub mod resample;

pub use engine::{AudioEngine, EngineCommand, EngineEvent, PlaybackState};
