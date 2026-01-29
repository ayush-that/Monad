//! Artist type representing a music artist/channel.

use serde::{Deserialize, Serialize};

use super::{Album, Playlist, Thumbnails, Track};

/// A music artist.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artist {
    /// Artist channel ID.
    pub id: String,
    /// Artist name.
    pub name: String,
    /// Artist description/bio.
    pub description: Option<String>,
    /// Subscriber count (formatted string like "1.2M").
    pub subscriber_count: Option<String>,
    /// Thumbnail/avatar images.
    pub thumbnails: Thumbnails,
    /// Top songs by this artist.
    pub songs: Vec<Track>,
    /// Albums by this artist.
    pub albums: Vec<Album>,
    /// Singles by this artist.
    pub singles: Vec<Album>,
    /// Playlists featuring this artist.
    pub playlists: Vec<Playlist>,
    /// Similar artists.
    pub similar_artists: Vec<ArtistPreview>,
}

impl Artist {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            subscriber_count: None,
            thumbnails: Thumbnails::default(),
            songs: Vec::new(),
            albums: Vec::new(),
            singles: Vec::new(),
            playlists: Vec::new(),
            similar_artists: Vec::new(),
        }
    }

    /// Get the best available thumbnail URL.
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnails.best().map(|t| t.url.as_str())
    }
}

/// A preview/summary of an artist (for lists and references).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtistPreview {
    /// Artist channel ID.
    pub id: String,
    /// Artist name.
    pub name: String,
    /// Subscriber count (formatted string).
    pub subscriber_count: Option<String>,
    /// Thumbnail images.
    pub thumbnails: Thumbnails,
}

impl ArtistPreview {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            subscriber_count: None,
            thumbnails: Thumbnails::default(),
        }
    }

    /// Get the best available thumbnail URL.
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnails.best().map(|t| t.url.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artist_creation() {
        let artist = Artist::new("channel_id", "Test Artist");
        assert_eq!(artist.id, "channel_id");
        assert_eq!(artist.name, "Test Artist");
    }
}
