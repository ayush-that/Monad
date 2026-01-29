//! FFmpeg-based audio decoding for maximum compatibility and quality.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use monad_core::{Error, Result};
use tracing::{debug, info};

/// FFmpeg decoder that converts any audio format to raw PCM.
pub struct FfmpegDecoder {
    /// Raw PCM samples (f32, interleaved stereo)
    samples: Vec<f32>,
    /// Current read position
    position: usize,
    /// Sample rate (always 48000 after ffmpeg processing)
    sample_rate: u32,
    /// Number of channels (always 2 after ffmpeg processing)
    channels: u16,
    /// Total duration in seconds
    duration: Option<f64>,
}

impl FfmpegDecoder {
    /// Get the path to the ffmpeg binary.
    fn ffmpeg_path() -> PathBuf {
        directories::ProjectDirs::from("", "", "monad")
            .map(|d| d.cache_dir().join("ffmpeg"))
            .unwrap_or_else(|| PathBuf::from("ffmpeg"))
    }

    /// Create a new decoder from raw audio data.
    /// FFmpeg handles all format detection and decoding.
    #[allow(clippy::needless_pass_by_value)]
    pub fn from_bytes(data: Vec<u8>, _mime_hint: Option<&str>) -> Result<Self> {
        let ffmpeg_path = Self::ffmpeg_path();

        if !ffmpeg_path.exists() {
            return Err(Error::AudioDecode(format!(
                "ffmpeg not found at {:?}",
                ffmpeg_path
            )));
        }

        info!("Decoding {} bytes with ffmpeg", data.len());

        // Use ffmpeg to decode to raw f32le PCM at 48kHz stereo
        // -i pipe:0        = read from stdin
        // -f f32le         = output format: 32-bit float little-endian
        // -acodec pcm_f32le = PCM codec
        // -ar 48000        = sample rate 48kHz
        // -ac 2            = stereo
        // -v quiet         = suppress output
        // pipe:1           = write to stdout
        let mut child = Command::new(&ffmpeg_path)
            .args([
                "-i",
                "pipe:0",
                "-f",
                "f32le",
                "-acodec",
                "pcm_f32le",
                "-ar",
                "48000",
                "-ac",
                "2",
                "-v",
                "quiet",
                "pipe:1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::AudioDecode(format!("Failed to spawn ffmpeg: {e}")))?;

        // Write input data to ffmpeg's stdin in a separate thread to avoid deadlock
        // (ffmpeg may block on stdout while we're still writing to stdin)
        let stdin = child.stdin.take();
        let write_thread = std::thread::spawn(move || {
            if let Some(mut stdin) = stdin {
                let _ = stdin.write_all(&data);
                // stdin is dropped here, closing the pipe
            }
        });

        // Read decoded PCM from stdout
        let output = child
            .wait_with_output()
            .map_err(|e| Error::AudioDecode(format!("Failed to read ffmpeg output: {e}")))?;

        // Wait for write thread to finish
        let _ = write_thread.join();

        if !output.status.success() {
            return Err(Error::AudioDecode("ffmpeg decoding failed".to_string()));
        }

        let pcm_bytes = output.stdout;
        if pcm_bytes.is_empty() {
            return Err(Error::AudioDecode("ffmpeg produced no output".to_string()));
        }

        // Convert bytes to f32 samples
        let samples = bytes_to_f32(&pcm_bytes);

        // Calculate duration: samples / (sample_rate * channels)
        #[allow(clippy::cast_precision_loss)]
        let duration = Some(samples.len() as f64 / (48000.0 * 2.0));

        info!(
            "Decoded {} samples ({:.2}s) with ffmpeg",
            samples.len(),
            duration.unwrap_or(0.0)
        );

        Ok(Self {
            samples,
            position: 0,
            sample_rate: 48000,
            channels: 2,
            duration,
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

    /// Get the total duration in seconds.
    pub const fn duration(&self) -> Option<f64> {
        self.duration
    }

    /// Decode the next chunk of samples.
    /// Returns None when all samples have been read.
    pub fn decode_next(&mut self) -> Result<Option<Vec<f32>>> {
        if self.position >= self.samples.len() {
            return Ok(None);
        }

        // Return chunks of ~1024 frames (2048 samples for stereo)
        let chunk_size = 2048;
        let end = (self.position + chunk_size).min(self.samples.len());
        let chunk = self.samples[self.position..end].to_vec();
        self.position = end;

        Ok(Some(chunk))
    }

    /// Seek to a position in seconds.
    pub fn seek(&mut self, position_secs: f64) -> Result<()> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sample_position = (position_secs * 48000.0 * 2.0) as usize;

        // Align to frame boundary (stereo = 2 samples per frame)
        let aligned = (sample_position / 2) * 2;
        self.position = aligned.min(self.samples.len());

        debug!(
            "Seeked to position {} (sample {})",
            position_secs, self.position
        );
        Ok(())
    }

    /// Reset the decoder to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

/// Convert raw bytes (f32le) to f32 samples.
fn bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
            f32::from_le_bytes(arr)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_f32() {
        // 0.5 as f32 little-endian
        let bytes = 0.5f32.to_le_bytes();
        let samples = bytes_to_f32(&bytes);
        assert_eq!(samples.len(), 1);
        assert!((samples[0] - 0.5).abs() < 0.0001);
    }
}
