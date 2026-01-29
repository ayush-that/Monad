//! Browse endpoint implementation for albums, artists, and playlists.

use monad_core::{
    types::{ArtistPreview, Thumbnail, Thumbnails, TrackAlbum, TrackArtist},
    Album, AlbumType, Artist, Duration, Error, Playlist, PlaylistAuthor, Result, Track,
};

use crate::{
    types::{BrowsePayload, InnerTubeRequest, RawBrowseResponse},
    InnerTubeClient,
};

impl InnerTubeClient {
    /// Get album details by browse ID.
    pub async fn get_album(&self, browse_id: &str) -> Result<Album> {
        let payload = BrowsePayload {
            browse_id: browse_id.to_string(),
            params: None,
            continuation: None,
        };

        let request = InnerTubeRequest::new(self.context.clone(), payload);

        let response: RawBrowseResponse = self
            .post("browse", &request)
            .await
            .map_err(|e| Error::InnerTube(format!("Browse request failed: {e}")))?;

        parse_album_response(browse_id, &response)
    }

    /// Get artist details by channel ID.
    pub async fn get_artist(&self, channel_id: &str) -> Result<Artist> {
        let payload = BrowsePayload {
            browse_id: channel_id.to_string(),
            params: None,
            continuation: None,
        };

        let request = InnerTubeRequest::new(self.context.clone(), payload);

        let response: RawBrowseResponse = self
            .post("browse", &request)
            .await
            .map_err(|e| Error::InnerTube(format!("Browse request failed: {e}")))?;

        parse_artist_response(channel_id, &response)
    }

    /// Get playlist details by playlist ID.
    pub async fn get_playlist(&self, playlist_id: &str) -> Result<Playlist> {
        // Add VL prefix if not present
        let browse_id = if playlist_id.starts_with("VL") {
            playlist_id.to_string()
        } else {
            format!("VL{playlist_id}")
        };

        let payload = BrowsePayload {
            browse_id,
            params: None,
            continuation: None,
        };

        let request = InnerTubeRequest::new(self.context.clone(), payload);

        let response: RawBrowseResponse = self
            .post("browse", &request)
            .await
            .map_err(|e| Error::InnerTube(format!("Browse request failed: {e}")))?;

        parse_playlist_response(playlist_id, &response)
    }
}

fn parse_album_response(browse_id: &str, response: &RawBrowseResponse) -> Result<Album> {
    let mut album = Album::new(browse_id, "Unknown Album");

    // Parse header
    if let Some(header) = &response.header {
        if let Some(detail_header) = header.get("musicDetailHeaderRenderer") {
            // Title
            if let Some(title) = detail_header
                .get("title")
                .and_then(|t| t.get("runs"))
                .and_then(|r| r.as_array())
                .and_then(|a| a.first())
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
            {
                album.title = title.to_string();
            }

            // Subtitle (artist, year, etc.)
            if let Some(subtitle_runs) = detail_header
                .get("subtitle")
                .and_then(|s| s.get("runs"))
                .and_then(|r| r.as_array())
            {
                for run in subtitle_runs {
                    if let Some(text) = run.get("text").and_then(|t| t.as_str()) {
                        // Check for year
                        if text.len() == 4 {
                            if let Ok(year) = text.parse::<u16>() {
                                if (1900..=2100).contains(&year) {
                                    album.year = Some(year);
                                }
                            }
                        }

                        // Check for album type
                        let text_lower = text.to_lowercase();
                        if text_lower == "album" {
                            album.album_type = AlbumType::Album;
                        } else if text_lower == "single" {
                            album.album_type = AlbumType::Single;
                        } else if text_lower == "ep" {
                            album.album_type = AlbumType::EP;
                        }
                    }

                    // Check for artist with browse endpoint
                    if let Some(browse_endpoint) = run
                        .get("navigationEndpoint")
                        .and_then(|n| n.get("browseEndpoint"))
                    {
                        if let Some(artist_id) =
                            browse_endpoint.get("browseId").and_then(|b| b.as_str())
                        {
                            if artist_id.starts_with("UC") {
                                if let Some(artist_name) = run.get("text").and_then(|t| t.as_str())
                                {
                                    album
                                        .artists
                                        .push(TrackArtist::new(artist_name).with_id(artist_id));
                                }
                            }
                        }
                    }
                }
            }

            // Thumbnails
            if let Some(thumbs) = detail_header
                .get("thumbnail")
                .and_then(|t| t.get("croppedSquareThumbnailRenderer"))
                .and_then(|c| c.get("thumbnail"))
                .and_then(|t| t.get("thumbnails"))
                .and_then(|t| t.as_array())
            {
                album.thumbnails = parse_thumbnail_array(thumbs);
            }

            // Description
            if let Some(desc) = detail_header
                .get("description")
                .and_then(|d| d.get("runs"))
                .and_then(|r| r.as_array())
            {
                let description: String = desc
                    .iter()
                    .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                    .collect();
                if !description.is_empty() {
                    album.description = Some(description);
                }
            }
        }
    }

    // Parse contents for tracks
    if let Some(contents) = &response.contents {
        if let Some(single_column) = contents.get("singleColumnBrowseResultsRenderer") {
            if let Some(tabs) = single_column.get("tabs").and_then(|t| t.as_array()) {
                for tab in tabs {
                    if let Some(tab_content) = tab
                        .get("tabRenderer")
                        .and_then(|t| t.get("content"))
                        .and_then(|c| c.get("sectionListRenderer"))
                        .and_then(|s| s.get("contents"))
                        .and_then(|c| c.as_array())
                    {
                        for section in tab_content {
                            if let Some(shelf) = section.get("musicShelfRenderer") {
                                if let Some(items) =
                                    shelf.get("contents").and_then(|c| c.as_array())
                                {
                                    for (index, item) in items.iter().enumerate() {
                                        if let Some(track) = parse_album_track(item, &album, index)
                                        {
                                            album.tracks.push(track);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    album.track_count = Some(album.tracks.len() as u32);

    // Calculate total duration
    let total_seconds: u64 = album.tracks.iter().map(|t| t.duration.as_seconds()).sum();
    if total_seconds > 0 {
        album.duration = Some(Duration::from_seconds(total_seconds));
    }

    Ok(album)
}

fn parse_album_track(item: &serde_json::Value, album: &Album, _index: usize) -> Option<Track> {
    let renderer = item.get("musicResponsiveListItemRenderer")?;

    // Get video ID from play endpoint or overlay
    let video_id = renderer
        .get("playlistItemData")
        .and_then(|p| p.get("videoId"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            renderer
                .get("overlay")
                .and_then(|o| o.get("musicItemThumbnailOverlayRenderer"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.get("musicPlayButtonRenderer"))
                .and_then(|m| m.get("playNavigationEndpoint"))
                .and_then(|p| p.get("watchEndpoint"))
                .and_then(|w| w.get("videoId"))
                .and_then(|v| v.as_str())
        })?;

    // Get title from flex columns
    let flex_columns = renderer.get("flexColumns")?.as_array()?;

    let title = flex_columns
        .first()
        .and_then(|c| c.get("musicResponsiveListItemFlexColumnRenderer"))
        .and_then(|r| r.get("text"))
        .and_then(|t| t.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())?;

    let mut track = Track::new(video_id, title);

    // Copy album info
    track.artists = album.artists.clone();
    track.album = Some(TrackAlbum::new(&album.title).with_id(&album.id));
    track.thumbnails = album.thumbnails.clone();

    // Get duration from fixed columns
    if let Some(fixed_columns) = renderer.get("fixedColumns").and_then(|f| f.as_array()) {
        if let Some(duration_text) = fixed_columns
            .first()
            .and_then(|c| c.get("musicResponsiveListItemFixedColumnRenderer"))
            .and_then(|r| r.get("text"))
            .and_then(|t| t.get("runs"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
        {
            track.duration = parse_duration_str(duration_text);
        }
    }

    // Check explicit badge
    if let Some(badges) = renderer.get("badges").and_then(|b| b.as_array()) {
        track.is_explicit = badges.iter().any(|b| {
            b.get("musicInlinedBadgeRenderer")
                .and_then(|m| m.get("icon"))
                .and_then(|i| i.get("iconType"))
                .and_then(|t| t.as_str())
                .is_some_and(|t| t == "MUSIC_EXPLICIT_BADGE")
        });
    }

    Some(track)
}

fn parse_artist_response(channel_id: &str, response: &RawBrowseResponse) -> Result<Artist> {
    let mut artist = Artist::new(channel_id, "Unknown Artist");

    // Parse header
    if let Some(header) = &response.header {
        if let Some(music_header) = header.get("musicImmersiveHeaderRenderer") {
            // Name
            if let Some(title) = music_header
                .get("title")
                .and_then(|t| t.get("runs"))
                .and_then(|r| r.as_array())
                .and_then(|a| a.first())
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
            {
                artist.name = title.to_string();
            }

            // Subscriber count
            if let Some(sub_text) = music_header
                .get("subscriptionButton")
                .and_then(|s| s.get("subscribeButtonRenderer"))
                .and_then(|s| s.get("subscriberCountText"))
                .and_then(|s| s.get("runs"))
                .and_then(|r| r.as_array())
                .and_then(|a| a.first())
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
            {
                artist.subscriber_count = Some(sub_text.to_string());
            }

            // Thumbnails
            if let Some(thumbs) = music_header
                .get("thumbnail")
                .and_then(|t| t.get("musicThumbnailRenderer"))
                .and_then(|m| m.get("thumbnail"))
                .and_then(|t| t.get("thumbnails"))
                .and_then(|t| t.as_array())
            {
                artist.thumbnails = parse_thumbnail_array(thumbs);
            }

            // Description
            if let Some(desc) = music_header
                .get("description")
                .and_then(|d| d.get("runs"))
                .and_then(|r| r.as_array())
            {
                let description: String = desc
                    .iter()
                    .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                    .collect();
                if !description.is_empty() {
                    artist.description = Some(description);
                }
            }
        }

        // Also check for visual header (fallback)
        if artist.name == "Unknown Artist" {
            if let Some(visual_header) = header.get("musicVisualHeaderRenderer") {
                if let Some(title) = visual_header
                    .get("title")
                    .and_then(|t| t.get("runs"))
                    .and_then(|r| r.as_array())
                    .and_then(|a| a.first())
                    .and_then(|r| r.get("text"))
                    .and_then(|t| t.as_str())
                {
                    artist.name = title.to_string();
                }
            }
        }
    }

    // Parse contents for songs, albums, etc.
    if let Some(contents) = &response.contents {
        if let Some(single_column) = contents.get("singleColumnBrowseResultsRenderer") {
            if let Some(tabs) = single_column.get("tabs").and_then(|t| t.as_array()) {
                for tab in tabs {
                    if let Some(tab_content) = tab
                        .get("tabRenderer")
                        .and_then(|t| t.get("content"))
                        .and_then(|c| c.get("sectionListRenderer"))
                        .and_then(|s| s.get("contents"))
                        .and_then(|c| c.as_array())
                    {
                        for section in tab_content {
                            parse_artist_section(section, &mut artist);
                        }
                    }
                }
            }
        }
    }

    Ok(artist)
}

fn parse_artist_section(section: &serde_json::Value, artist: &mut Artist) {
    // Check for music shelf (songs)
    if let Some(shelf) = section.get("musicShelfRenderer") {
        let title = shelf
            .get("title")
            .and_then(|t| t.get("runs"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        if title.to_lowercase().contains("song") {
            if let Some(items) = shelf.get("contents").and_then(|c| c.as_array()) {
                for item in items.iter().take(10) {
                    if let Some(track) = parse_artist_track(item) {
                        artist.songs.push(track);
                    }
                }
            }
        }
    }

    // Check for carousel shelf (albums, singles)
    if let Some(carousel) = section.get("musicCarouselShelfRenderer") {
        let title = carousel
            .get("header")
            .and_then(|h| h.get("musicCarouselShelfBasicHeaderRenderer"))
            .and_then(|h| h.get("title"))
            .and_then(|t| t.get("runs"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("");

        if let Some(items) = carousel.get("contents").and_then(|c| c.as_array()) {
            let title_lower = title.to_lowercase();

            if title_lower.contains("album") {
                for item in items.iter().take(10) {
                    if let Some(album) = parse_carousel_album(item) {
                        artist.albums.push(album);
                    }
                }
            } else if title_lower.contains("single") {
                for item in items.iter().take(10) {
                    if let Some(mut single) = parse_carousel_album(item) {
                        single.album_type = AlbumType::Single;
                        artist.singles.push(single);
                    }
                }
            } else if title_lower.contains("similar") || title_lower.contains("fans also like") {
                for item in items.iter().take(10) {
                    if let Some(similar) = parse_carousel_artist(item) {
                        artist.similar_artists.push(similar);
                    }
                }
            }
        }
    }
}

fn parse_artist_track(item: &serde_json::Value) -> Option<Track> {
    let renderer = item.get("musicResponsiveListItemRenderer")?;

    let video_id = renderer
        .get("playlistItemData")
        .and_then(|p| p.get("videoId"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            renderer
                .get("overlay")
                .and_then(|o| o.get("musicItemThumbnailOverlayRenderer"))
                .and_then(|m| m.get("content"))
                .and_then(|c| c.get("musicPlayButtonRenderer"))
                .and_then(|m| m.get("playNavigationEndpoint"))
                .and_then(|p| p.get("watchEndpoint"))
                .and_then(|w| w.get("videoId"))
                .and_then(|v| v.as_str())
        })?;

    let flex_columns = renderer.get("flexColumns")?.as_array()?;

    let title = flex_columns
        .first()
        .and_then(|c| c.get("musicResponsiveListItemFlexColumnRenderer"))
        .and_then(|r| r.get("text"))
        .and_then(|t| t.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())?;

    let mut track = Track::new(video_id, title);

    // Get thumbnails
    if let Some(thumbs) = renderer
        .get("thumbnail")
        .and_then(|t| t.get("musicThumbnailRenderer"))
        .and_then(|m| m.get("thumbnail"))
        .and_then(|t| t.get("thumbnails"))
        .and_then(|t| t.as_array())
    {
        track.thumbnails = parse_thumbnail_array(thumbs);
    }

    // Get duration
    if let Some(fixed_columns) = renderer.get("fixedColumns").and_then(|f| f.as_array()) {
        if let Some(duration_text) = fixed_columns
            .first()
            .and_then(|c| c.get("musicResponsiveListItemFixedColumnRenderer"))
            .and_then(|r| r.get("text"))
            .and_then(|t| t.get("runs"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
        {
            track.duration = parse_duration_str(duration_text);
        }
    }

    Some(track)
}

fn parse_carousel_album(item: &serde_json::Value) -> Option<Album> {
    let renderer = item.get("musicTwoRowItemRenderer")?;

    let browse_id = renderer
        .get("navigationEndpoint")
        .and_then(|n| n.get("browseEndpoint"))
        .and_then(|b| b.get("browseId"))
        .and_then(|b| b.as_str())?;

    let title = renderer
        .get("title")
        .and_then(|t| t.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())?;

    let mut album = Album::new(browse_id, title);

    // Get year from subtitle
    if let Some(subtitle_runs) = renderer
        .get("subtitle")
        .and_then(|s| s.get("runs"))
        .and_then(|r| r.as_array())
    {
        for run in subtitle_runs {
            if let Some(text) = run.get("text").and_then(|t| t.as_str()) {
                if text.len() == 4 {
                    if let Ok(year) = text.parse::<u16>() {
                        if (1900..=2100).contains(&year) {
                            album.year = Some(year);
                        }
                    }
                }
            }
        }
    }

    // Get thumbnails
    if let Some(thumbs) = renderer
        .get("thumbnailRenderer")
        .and_then(|t| t.get("musicThumbnailRenderer"))
        .and_then(|m| m.get("thumbnail"))
        .and_then(|t| t.get("thumbnails"))
        .and_then(|t| t.as_array())
    {
        album.thumbnails = parse_thumbnail_array(thumbs);
    }

    Some(album)
}

fn parse_carousel_artist(item: &serde_json::Value) -> Option<ArtistPreview> {
    let renderer = item.get("musicTwoRowItemRenderer")?;

    let browse_id = renderer
        .get("navigationEndpoint")
        .and_then(|n| n.get("browseEndpoint"))
        .and_then(|b| b.get("browseId"))
        .and_then(|b| b.as_str())?;

    if !browse_id.starts_with("UC") {
        return None;
    }

    let name = renderer
        .get("title")
        .and_then(|t| t.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())?;

    let mut artist = ArtistPreview::new(browse_id, name);

    // Get thumbnails
    if let Some(thumbs) = renderer
        .get("thumbnailRenderer")
        .and_then(|t| t.get("musicThumbnailRenderer"))
        .and_then(|m| m.get("thumbnail"))
        .and_then(|t| t.get("thumbnails"))
        .and_then(|t| t.as_array())
    {
        artist.thumbnails = parse_thumbnail_array(thumbs);
    }

    Some(artist)
}

fn parse_playlist_response(playlist_id: &str, response: &RawBrowseResponse) -> Result<Playlist> {
    let mut playlist = Playlist::new(playlist_id, "Unknown Playlist");

    // Parse header
    if let Some(header) = &response.header {
        if let Some(detail_header) = header.get("musicDetailHeaderRenderer") {
            // Title
            if let Some(title) = detail_header
                .get("title")
                .and_then(|t| t.get("runs"))
                .and_then(|r| r.as_array())
                .and_then(|a| a.first())
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
            {
                playlist.title = title.to_string();
            }

            // Subtitle (author, track count)
            if let Some(subtitle_runs) = detail_header
                .get("subtitle")
                .and_then(|s| s.get("runs"))
                .and_then(|r| r.as_array())
            {
                for run in subtitle_runs {
                    if let Some(text) = run.get("text").and_then(|t| t.as_str()) {
                        // Check for track count
                        if text.contains("song") || text.contains("track") {
                            if let Some(num_str) = text.split_whitespace().next() {
                                if let Ok(count) = num_str.parse::<u32>() {
                                    playlist.track_count = Some(count);
                                }
                            }
                        }
                    }

                    // Check for author
                    if run.get("navigationEndpoint").is_some() {
                        if let Some(author_name) = run.get("text").and_then(|t| t.as_str()) {
                            playlist.author = Some(PlaylistAuthor::new(author_name));
                        }
                    }
                }
            }

            // Description
            if let Some(desc) = detail_header
                .get("description")
                .and_then(|d| d.get("runs"))
                .and_then(|r| r.as_array())
            {
                let description: String = desc
                    .iter()
                    .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                    .collect();
                if !description.is_empty() {
                    playlist.description = Some(description);
                }
            }

            // Thumbnails
            if let Some(thumbs) = detail_header
                .get("thumbnail")
                .and_then(|t| t.get("croppedSquareThumbnailRenderer"))
                .and_then(|c| c.get("thumbnail"))
                .and_then(|t| t.get("thumbnails"))
                .and_then(|t| t.as_array())
            {
                playlist.thumbnails = parse_thumbnail_array(thumbs);
            }
        }
    }

    // Parse contents for tracks
    if let Some(contents) = &response.contents {
        if let Some(single_column) = contents.get("singleColumnBrowseResultsRenderer") {
            if let Some(tabs) = single_column.get("tabs").and_then(|t| t.as_array()) {
                for tab in tabs {
                    if let Some(tab_content) = tab
                        .get("tabRenderer")
                        .and_then(|t| t.get("content"))
                        .and_then(|c| c.get("sectionListRenderer"))
                        .and_then(|s| s.get("contents"))
                        .and_then(|c| c.as_array())
                    {
                        for section in tab_content {
                            if let Some(shelf) = section.get("musicPlaylistShelfRenderer") {
                                if let Some(items) =
                                    shelf.get("contents").and_then(|c| c.as_array())
                                {
                                    for item in items {
                                        if let Some(track) = parse_playlist_track(item) {
                                            playlist.tracks.push(track);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if playlist.track_count.is_none() {
        playlist.track_count = Some(playlist.tracks.len() as u32);
    }

    // Calculate total duration
    let total_seconds: u64 = playlist
        .tracks
        .iter()
        .map(|t| t.duration.as_seconds())
        .sum();
    if total_seconds > 0 {
        playlist.duration = Some(Duration::from_seconds(total_seconds));
    }

    Ok(playlist)
}

fn parse_playlist_track(item: &serde_json::Value) -> Option<Track> {
    let renderer = item.get("musicResponsiveListItemRenderer")?;

    let video_id = renderer
        .get("playlistItemData")
        .and_then(|p| p.get("videoId"))
        .and_then(|v| v.as_str())?;

    let flex_columns = renderer.get("flexColumns")?.as_array()?;

    let title = flex_columns
        .first()
        .and_then(|c| c.get("musicResponsiveListItemFlexColumnRenderer"))
        .and_then(|r| r.get("text"))
        .and_then(|t| t.get("runs"))
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("text"))
        .and_then(|t| t.as_str())?;

    let mut track = Track::new(video_id, title);

    // Parse artists and album from second column
    if let Some(second_col) = flex_columns.get(1) {
        if let Some(runs) = second_col
            .get("musicResponsiveListItemFlexColumnRenderer")
            .and_then(|r| r.get("text"))
            .and_then(|t| t.get("runs"))
            .and_then(|r| r.as_array())
        {
            for run in runs {
                if let Some(browse_endpoint) = run
                    .get("navigationEndpoint")
                    .and_then(|n| n.get("browseEndpoint"))
                {
                    if let Some(browse_id) =
                        browse_endpoint.get("browseId").and_then(|b| b.as_str())
                    {
                        if let Some(text) = run.get("text").and_then(|t| t.as_str()) {
                            if browse_id.starts_with("UC") {
                                track
                                    .artists
                                    .push(TrackArtist::new(text).with_id(browse_id));
                            } else if browse_id.starts_with("MPREb") {
                                track.album = Some(TrackAlbum::new(text).with_id(browse_id));
                            }
                        }
                    }
                }
            }
        }
    }

    // Get thumbnails
    if let Some(thumbs) = renderer
        .get("thumbnail")
        .and_then(|t| t.get("musicThumbnailRenderer"))
        .and_then(|m| m.get("thumbnail"))
        .and_then(|t| t.get("thumbnails"))
        .and_then(|t| t.as_array())
    {
        track.thumbnails = parse_thumbnail_array(thumbs);
    }

    // Get duration from fixed columns
    if let Some(fixed_columns) = renderer.get("fixedColumns").and_then(|f| f.as_array()) {
        if let Some(duration_text) = fixed_columns
            .first()
            .and_then(|c| c.get("musicResponsiveListItemFixedColumnRenderer"))
            .and_then(|r| r.get("text"))
            .and_then(|t| t.get("runs"))
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
        {
            track.duration = parse_duration_str(duration_text);
        }
    }

    Some(track)
}

fn parse_thumbnail_array(thumbs: &[serde_json::Value]) -> Thumbnails {
    Thumbnails::new(
        thumbs
            .iter()
            .filter_map(|t| {
                let url = t.get("url")?.as_str()?;
                let width = t
                    .get("width")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u32;
                let height = t
                    .get("height")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0) as u32;
                Some(Thumbnail::new(url, width, height))
            })
            .collect(),
    )
}

fn parse_duration_str(s: &str) -> Duration {
    let parts: Vec<&str> = s.split(':').collect();
    let seconds: u64 = match parts.len() {
        2 => {
            let mins: u64 = parts[0].parse().unwrap_or(0);
            let secs: u64 = parts[1].parse().unwrap_or(0);
            mins * 60 + secs
        }
        3 => {
            let hours: u64 = parts[0].parse().unwrap_or(0);
            let mins: u64 = parts[1].parse().unwrap_or(0);
            let secs: u64 = parts[2].parse().unwrap_or(0);
            hours * 3600 + mins * 60 + secs
        }
        _ => 0,
    };
    Duration::from_seconds(seconds)
}
