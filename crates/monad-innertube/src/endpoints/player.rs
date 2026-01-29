//! Player endpoint implementation for stream URL extraction.

use monad_core::{
    types::{AudioFormat, AudioQuality, StreamInfo},
    Error, Result, StreamCollection,
};

use crate::{
    types::{Format, InnerTubeRequest, PlayerPayload, RawPlayerResponse},
    InnerTubeClient,
};

impl InnerTubeClient {
    /// Get stream information for a video.
    ///
    /// # Arguments
    /// * `video_id` - The `YouTube` video ID
    ///
    /// # Returns
    /// Collection of available audio streams.
    pub async fn get_streams(&self, video_id: &str) -> Result<StreamCollection> {
        let payload = PlayerPayload {
            video_id: video_id.to_string(),
            playlist_id: None,
            content_check_ok: Some(true),
            racy_check_ok: Some(true),
        };

        let request = InnerTubeRequest::new(self.context.clone(), payload);

        let response: RawPlayerResponse = self
            .post("player", &request)
            .await
            .map_err(|e| Error::InnerTube(format!("Player request failed: {e}")))?;

        // Check playability
        if let Some(status) = &response.playability_status {
            if status.status != "OK" {
                let reason = status.reason.as_deref().unwrap_or("Unknown error");
                return Err(Error::ContentNotAvailable(reason.to_string()));
            }
        }

        // Extract streams
        let streams = parse_streams(&response)?;

        if streams.is_empty() {
            return Err(Error::ContentNotAvailable(
                "No audio streams available".to_string(),
            ));
        }

        Ok(StreamCollection::new(streams))
    }

    /// Get the player response for a video (raw data).
    pub async fn get_player_response(&self, video_id: &str) -> Result<RawPlayerResponse> {
        let payload = PlayerPayload {
            video_id: video_id.to_string(),
            playlist_id: None,
            content_check_ok: Some(true),
            racy_check_ok: Some(true),
        };

        let request = InnerTubeRequest::new(self.context.clone(), payload);

        self.post("player", &request)
            .await
            .map_err(|e| Error::InnerTube(format!("Player request failed: {e}")))
    }
}

fn parse_streams(response: &RawPlayerResponse) -> Result<Vec<StreamInfo>> {
    let streaming_data = response
        .streaming_data
        .as_ref()
        .ok_or_else(|| Error::ContentNotAvailable("No streaming data".to_string()))?;

    let expires_in: u64 = streaming_data
        .expires_in_seconds
        .as_ref()
        .and_then(|s| s.parse().ok())
        .unwrap_or(21600); // Default 6 hours

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let expires_at = now + expires_in;

    let mut streams = Vec::new();

    // Process adaptive formats (audio-only streams)
    if let Some(formats) = &streaming_data.adaptive_formats {
        for format in formats {
            if format.is_audio_only() {
                if let Some(stream) = parse_format(format, expires_at) {
                    streams.push(stream);
                }
            }
        }
    }

    // Process combined formats as fallback
    if let Some(formats) = &streaming_data.formats {
        for format in formats {
            // Include combined formats only if we have few audio-only options
            if streams.len() < 2 {
                if let Some(stream) = parse_format(format, expires_at) {
                    streams.push(stream);
                }
            }
        }
    }

    // Sort by quality (best first)
    streams.sort_by(|a, b| b.quality_score().cmp(&a.quality_score()));

    Ok(streams)
}

fn parse_format(format: &Format, expires_at: u64) -> Option<StreamInfo> {
    // Need either URL or signature cipher
    let url = format.url.clone().or({
        // If no direct URL, we need signature decryption (handled elsewhere)
        None
    })?;

    let audio_format = AudioFormat::from_mime(&format.mime_type);
    let quality = format.bitrate.map_or(AudioQuality::Medium, |b| {
        AudioQuality::from_bitrate(b / 1000)
    });

    let mut stream = StreamInfo::new(url, audio_format, quality);

    stream.bitrate = format.bitrate.map(|b| b / 1000);
    stream.sample_rate = format.sample_rate_u32();
    stream.channels = format.audio_channels;
    stream.content_length = format.content_length_u64();
    stream.mime_type = Some(format.mime_type.clone());
    stream.signature_cipher = format.signature_cipher.clone().or(format.cipher.clone());
    stream.expires_at = Some(expires_at);

    Some(stream)
}

/// Known audio itags and their properties.
#[derive(Debug, Clone, Copy)]
pub struct ItagInfo {
    pub itag: u32,
    pub format: AudioFormat,
    pub quality: AudioQuality,
    pub bitrate: u32,
    pub sample_rate: u32,
}

impl ItagInfo {
    /// Get info for a known itag.
    pub fn from_itag(itag: u32) -> Option<Self> {
        KNOWN_AUDIO_ITAGS.iter().find(|i| i.itag == itag).copied()
    }
}

/// Known audio-only itags from `YouTube`.
pub const KNOWN_AUDIO_ITAGS: &[ItagInfo] = &[
    // Opus
    ItagInfo {
        itag: 249,
        format: AudioFormat::Opus,
        quality: AudioQuality::Low,
        bitrate: 50,
        sample_rate: 48000,
    },
    ItagInfo {
        itag: 250,
        format: AudioFormat::Opus,
        quality: AudioQuality::Low,
        bitrate: 70,
        sample_rate: 48000,
    },
    ItagInfo {
        itag: 251,
        format: AudioFormat::Opus,
        quality: AudioQuality::High,
        bitrate: 160,
        sample_rate: 48000,
    },
    // AAC
    ItagInfo {
        itag: 139,
        format: AudioFormat::Aac,
        quality: AudioQuality::Low,
        bitrate: 48,
        sample_rate: 22050,
    },
    ItagInfo {
        itag: 140,
        format: AudioFormat::Aac,
        quality: AudioQuality::Medium,
        bitrate: 128,
        sample_rate: 44100,
    },
    ItagInfo {
        itag: 141,
        format: AudioFormat::Aac,
        quality: AudioQuality::High,
        bitrate: 256,
        sample_rate: 44100,
    },
    // MP3 (rare)
    ItagInfo {
        itag: 17,
        format: AudioFormat::Mp3,
        quality: AudioQuality::Low,
        bitrate: 64,
        sample_rate: 22050,
    },
];
