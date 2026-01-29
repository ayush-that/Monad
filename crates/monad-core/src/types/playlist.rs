//! Playlist type representing a collection of tracks.

use serde::{Deserialize, Serialize};

use super::{Duration, Thumbnails, Track};

/// A playlist (user-created or auto-generated).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Playlist {
    /// Playlist ID.
    pub id: String,
    /// Playlist title.
    pub title: String,
    /// Playlist description.
    pub description: Option<String>,
    /// Owner/author name.
    pub author: Option<PlaylistAuthor>,
    /// Number of tracks.
    pub track_count: Option<u32>,
    /// Total duration.
    pub duration: Option<Duration>,
    /// Thumbnail images.
    pub thumbnails: Thumbnails,
    /// Tracks in this playlist.
    pub tracks: Vec<Track>,
    /// Privacy status.
    pub privacy: PlaylistPrivacy,
    /// Year the playlist was created/updated.
    pub year: Option<u16>,
}

impl Playlist {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: None,
            author: None,
            track_count: None,
            duration: None,
            thumbnails: Thumbnails::default(),
            tracks: Vec::new(),
            privacy: PlaylistPrivacy::Public,
            year: None,
        }
    }

    /// Get the author name.
    pub fn author_name(&self) -> Option<&str> {
        self.author.as_ref().map(|a| a.name.as_str())
    }

    /// Get the best available thumbnail URL.
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnails.best().map(|t| t.url.as_str())
    }

    /// Get subtitle text showing author and track count.
    pub fn subtitle(&self) -> String {
        let mut parts = Vec::new();

        if let Some(author) = &self.author {
            parts.push(author.name.clone());
        }

        if let Some(count) = self.track_count {
            let tracks = if count == 1 { "track" } else { "tracks" };
            parts.push(format!("{count} {tracks}"));
        }

        parts.join(" \u{2022} ")
    }
}

/// Playlist author/owner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaylistAuthor {
    /// Author channel ID (if available).
    pub id: Option<String>,
    /// Author name.
    pub name: String,
}

impl PlaylistAuthor {
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

/// Playlist privacy status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlaylistPrivacy {
    #[default]
    Public,
    Unlisted,
    Private,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playlist_creation() {
        let playlist = Playlist::new("playlist_id", "My Playlist");
        assert_eq!(playlist.id, "playlist_id");
        assert_eq!(playlist.title, "My Playlist");
        assert_eq!(playlist.privacy, PlaylistPrivacy::Public);
    }

    #[test]
    fn test_playlist_subtitle() {
        let mut playlist = Playlist::new("id", "Title");
        playlist.author = Some(PlaylistAuthor::new("User"));
        playlist.track_count = Some(10);
        assert_eq!(playlist.subtitle(), "User \u{2022} 10 tracks");
    }
}
