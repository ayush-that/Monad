//! Backend services integration.
//!
//! This module connects the UI to the backend services:
//! - Audio engine for playback
//! - Stream extractor for getting playable URLs

pub mod audio;

pub use audio::AudioService;
