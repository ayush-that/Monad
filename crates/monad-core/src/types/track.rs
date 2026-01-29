//! Track type representing a single song/video.

use serde::{Deserialize, Serialize};

use super::{Duration, Thumbnails};

/// A single track (song/video).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Track {
    /// `YouTube` video ID.
    pub id: String,
    /// Track title.
    pub title: String,
    /// Artist name(s).
    pub artists: Vec<TrackArtist>,
    /// Album information (if available).
    pub album: Option<TrackAlbum>,
    /// Track duration.
    pub duration: Duration,
    /// Thumbnail images.
    pub thumbnails: Thumbnails,
    /// Whether this is an explicit track.
    pub is_explicit: bool,
    /// Whether this track is available for playback.
    pub is_available: bool,
}

impl Track {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            artists: Vec::new(),
            album: None,
            duration: Duration::default(),
            thumbnails: Thumbnails::default(),
            is_explicit: false,
            is_available: true,
        }
    }

    /// Get the primary artist name.
    pub fn artist_name(&self) -> &str {
        self.artists.first().map_or("", |a| a.name.as_str())
    }

    /// Get all artist names joined.
    pub fn artists_display(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Get album name if available.
    pub fn album_name(&self) -> Option<&str> {
        self.album.as_ref().map(|a| a.name.as_str())
    }

    /// Get the best available thumbnail URL.
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnails.best().map(|t| t.url.as_str())
    }

    /// Get a high-quality thumbnail URL using YouTube's image service.
    /// Returns maxresdefault (1920x1080) quality thumbnail.
    pub fn hq_thumbnail_url(&self) -> String {
        format!("https://i.ytimg.com/vi/{}/maxresdefault.jpg", self.id)
    }

    /// Get a standard quality thumbnail URL (640x480).
    pub fn sd_thumbnail_url(&self) -> String {
        format!("https://i.ytimg.com/vi/{}/sddefault.jpg", self.id)
    }
}

/// Artist reference within a track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackArtist {
    /// Artist channel ID (if available).
    pub id: Option<String>,
    /// Artist name.
    pub name: String,
}

impl TrackArtist {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

/// Album reference within a track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackAlbum {
    /// Album/playlist ID.
    pub id: Option<String>,
    /// Album name.
    pub name: String,
}

impl TrackAlbum {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_creation() {
        let track = Track::new("abc123", "Test Song");
        assert_eq!(track.id, "abc123");
        assert_eq!(track.title, "Test Song");
        assert!(track.is_available);
    }

    #[test]
    fn test_track_artists_display() {
        let mut track = Track::new("id", "Title");
        track.artists = vec![TrackArtist::new("Artist 1"), TrackArtist::new("Artist 2")];
        assert_eq!(track.artists_display(), "Artist 1, Artist 2");
    }
}
