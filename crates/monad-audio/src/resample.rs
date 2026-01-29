//! Audio resampling using rubato.

#![allow(clippy::unwrap_used)] // Tests use unwrap for brevity

use monad_core::{Error, Result};
use rubato::{FftFixedIn, Resampler as RubatoResampler};
use tracing::debug;

/// Audio resampler for matching device sample rate.
pub struct Resampler {
    #[allow(clippy::struct_field_names)]
    resampler: FftFixedIn<f32>,
    input_rate: u32,
    output_rate: u32,
    channels: usize,
    chunk_size: usize,
    /// Buffer for deinterleaved input.
    input_buffer: Vec<Vec<f32>>,
    /// Buffer for deinterleaved output.
    #[allow(dead_code)] // Will be used for streaming resampling
    output_buffer: Vec<Vec<f32>>,
}

impl Resampler {
    /// Create a new resampler.
    pub fn new(input_rate: u32, output_rate: u32, channels: usize) -> Result<Self> {
        if input_rate == output_rate {
            // No resampling needed, but we still create the structure
            return Ok(Self {
                resampler: FftFixedIn::new(
                    input_rate as usize,
                    output_rate as usize,
                    1024,
                    2,
                    channels,
                )
                .map_err(|e| Error::AudioOutput(format!("Failed to create resampler: {e}")))?,
                input_rate,
                output_rate,
                channels,
                chunk_size: 1024,
                input_buffer: vec![Vec::new(); channels],
                output_buffer: vec![Vec::new(); channels],
            });
        }

        let chunk_size = 1024;

        let resampler = FftFixedIn::new(
            input_rate as usize,
            output_rate as usize,
            chunk_size,
            2,
            channels,
        )
        .map_err(|e| Error::AudioOutput(format!("Failed to create resampler: {e}")))?;

        debug!(
            "Resampler created: {}Hz -> {}Hz, {} channels",
            input_rate, output_rate, channels
        );

        Ok(Self {
            resampler,
            input_rate,
            output_rate,
            channels,
            chunk_size,
            input_buffer: vec![Vec::new(); channels],
            output_buffer: vec![Vec::new(); channels],
        })
    }

    /// Check if resampling is needed.
    pub const fn needs_resampling(&self) -> bool {
        self.input_rate != self.output_rate
    }

    /// Get the input sample rate.
    pub const fn input_rate(&self) -> u32 {
        self.input_rate
    }

    /// Get the output sample rate.
    pub const fn output_rate(&self) -> u32 {
        self.output_rate
    }

    /// Get the number of channels.
    pub const fn channels(&self) -> usize {
        self.channels
    }

    /// Process interleaved samples and return resampled interleaved samples.
    pub fn process(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        if !self.needs_resampling() {
            return Ok(input.to_vec());
        }

        // Deinterleave input
        self.deinterleave(input);

        // Ensure we have enough samples for processing
        let frames_available = self.input_buffer[0].len();
        if frames_available < self.chunk_size {
            // Not enough samples, return empty
            return Ok(Vec::new());
        }

        // Process in chunks
        let mut all_output = Vec::new();

        while self.input_buffer[0].len() >= self.chunk_size {
            // Take a chunk from each channel
            let chunk: Vec<Vec<f32>> = self
                .input_buffer
                .iter_mut()
                .map(|ch| ch.drain(..self.chunk_size).collect())
                .collect();

            // Resample
            let resampled = self
                .resampler
                .process(&chunk, None)
                .map_err(|e| Error::AudioOutput(format!("Resample failed: {e}")))?;

            // Interleave output
            let interleaved = self.interleave(&resampled);
            all_output.extend(interleaved);
        }

        Ok(all_output)
    }

    /// Process all remaining samples (for end of stream).
    pub fn flush(&mut self) -> Result<Vec<f32>> {
        if !self.needs_resampling() {
            // Return any buffered samples
            let remaining: Vec<f32> = self.interleave_buffers();
            self.input_buffer.iter_mut().for_each(Vec::clear);
            return Ok(remaining);
        }

        let mut all_output = Vec::new();

        // Process any remaining complete chunks
        while self.input_buffer[0].len() >= self.chunk_size {
            let chunk: Vec<Vec<f32>> = self
                .input_buffer
                .iter_mut()
                .map(|ch| ch.drain(..self.chunk_size).collect())
                .collect();

            let resampled = self
                .resampler
                .process(&chunk, None)
                .map_err(|e| Error::AudioOutput(format!("Resample failed: {e}")))?;

            let interleaved = self.interleave(&resampled);
            all_output.extend(interleaved);
        }

        // Pad remaining samples and process
        if !self.input_buffer[0].is_empty() {
            let remaining = self.input_buffer[0].len();
            let padding = self.chunk_size - remaining;

            let chunk: Vec<Vec<f32>> = self
                .input_buffer
                .iter_mut()
                .map(|ch| {
                    let mut data: Vec<f32> = std::mem::take(ch);
                    data.extend(std::iter::repeat_n(0.0, padding));
                    data
                })
                .collect();

            let resampled = self
                .resampler
                .process(&chunk, None)
                .map_err(|e| Error::AudioOutput(format!("Resample failed: {e}")))?;

            // Only take the non-padded portion
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let output_frames = (remaining as f64 * f64::from(self.output_rate)
                / f64::from(self.input_rate)) as usize;
            let interleaved = self.interleave_partial(&resampled, output_frames);
            all_output.extend(interleaved);
        }

        Ok(all_output)
    }

    /// Reset the resampler state.
    pub fn reset(&mut self) {
        self.resampler.reset();
        self.input_buffer.iter_mut().for_each(Vec::clear);
        self.output_buffer.iter_mut().for_each(Vec::clear);
    }

    /// Deinterleave input samples into channel buffers.
    fn deinterleave(&mut self, input: &[f32]) {
        let frames = input.len() / self.channels;

        for frame in 0..frames {
            for (ch, buffer) in self.input_buffer.iter_mut().enumerate() {
                buffer.push(input[frame * self.channels + ch]);
            }
        }
    }

    /// Interleave channel buffers into output.
    fn interleave(&self, channels: &[Vec<f32>]) -> Vec<f32> {
        if channels.is_empty() || channels[0].is_empty() {
            return Vec::new();
        }

        let frames = channels[0].len();
        let mut output = Vec::with_capacity(frames * self.channels);

        for frame in 0..frames {
            for ch in channels {
                output.push(ch[frame]);
            }
        }

        output
    }

    /// Interleave only a partial output (for flushing).
    fn interleave_partial(&self, channels: &[Vec<f32>], frames: usize) -> Vec<f32> {
        if channels.is_empty() {
            return Vec::new();
        }

        let frames = frames.min(channels[0].len());
        let mut output = Vec::with_capacity(frames * self.channels);

        for frame in 0..frames {
            for ch in channels {
                output.push(ch[frame]);
            }
        }

        output
    }

    /// Interleave internal buffers.
    fn interleave_buffers(&self) -> Vec<f32> {
        if self.input_buffer.is_empty() || self.input_buffer[0].is_empty() {
            return Vec::new();
        }

        let frames = self.input_buffer[0].len();
        let mut output = Vec::with_capacity(frames * self.channels);

        for frame in 0..frames {
            for ch in &self.input_buffer {
                output.push(ch[frame]);
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_resampling() {
        let mut resampler = Resampler::new(48000, 48000, 2).unwrap();
        assert!(!resampler.needs_resampling());

        let input = vec![0.5f32; 2048];
        let output = resampler.process(&input).unwrap();
        assert_eq!(output.len(), input.len());
    }

    #[test]
    fn test_resampler_creation() {
        let resampler = Resampler::new(48000, 44100, 2).unwrap();
        assert!(resampler.needs_resampling());
        assert_eq!(resampler.input_rate(), 48000);
        assert_eq!(resampler.output_rate(), 44100);
        assert_eq!(resampler.channels(), 2);
    }
}
