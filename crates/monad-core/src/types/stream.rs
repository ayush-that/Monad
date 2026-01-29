//! Stream and audio format types.

#![allow(clippy::unwrap_used)] // Tests use unwrap for brevity

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Information about an audio stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamInfo {
    /// The stream URL.
    pub url: String,
    /// Audio format/codec.
    pub format: AudioFormat,
    /// Audio quality level.
    pub quality: AudioQuality,
    /// Bitrate in kbps (if known).
    pub bitrate: Option<u32>,
    /// Sample rate in Hz (if known).
    pub sample_rate: Option<u32>,
    /// Number of audio channels (if known).
    pub channels: Option<u8>,
    /// Content length in bytes (if known).
    pub content_length: Option<u64>,
    /// MIME type.
    pub mime_type: Option<String>,
    /// Whether this stream requires signature decryption.
    pub signature_cipher: Option<String>,
    /// Expiry time (Unix timestamp).
    pub expires_at: Option<u64>,
    /// HTTP headers required to fetch this stream (from yt-dlp).
    #[serde(default)]
    pub http_headers: Option<HashMap<String, String>>,
}

impl StreamInfo {
    pub fn new(url: impl Into<String>, format: AudioFormat, quality: AudioQuality) -> Self {
        Self {
            url: url.into(),
            format,
            quality,
            bitrate: None,
            sample_rate: None,
            channels: None,
            content_length: None,
            mime_type: None,
            signature_cipher: None,
            expires_at: None,
            http_headers: None,
        }
    }

    /// Check if this stream URL has expired.
    pub fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|expires_at| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            now >= expires_at
        })
    }

    /// Get a quality score for sorting (higher is better).
    pub const fn quality_score(&self) -> u32 {
        let format_score = self.format.quality_score();
        let quality_score = self.quality.bitrate_estimate();
        format_score * 1000 + quality_score
    }
}

/// Audio codec/format.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AudioFormat {
    /// Opus codec (best quality/size ratio).
    Opus,
    /// AAC codec.
    Aac,
    /// MP3 codec.
    Mp3,
    /// FLAC codec (lossless).
    Flac,
    /// Vorbis codec.
    Vorbis,
    /// `WebM` container with audio.
    WebM,
    /// MP4/M4A container.
    M4a,
    /// Unknown format.
    #[default]
    Unknown,
}

impl AudioFormat {
    /// Parse from MIME type or format string.
    pub fn from_mime(mime: &str) -> Self {
        let mime_lower = mime.to_lowercase();

        if mime_lower.contains("opus") {
            Self::Opus
        } else if mime_lower.contains("aac") || mime_lower.contains("mp4a") {
            Self::Aac
        } else if mime_lower.contains("mp3") || mime_lower.contains("mpeg") {
            Self::Mp3
        } else if mime_lower.contains("flac") {
            Self::Flac
        } else if mime_lower.contains("vorbis") {
            Self::Vorbis
        } else if mime_lower.contains("webm") {
            Self::WebM
        } else if mime_lower.contains("m4a") {
            Self::M4a
        } else {
            Self::Unknown
        }
    }

    /// Get the file extension for this format.
    pub const fn extension(&self) -> &'static str {
        match self {
            Self::Opus => "opus",
            Self::Aac | Self::M4a => "m4a",
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Vorbis => "ogg",
            Self::WebM => "webm",
            Self::Unknown => "audio",
        }
    }

    /// Quality score for sorting (higher = better codec efficiency).
    pub const fn quality_score(&self) -> u32 {
        match self {
            Self::Opus => 100,
            Self::Flac => 95,
            Self::Aac | Self::M4a => 80,
            Self::Vorbis => 75,
            Self::Mp3 => 70,
            Self::WebM => 60,
            Self::Unknown => 0,
        }
    }

    /// Whether this is a lossless format.
    pub const fn is_lossless(&self) -> bool {
        matches!(self, Self::Flac)
    }
}

/// Audio quality level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum AudioQuality {
    /// Low quality (~64 kbps).
    Low,
    /// Medium quality (~128 kbps).
    Medium,
    /// High quality (~256 kbps).
    #[default]
    High,
    /// Maximum quality (320+ kbps or lossless).
    Max,
}

impl AudioQuality {
    /// Parse from itag or bitrate.
    pub const fn from_bitrate(bitrate: u32) -> Self {
        match bitrate {
            0..=80 => Self::Low,
            81..=160 => Self::Medium,
            161..=280 => Self::High,
            _ => Self::Max,
        }
    }

    /// Estimated bitrate for this quality level.
    pub const fn bitrate_estimate(&self) -> u32 {
        match self {
            Self::Low => 64,
            Self::Medium => 128,
            Self::High => 256,
            Self::Max => 320,
        }
    }
}

/// Represents a collection of available streams for a track.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamCollection {
    pub streams: Vec<StreamInfo>,
}

impl StreamCollection {
    pub const fn new(streams: Vec<StreamInfo>) -> Self {
        Self { streams }
    }

    /// Get the best quality stream.
    pub fn best(&self) -> Option<&StreamInfo> {
        self.streams.iter().max_by_key(|s| s.quality_score())
    }

    /// Get the best stream matching the preferred format.
    pub fn best_for_format(&self, format: AudioFormat) -> Option<&StreamInfo> {
        self.streams
            .iter()
            .filter(|s| s.format == format)
            .max_by_key(|s| s.quality_score())
    }

    /// Get the best stream at or below the target quality.
    pub fn best_for_quality(&self, max_quality: AudioQuality) -> Option<&StreamInfo> {
        self.streams
            .iter()
            .filter(|s| s.quality <= max_quality)
            .max_by_key(|s| s.quality_score())
    }

    /// Get streams sorted by quality (best first).
    pub fn sorted_by_quality(&self) -> Vec<&StreamInfo> {
        let mut streams: Vec<_> = self.streams.iter().collect();
        streams.sort_by_key(|s| std::cmp::Reverse(s.quality_score()));
        streams
    }

    pub const fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_from_mime() {
        assert_eq!(
            AudioFormat::from_mime("audio/webm; codecs=\"opus\""),
            AudioFormat::Opus
        );
        assert_eq!(
            AudioFormat::from_mime("audio/mp4; codecs=\"mp4a.40.2\""),
            AudioFormat::Aac
        );
        assert_eq!(AudioFormat::from_mime("audio/mpeg"), AudioFormat::Mp3);
    }

    #[test]
    fn test_stream_quality_score() {
        let opus_high = StreamInfo::new("url1", AudioFormat::Opus, AudioQuality::High);
        let aac_high = StreamInfo::new("url2", AudioFormat::Aac, AudioQuality::High);
        let mp3_max = StreamInfo::new("url3", AudioFormat::Mp3, AudioQuality::Max);

        assert!(opus_high.quality_score() > aac_high.quality_score());
        assert!(opus_high.quality_score() > mp3_max.quality_score());
    }

    #[test]
    fn test_stream_collection_best() {
        let collection = StreamCollection::new(vec![
            StreamInfo::new("url1", AudioFormat::Mp3, AudioQuality::Medium),
            StreamInfo::new("url2", AudioFormat::Opus, AudioQuality::High),
            StreamInfo::new("url3", AudioFormat::Aac, AudioQuality::Low),
        ]);

        let best = collection.best().unwrap();
        assert_eq!(best.format, AudioFormat::Opus);
    }
}
