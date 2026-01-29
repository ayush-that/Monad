//! Core domain types for Monad.

pub mod album;
pub mod artist;
pub mod common;
pub mod playlist;
pub mod queue;
pub mod stream;
pub mod track;

pub use album::{Album, AlbumType};
pub use artist::{Artist, ArtistPreview};
pub use common::*;
pub use playlist::Playlist;
pub use playlist::{PlaylistAuthor, PlaylistPrivacy};
pub use queue::{Queue, QueueItem, QueueSource, RepeatMode};
pub use stream::{AudioFormat, AudioQuality, StreamCollection, StreamInfo};
pub use track::{Track, TrackAlbum, TrackArtist};
