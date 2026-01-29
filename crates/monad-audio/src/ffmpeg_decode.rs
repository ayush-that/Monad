//! FFmpeg-based audio decoding for maximum compatibility and quality.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use monad_core::{Error, Result};
use tracing::{debug, info, warn};

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

/// Streaming FFmpeg decoder for decoding audio as it's being downloaded.
///
/// This decoder spawns ffmpeg with piped stdin/stdout, allowing audio to be
/// fed in chunks and decoded PCM to be read as it becomes available.
pub struct StreamingFfmpegDecoder {
    /// Sender for input audio data (compressed).
    input_tx: Option<Sender<Vec<u8>>>,
    /// Receiver for decoded PCM samples.
    output_rx: Receiver<Vec<f32>>,
    /// Flag indicating input has been closed.
    input_closed: Arc<AtomicBool>,
    /// Flag indicating decoding is complete.
    decode_complete: Arc<AtomicBool>,
    /// Total samples decoded so far.
    samples_decoded: u64,
    /// Handle to writer thread.
    writer_handle: Option<std::thread::JoinHandle<()>>,
    /// Handle to reader thread.
    reader_handle: Option<std::thread::JoinHandle<()>>,
}

impl StreamingFfmpegDecoder {
    /// Create a new streaming decoder.
    /// Spawns ffmpeg and starts writer/reader threads.
    pub fn new() -> Result<Self> {
        let ffmpeg_path = FfmpegDecoder::ffmpeg_path();

        if !ffmpeg_path.exists() {
            return Err(Error::AudioDecode(format!(
                "ffmpeg not found at {:?}",
                ffmpeg_path
            )));
        }

        info!("Creating streaming FFmpeg decoder");

        // Spawn ffmpeg with piped stdin/stdout
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

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::AudioDecode("Failed to capture ffmpeg stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::AudioDecode("Failed to capture ffmpeg stdout".to_string()))?;

        // Channel for input data (compressed audio)
        let (input_tx, input_rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = bounded(64);

        // Channel for output data (decoded PCM)
        let (output_tx, output_rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = bounded(128);

        let input_closed = Arc::new(AtomicBool::new(false));
        let decode_complete = Arc::new(AtomicBool::new(false));

        let input_closed_writer = input_closed.clone();
        let decode_complete_reader = decode_complete.clone();

        // Writer thread: receives compressed audio chunks and writes to ffmpeg stdin
        let writer_handle = std::thread::Builder::new()
            .name("ffmpeg-writer".to_string())
            .spawn(move || {
                Self::writer_thread(stdin, input_rx, input_closed_writer);
            })
            .map_err(|e| Error::AudioDecode(format!("Failed to spawn writer thread: {e}")))?;

        // Reader thread: reads decoded PCM from ffmpeg stdout
        let reader_handle = std::thread::Builder::new()
            .name("ffmpeg-reader".to_string())
            .spawn(move || {
                Self::reader_thread(stdout, output_tx, decode_complete_reader, child);
            })
            .map_err(|e| Error::AudioDecode(format!("Failed to spawn reader thread: {e}")))?;

        Ok(Self {
            input_tx: Some(input_tx),
            output_rx,
            input_closed,
            decode_complete,
            samples_decoded: 0,
            writer_handle: Some(writer_handle),
            reader_handle: Some(reader_handle),
        })
    }

    /// Writer thread function.
    fn writer_thread(
        mut stdin: ChildStdin,
        input_rx: Receiver<Vec<u8>>,
        input_closed: Arc<AtomicBool>,
    ) {
        debug!("FFmpeg writer thread started");
        let mut total_written = 0usize;

        loop {
            match input_rx.recv() {
                Ok(data) => {
                    if let Err(e) = stdin.write_all(&data) {
                        warn!("Error writing to ffmpeg stdin: {e}");
                        break;
                    }
                    total_written += data.len();
                }
                Err(_) => {
                    // Channel closed, input finished
                    break;
                }
            }
        }

        // Close stdin to signal EOF to ffmpeg
        drop(stdin);
        input_closed.store(true, Ordering::SeqCst);
        debug!(
            "FFmpeg writer thread finished, wrote {} bytes",
            total_written
        );
    }

    /// Reader thread function.
    fn reader_thread(
        mut stdout: std::process::ChildStdout,
        output_tx: Sender<Vec<f32>>,
        decode_complete: Arc<AtomicBool>,
        mut child: Child,
    ) {
        use std::io::Read;

        debug!("FFmpeg reader thread started");
        let mut buffer = vec![0u8; 32768]; // 32KB buffer
        let mut total_samples = 0u64;

        loop {
            match stdout.read(&mut buffer) {
                Ok(0) => {
                    // EOF - decoding complete
                    break;
                }
                Ok(n) => {
                    let samples = bytes_to_f32(&buffer[..n]);
                    total_samples += samples.len() as u64;

                    if output_tx.send(samples).is_err() {
                        // Receiver dropped
                        warn!("Output receiver dropped, stopping FFmpeg reader");
                        break;
                    }
                }
                Err(e) => {
                    warn!("Error reading from ffmpeg stdout: {e}");
                    break;
                }
            }
        }

        // Wait for ffmpeg to exit
        let _ = child.wait();
        decode_complete.store(true, Ordering::SeqCst);
        debug!(
            "FFmpeg reader thread finished, decoded {} samples",
            total_samples
        );
    }

    /// Feed compressed audio data to the decoder.
    pub fn feed(&self, data: Vec<u8>) -> Result<()> {
        if let Some(ref tx) = self.input_tx {
            tx.send(data)
                .map_err(|_| Error::AudioDecode("FFmpeg input channel closed".to_string()))
        } else {
            Err(Error::AudioDecode("Input already finished".to_string()))
        }
    }

    /// Signal that no more input data will be sent.
    /// This closes the input channel and allows ffmpeg to finish processing.
    pub fn finish_input(&mut self) {
        debug!("Finishing FFmpeg input");
        self.input_tx = None;
    }

    /// Try to read the next chunk of decoded PCM samples.
    /// Returns None if no data is currently available (non-blocking).
    pub fn try_decode_next(&mut self) -> Option<Vec<f32>> {
        match self.output_rx.try_recv() {
            Ok(samples) => {
                self.samples_decoded += samples.len() as u64;
                Some(samples)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Check if decoding is complete (all data processed).
    pub fn is_complete(&self) -> bool {
        self.decode_complete.load(Ordering::SeqCst) && self.output_rx.is_empty()
    }

    /// Check if input has been closed.
    pub fn is_input_closed(&self) -> bool {
        self.input_closed.load(Ordering::SeqCst)
    }

    /// Get the total number of samples decoded so far.
    pub const fn samples_decoded(&self) -> u64 {
        self.samples_decoded
    }
}

impl Drop for StreamingFfmpegDecoder {
    fn drop(&mut self) {
        // Close input channel to signal ffmpeg to finish
        self.input_tx = None;

        // Wait for threads to finish
        if let Some(handle) = self.writer_handle.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
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
