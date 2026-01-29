//! Album type representing a music album.

use serde::{Deserialize, Serialize};

use super::{Duration, Thumbnails, Track, TrackArtist};

/// A music album.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Album {
    /// Album browse ID.
    pub id: String,
    /// Album title.
    pub title: String,
    /// Album artists.
    pub artists: Vec<TrackArtist>,
    /// Release year (if known).
    pub year: Option<u16>,
    /// Number of tracks.
    pub track_count: Option<u32>,
    /// Total duration of all tracks.
    pub duration: Option<Duration>,
    /// Album description.
    pub description: Option<String>,
    /// Thumbnail images.
    pub thumbnails: Thumbnails,
    /// Album type (Album, EP, Single).
    pub album_type: AlbumType,
    /// Tracks in this album (populated when fetching full details).
    pub tracks: Vec<Track>,
    /// Whether explicit content.
    pub is_explicit: bool,
}

impl Album {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            artists: Vec::new(),
            year: None,
            track_count: None,
            duration: None,
            description: None,
            thumbnails: Thumbnails::default(),
            album_type: AlbumType::Album,
            tracks: Vec::new(),
            is_explicit: false,
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

    /// Get the best available thumbnail URL.
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnails.best().map(|t| t.url.as_str())
    }

    /// Get subtitle text (artist - year).
    pub fn subtitle(&self) -> String {
        let artist = self.artists_display();
        match self.year {
            Some(year) => format!("{artist} \u{2022} {year}"),
            None => artist,
        }
    }
}

/// Type of album release.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum AlbumType {
    #[default]
    Album,
    Single,
    EP,
    Compilation,
}

impl AlbumType {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Album => "Album",
            Self::Single => "Single",
            Self::EP => "EP",
            Self::Compilation => "Compilation",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_album_creation() {
        let album = Album::new("browse_id", "Test Album");
        assert_eq!(album.id, "browse_id");
        assert_eq!(album.title, "Test Album");
        assert_eq!(album.album_type, AlbumType::Album);
    }

    #[test]
    fn test_album_subtitle() {
        let mut album = Album::new("id", "Title");
        album.artists = vec![TrackArtist::new("Artist")];
        album.year = Some(2024);
        assert_eq!(album.subtitle(), "Artist \u{2022} 2024");
    }
}
