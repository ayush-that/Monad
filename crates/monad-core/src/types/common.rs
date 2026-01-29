//! Common types shared across the application.

#![allow(clippy::unwrap_used)] // Tests use unwrap for brevity

use serde::{Deserialize, Serialize};

/// Thumbnail image with URL and dimensions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Thumbnail {
    pub url: String,
    pub width: u32,
    pub height: u32,
}

impl Thumbnail {
    pub fn new(url: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            url: url.into(),
            width,
            height,
        }
    }
}

/// Collection of thumbnails at different resolutions.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Thumbnails(pub Vec<Thumbnail>);

impl Thumbnails {
    pub const fn new(thumbnails: Vec<Thumbnail>) -> Self {
        Self(thumbnails)
    }

    /// Get the best quality thumbnail (largest).
    pub fn best(&self) -> Option<&Thumbnail> {
        self.0.iter().max_by_key(|t| t.width * t.height)
    }

    /// Get the smallest thumbnail.
    pub fn smallest(&self) -> Option<&Thumbnail> {
        self.0.iter().min_by_key(|t| t.width * t.height)
    }

    /// Get a thumbnail closest to the target size.
    pub fn closest_to(&self, target_width: u32, target_height: u32) -> Option<&Thumbnail> {
        let target_area = target_width * target_height;
        self.0
            .iter()
            .min_by_key(|t| (i64::from(t.width * t.height) - i64::from(target_area)).abs())
    }

    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Duration in seconds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Duration(pub u64);

impl Duration {
    pub const fn from_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn from_millis(millis: u64) -> Self {
        Self(millis / 1000)
    }

    pub const fn as_seconds(&self) -> u64 {
        self.0
    }

    pub const fn as_millis(&self) -> u64 {
        self.0 * 1000
    }

    /// Format as MM:SS or HH:MM:SS.
    pub fn format(&self) -> String {
        let total_secs = self.0;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        }
    }
}

impl From<u64> for Duration {
    fn from(seconds: u64) -> Self {
        Self(seconds)
    }
}

impl From<Duration> for u64 {
    fn from(d: Duration) -> Self {
        d.0
    }
}

/// Playback position in milliseconds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Position(pub u64);

impl Position {
    pub const fn from_millis(millis: u64) -> Self {
        Self(millis)
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn from_seconds(seconds: f64) -> Self {
        Self((seconds * 1000.0) as u64)
    }

    pub const fn as_millis(&self) -> u64 {
        self.0
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn as_seconds(&self) -> f64 {
        self.0 as f64 / 1000.0
    }

    /// Format as MM:SS or HH:MM:SS.
    pub fn format(&self) -> String {
        Duration::from_seconds(self.0 / 1000).format()
    }
}

impl From<u64> for Position {
    fn from(millis: u64) -> Self {
        Self(millis)
    }
}

/// Volume level (0.0 to 1.0).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Volume(f32);

impl Volume {
    pub const MIN: Self = Self(0.0);
    pub const MAX: Self = Self(1.0);
    pub const DEFAULT: Self = Self(1.0);

    pub const fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub const fn as_f32(&self) -> f32 {
        self.0
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn as_percentage(&self) -> u8 {
        (self.0 * 100.0) as u8
    }

    pub fn from_percentage(percent: u8) -> Self {
        Self::new(f32::from(percent) / 100.0)
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Content type identifier for `YouTube` content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ContentId {
    Video(String),
    Playlist(String),
    Album(String),
    Artist(String),
    Channel(String),
}

impl ContentId {
    pub fn video(id: impl Into<String>) -> Self {
        Self::Video(id.into())
    }

    pub fn playlist(id: impl Into<String>) -> Self {
        Self::Playlist(id.into())
    }

    pub fn album(id: impl Into<String>) -> Self {
        Self::Album(id.into())
    }

    pub fn artist(id: impl Into<String>) -> Self {
        Self::Artist(id.into())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Video(id)
            | Self::Playlist(id)
            | Self::Album(id)
            | Self::Artist(id)
            | Self::Channel(id) => id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_format() {
        assert_eq!(Duration::from_seconds(65).format(), "1:05");
        assert_eq!(Duration::from_seconds(3661).format(), "1:01:01");
        assert_eq!(Duration::from_seconds(0).format(), "0:00");
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_volume_clamping() {
        assert_eq!(Volume::new(1.5).as_f32(), 1.0);
        assert_eq!(Volume::new(-0.5).as_f32(), 0.0);
        assert_eq!(Volume::new(0.5).as_f32(), 0.5);
    }

    #[test]
    fn test_thumbnails_best() {
        let thumbs = Thumbnails::new(vec![
            Thumbnail::new("small", 100, 100),
            Thumbnail::new("large", 500, 500),
            Thumbnail::new("medium", 200, 200),
        ]);
        assert_eq!(thumbs.best().unwrap().url, "large");
        assert_eq!(thumbs.smallest().unwrap().url, "small");
    }
}
