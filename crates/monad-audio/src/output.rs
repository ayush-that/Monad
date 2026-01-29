//! Audio output using cpal.

use crate::buffer::SharedRingBuffer;
use crate::PlaybackState;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, Stream, StreamConfig,
};
use monad_core::{Error, Result};
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Audio output stream configuration.
#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: u32,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 1024,
        }
    }
}

/// Audio output stream wrapper.
pub struct AudioOutput {
    _stream: Stream,
    config: OutputConfig,
    device_name: String,
}

impl AudioOutput {
    /// Create a new audio output with the default device.
    pub fn new(
        ring_buffer: SharedRingBuffer,
        volume: Arc<Mutex<f32>>,
        state: Arc<RwLock<PlaybackState>>,
    ) -> Result<Self> {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .ok_or_else(|| Error::AudioOutput("No output device found".to_string()))?;

        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        info!("Using audio output device: {device_name}");

        Self::with_device(device, ring_buffer, volume, state)
    }

    /// Create a new audio output with a specific device.
    #[allow(clippy::needless_pass_by_value)] // Device is typically moved
    pub fn with_device(
        device: Device,
        ring_buffer: SharedRingBuffer,
        volume: Arc<Mutex<f32>>,
        state: Arc<RwLock<PlaybackState>>,
    ) -> Result<Self> {
        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());

        // Get supported config
        let supported_config = device
            .default_output_config()
            .map_err(|e| Error::AudioOutput(format!("Failed to get output config: {e}")))?;

        debug!("Supported output config: {:?}", supported_config);

        let sample_format = supported_config.sample_format();
        let config: StreamConfig = supported_config.into();

        let output_config = OutputConfig {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
            buffer_size: 1024,
        };

        debug!(
            "Output config: {}Hz, {} channels",
            output_config.sample_rate, output_config.channels
        );

        let stream = match sample_format {
            SampleFormat::F32 => {
                Self::build_stream::<f32>(&device, &config, ring_buffer, volume, state)?
            }
            SampleFormat::I16 => {
                Self::build_stream::<i16>(&device, &config, ring_buffer, volume, state)?
            }
            SampleFormat::U16 => {
                Self::build_stream::<u16>(&device, &config, ring_buffer, volume, state)?
            }
            _ => {
                return Err(Error::AudioOutput(format!(
                    "Unsupported sample format: {sample_format:?}"
                )));
            }
        };

        stream
            .play()
            .map_err(|e| Error::AudioOutput(format!("Failed to start stream: {e}")))?;

        Ok(Self {
            _stream: stream,
            config: output_config,
            device_name,
        })
    }

    fn build_stream<T: cpal::SizedSample + cpal::FromSample<f32>>(
        device: &Device,
        config: &StreamConfig,
        ring_buffer: SharedRingBuffer,
        volume: Arc<Mutex<f32>>,
        state: Arc<RwLock<PlaybackState>>,
    ) -> Result<Stream> {
        let _channels = usize::from(config.channels);

        let err_fn = |err| {
            error!("Audio stream error: {err}");
        };

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    // Check playback state - only output audio when playing
                    let current_state = *state.read();
                    if current_state != PlaybackState::Playing {
                        // Output silence when not playing
                        for sample in data.iter_mut() {
                            *sample = T::from_sample(0.0f32);
                        }
                        return;
                    }

                    let vol = *volume.lock();
                    let samples_needed = data.len();

                    // Read from ring buffer
                    let mut temp_buffer = vec![0.0f32; samples_needed];
                    let samples_read = ring_buffer.read(&mut temp_buffer);

                    // Convert and apply volume with soft limiting to prevent distortion
                    for (i, sample) in data.iter_mut().enumerate() {
                        if i < samples_read {
                            let s = temp_buffer[i] * vol;
                            // Soft clipping using tanh for smooth limiting
                            let limited = if s.abs() > 0.9 { s.tanh() } else { s };
                            *sample = T::from_sample(limited);
                        } else {
                            // Fill with silence if buffer underrun
                            *sample = T::from_sample(0.0f32);
                        }
                    }

                    if samples_read < samples_needed && samples_read > 0 {
                        warn!(
                            "Buffer underrun: needed {}, got {}",
                            samples_needed, samples_read
                        );
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| Error::AudioOutput(format!("Failed to build stream: {e}")))?;

        Ok(stream)
    }

    /// Get the output configuration.
    pub const fn config(&self) -> &OutputConfig {
        &self.config
    }

    /// Get the device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Get the sample rate.
    pub const fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    /// Get the number of channels.
    pub const fn channels(&self) -> u16 {
        self.config.channels
    }
}

/// List available output devices.
pub fn list_output_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();

    let devices: Vec<String> = host
        .output_devices()
        .map_err(|e| Error::AudioOutput(format!("Failed to list devices: {e}")))?
        .filter_map(|d| d.name().ok())
        .collect();

    Ok(devices)
}

/// Get the default output device name.
pub fn default_device_name() -> Option<String> {
    let host = cpal::default_host();
    host.default_output_device().and_then(|d| d.name().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        // This test may fail on CI without audio hardware
        let result = list_output_devices();
        // Just ensure it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_default_config() {
        let config = OutputConfig::default();
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
    }
}
