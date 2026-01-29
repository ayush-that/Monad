//! Audio decoding using symphonia.

use std::io::Cursor;

use bytes::Bytes;
use monad_core::{Error, Result};
use symphonia::core::{
    audio::{AudioBufferRef, Signal},
    codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL},
    formats::{FormatOptions, FormatReader},
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
    probe::Hint,
};
use tracing::{debug, error, trace};

/// Audio decoder wrapping symphonia.
pub struct AudioDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
}

impl AudioDecoder {
    /// Create a new decoder from a byte buffer.
    #[allow(clippy::needless_pass_by_value)] // Bytes is cheaply cloneable
    pub fn from_bytes(data: Bytes, mime_hint: Option<&str>) -> Result<Self> {
        let cursor = Cursor::new(data.to_vec());
        let mss = MediaSourceStream::new(Box::new(cursor), MediaSourceStreamOptions::default());

        let mut hint = Hint::new();
        if let Some(mime) = mime_hint {
            if mime.contains("webm") || mime.contains("opus") {
                hint.with_extension("webm");
            } else if mime.contains("mp4") || mime.contains("m4a") || mime.contains("aac") {
                hint.with_extension("m4a");
            } else if mime.contains("mp3") || mime.contains("mpeg") {
                hint.with_extension("mp3");
            } else if mime.contains("ogg") || mime.contains("vorbis") {
                hint.with_extension("ogg");
            } else if mime.contains("flac") {
                hint.with_extension("flac");
            }
        }

        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| Error::AudioDecode(format!("Failed to probe format: {e}")))?;

        let format = probed.format;

        // Find the first audio track
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| Error::AudioDecode("No audio tracks found".to_string()))?;

        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(48000);
        #[allow(clippy::cast_possible_truncation)]
        let channels = track.codec_params.channels.map_or(2, |c| c.count() as u16);

        debug!(
            "Audio track: id={}, sample_rate={}, channels={}",
            track_id, sample_rate, channels
        );

        let decoder_opts = DecoderOptions::default();
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .map_err(|e| Error::AudioDecode(format!("Failed to create decoder: {e}")))?;

        Ok(Self {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
        })
    }

    /// Get the sample rate.
    pub const fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get the number of channels.
    pub const fn channels(&self) -> u16 {
        self.channels
    }

    /// Decode the next packet and return interleaved f32 samples.
    pub fn decode_next(&mut self) -> Result<Option<Vec<f32>>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None); // End of stream
                }
                Err(e) => {
                    return Err(Error::AudioDecode(format!("Failed to read packet: {e}")));
                }
            };

            // Skip packets for other tracks
            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let samples = audio_buffer_to_f32(&decoded);
                    return Ok(Some(samples));
                }
                Err(symphonia::core::errors::Error::DecodeError(e)) => {
                    // Log and skip corrupt frames
                    error!("Decode error (skipping): {e}");
                }
                Err(e) => {
                    return Err(Error::AudioDecode(format!("Decode failed: {e}")));
                }
            }
        }
    }
}

/// Convert an `AudioBuffer` to interleaved f32 samples.
#[allow(clippy::cast_possible_truncation)]
fn audio_buffer_to_f32(buffer: &AudioBufferRef<'_>) -> Vec<f32> {
    match buffer {
        AudioBufferRef::F32(buf) => interleave_planes(buf.planes()),
        AudioBufferRef::F64(buf) => {
            let planes = buf.planes();
            let frames = buf.frames();
            let channels = planes.planes().len();
            let mut output = Vec::with_capacity(frames * channels);
            for frame in 0..frames {
                for plane in planes.planes() {
                    output.push(plane[frame] as f32);
                }
            }
            output
        }
        AudioBufferRef::S32(buf) => {
            let planes = buf.planes();
            let frames = buf.frames();
            let channels = planes.planes().len();
            let mut output = Vec::with_capacity(frames * channels);
            for frame in 0..frames {
                for plane in planes.planes() {
                    #[allow(clippy::cast_precision_loss)]
                    output.push(plane[frame] as f32 / i32::MAX as f32);
                }
            }
            output
        }
        AudioBufferRef::S16(buf) => {
            let planes = buf.planes();
            let frames = buf.frames();
            let channels = planes.planes().len();
            let mut output = Vec::with_capacity(frames * channels);
            for frame in 0..frames {
                for plane in planes.planes() {
                    output.push(f32::from(plane[frame]) / f32::from(i16::MAX));
                }
            }
            output
        }
        AudioBufferRef::U8(buf) => {
            let planes = buf.planes();
            let frames = buf.frames();
            let channels = planes.planes().len();
            let mut output = Vec::with_capacity(frames * channels);
            for frame in 0..frames {
                for plane in planes.planes() {
                    output.push((f32::from(plane[frame]) - 128.0) / 128.0);
                }
            }
            output
        }
        _ => Vec::new(),
    }
}

fn interleave_planes(planes: symphonia::core::audio::AudioPlanes<'_, f32>) -> Vec<f32> {
    let channel_planes = planes.planes();
    if channel_planes.is_empty() {
        return Vec::new();
    }

    let frames = channel_planes[0].len();
    let channels = channel_planes.len();
    let mut output = Vec::with_capacity(frames * channels);

    for frame in 0..frames {
        for plane in channel_planes {
            output.push(plane[frame]);
        }
    }

    output
}

/// Streaming decoder that can handle data as it arrives.
pub struct StreamingDecoder {
    buffer: Vec<u8>,
    decoder: Option<AudioDecoder>,
    mime_hint: Option<String>,
    ready: bool,
}

impl StreamingDecoder {
    /// Create a new streaming decoder.
    pub fn new(mime_hint: Option<&str>) -> Self {
        Self {
            buffer: Vec::new(),
            decoder: None,
            mime_hint: mime_hint.map(String::from),
            ready: false,
        }
    }

    /// Feed data to the decoder.
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);

        // Try to initialize decoder if we have enough data
        if !self.ready && self.buffer.len() > 8192 {
            self.try_init();
        }
    }

    /// Mark the stream as complete and initialize decoder.
    pub fn finish(&mut self) -> Result<()> {
        if !self.ready {
            self.try_init();
        }

        if !self.ready {
            return Err(Error::AudioDecode(
                "Failed to initialize decoder".to_string(),
            ));
        }

        Ok(())
    }

    fn try_init(&mut self) {
        let bytes = Bytes::from(self.buffer.clone());
        match AudioDecoder::from_bytes(bytes, self.mime_hint.as_deref()) {
            Ok(decoder) => {
                self.decoder = Some(decoder);
                self.ready = true;
                debug!("Streaming decoder initialized");
            }
            Err(e) => {
                trace!("Decoder init failed (may need more data): {e}");
            }
        }
    }

    /// Check if the decoder is ready.
    pub const fn is_ready(&self) -> bool {
        self.ready
    }

    /// Get the decoder (if ready).
    pub const fn decoder(&mut self) -> Option<&mut AudioDecoder> {
        self.decoder.as_mut()
    }

    /// Get the sample rate.
    pub fn sample_rate(&self) -> Option<u32> {
        self.decoder.as_ref().map(AudioDecoder::sample_rate)
    }

    /// Get the number of channels.
    pub fn channels(&self) -> Option<u16> {
        self.decoder.as_ref().map(AudioDecoder::channels)
    }
}

impl AudioDecoder {
    /// Seek to a position in seconds.
    pub fn seek(&mut self, position_secs: f64) -> Result<()> {
        use symphonia::core::formats::SeekMode;
        use symphonia::core::units::Time;

        let time = Time::from(position_secs);

        self.format
            .seek(
                SeekMode::Accurate,
                symphonia::core::formats::SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| Error::AudioDecode(format!("Seek failed: {e}")))?;

        // Reset decoder state
        self.decoder.reset();

        Ok(())
    }

    /// Get the total duration in seconds (if known).
    pub fn duration(&self) -> Option<f64> {
        let track = self
            .format
            .tracks()
            .iter()
            .find(|t| t.id == self.track_id)?;

        let time_base = track.codec_params.time_base?;
        let n_frames = track.codec_params.n_frames?;

        #[allow(clippy::cast_precision_loss)]
        Some(time_base.calc_time(n_frames).seconds as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_decoder_creation() {
        let decoder = StreamingDecoder::new(Some("audio/webm"));
        assert!(!decoder.is_ready());
    }
}
