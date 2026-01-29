//! Response parsers for `InnerTube` API responses.

use monad_core::{
    types::{ArtistPreview, Thumbnail, Thumbnails, TrackAlbum, TrackArtist},
    Album, AlbumType, Duration, Playlist, PlaylistAuthor, Track,
};

use crate::types::{
    MusicResponsiveListItemRenderer, MusicTwoRowItemRenderer, RawSearchResponse, SearchResults,
    SectionListRenderer, ShelfItem, ThumbnailRenderer,
};

/// Parse search results from raw `InnerTube` response.
pub fn parse_search_results(response: &RawSearchResponse) -> SearchResults {
    let mut results = SearchResults::default();

    // Parse from main contents
    if let Some(contents) = &response.contents {
        if let Some(tabbed) = &contents.tabbed_search_results_renderer {
            for tab in &tabbed.tabs {
                if let Some(tab_renderer) = &tab.tab_renderer {
                    if let Some(content) = &tab_renderer.content {
                        if let Some(section_list) = &content.section_list_renderer {
                            parse_section_list(section_list, &mut results);
                        }
                    }
                }
            }
        }
    }

    // Parse from continuation contents
    if let Some(cont) = &response.continuation_contents {
        if let Some(shelf_cont) = &cont.music_shelf_continuation {
            if let Some(items) = &shelf_cont.contents {
                for item in items {
                    parse_shelf_item(item, &mut results, None);
                }
            }
            results.continuation = shelf_cont
                .continuations
                .as_ref()
                .and_then(|c| c.first())
                .and_then(|c| c.next_continuation_data.as_ref())
                .map(|d| d.continuation.clone());
        }
    }

    results
}

fn parse_section_list(section_list: &SectionListRenderer, results: &mut SearchResults) {
    if let Some(contents) = &section_list.contents {
        for section in contents {
            if let Some(shelf) = &section.music_shelf_renderer {
                let category = shelf
                    .title
                    .as_ref()
                    .map(super::types::TextRuns::text)
                    .unwrap_or_default();

                if let Some(items) = &shelf.contents {
                    for item in items {
                        parse_shelf_item(item, results, Some(&category));
                    }
                }

                // Get continuation if present
                if results.continuation.is_none() {
                    results.continuation = shelf
                        .continuations
                        .as_ref()
                        .and_then(|c| c.first())
                        .and_then(|c| c.next_continuation_data.as_ref())
                        .map(|d| d.continuation.clone());
                }
            }

            if let Some(card) = &section.music_card_shelf_renderer {
                // Top result card
                if let Some(items) = &card.contents {
                    for item in items {
                        parse_shelf_item(item, results, None);
                    }
                }
            }
        }
    }
}

fn parse_shelf_item(item: &ShelfItem, results: &mut SearchResults, category: Option<&str>) {
    if let Some(renderer) = &item.music_responsive_list_item_renderer {
        let item_type = determine_item_type(renderer, category);

        // Debug: log item details
        let has_watch = renderer
            .navigation_endpoint
            .as_ref()
            .and_then(|n| n.watch_endpoint.as_ref())
            .is_some();
        let has_play = renderer.play_endpoint.is_some();
        let has_browse = renderer
            .navigation_endpoint
            .as_ref()
            .and_then(|n| n.browse_endpoint.as_ref())
            .map(|b| b.browse_id.clone());

        // Check overlay for video_id (songs often have this)
        let overlay_video_id = renderer
            .overlay
            .as_ref()
            .and_then(|o| o.get("musicItemThumbnailOverlayRenderer"))
            .and_then(|r| r.get("content"))
            .and_then(|c| c.get("musicPlayButtonRenderer"))
            .and_then(|p| p.get("playNavigationEndpoint"))
            .and_then(|n| n.get("watchEndpoint"))
            .and_then(|w| w.get("videoId"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        tracing::debug!(
            "Item: type={:?}, has_watch={}, has_play={}, browse_id={:?}, overlay_video_id={:?}, category={:?}",
            item_type, has_watch, has_play, has_browse, overlay_video_id, category
        );

        match item_type {
            ItemType::Song => {
                if let Some(track) = parse_track_from_renderer(renderer) {
                    results.songs.push(track);
                }
            }
            ItemType::Video => {
                if let Some(track) = parse_track_from_renderer(renderer) {
                    results.videos.push(track);
                }
            }
            ItemType::Album => {
                if let Some(album) = parse_album_from_renderer(renderer) {
                    results.albums.push(album);
                }
            }
            ItemType::Artist => {
                if let Some(artist) = parse_artist_from_renderer(renderer) {
                    results.artists.push(artist);
                }
            }
            ItemType::Playlist => {
                if let Some(playlist) = parse_playlist_from_renderer(renderer) {
                    results.playlists.push(playlist);
                }
            }
            ItemType::Unknown => {
                tracing::debug!("Unknown item type, skipping");
            }
        }
    }

    if let Some(renderer) = &item.music_two_row_item_renderer {
        // Two-row items (albums, artists, playlists in grid view)
        if let Some(nav) = &renderer.navigation_endpoint {
            if let Some(browse) = &nav.browse_endpoint {
                let browse_id = &browse.browse_id;

                if browse_id.starts_with("MPREb") {
                    // Album
                    if let Some(album) = parse_album_from_two_row(renderer) {
                        results.albums.push(album);
                    }
                } else if browse_id.starts_with("UC") {
                    // Artist
                    if let Some(artist) = parse_artist_from_two_row(renderer) {
                        results.artists.push(artist);
                    }
                } else if browse_id.starts_with("VL") || browse_id.starts_with("PL") {
                    // Playlist
                    if let Some(playlist) = parse_playlist_from_two_row(renderer) {
                        results.playlists.push(playlist);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemType {
    Song,
    Video,
    Album,
    Artist,
    Playlist,
    Unknown,
}

fn determine_item_type(
    renderer: &MusicResponsiveListItemRenderer,
    category: Option<&str>,
) -> ItemType {
    // Check category hint first
    if let Some(cat) = category {
        let cat_lower = cat.to_lowercase();
        if cat_lower.contains("song") {
            return ItemType::Song;
        }
        if cat_lower.contains("video") {
            return ItemType::Video;
        }
        if cat_lower.contains("album") || cat_lower.contains("single") || cat_lower.contains("ep") {
            return ItemType::Album;
        }
        if cat_lower.contains("artist") {
            return ItemType::Artist;
        }
        if cat_lower.contains("playlist") {
            return ItemType::Playlist;
        }
    }

    // Check navigation endpoint
    if let Some(nav) = &renderer.navigation_endpoint {
        if nav.watch_endpoint.is_some() {
            // Has watch endpoint = playable = song or video
            return ItemType::Song;
        }
        if let Some(browse) = &nav.browse_endpoint {
            let browse_id = &browse.browse_id;
            if browse_id.starts_with("MPREb") {
                return ItemType::Album;
            }
            if browse_id.starts_with("UC") {
                return ItemType::Artist;
            }
            if browse_id.starts_with("VL") || browse_id.starts_with("PL") {
                return ItemType::Playlist;
            }
        }
    }

    // Check play endpoint
    if renderer.play_endpoint.is_some() {
        return ItemType::Song;
    }

    // Check overlay for video ID (songs often have playable overlay)
    let has_overlay_video = renderer
        .overlay
        .as_ref()
        .and_then(|o| o.get("musicItemThumbnailOverlayRenderer"))
        .and_then(|r| r.get("content"))
        .and_then(|c| c.get("musicPlayButtonRenderer"))
        .and_then(|p| p.get("playNavigationEndpoint"))
        .and_then(|n| n.get("watchEndpoint"))
        .and_then(|w| w.get("videoId"))
        .is_some();

    if has_overlay_video {
        return ItemType::Song;
    }

    ItemType::Unknown
}

fn parse_track_from_renderer(renderer: &MusicResponsiveListItemRenderer) -> Option<Track> {
    let video_id = renderer
        .navigation_endpoint
        .as_ref()
        .and_then(|n| n.watch_endpoint.as_ref())
        .map(|w| w.video_id.clone())
        .or_else(|| {
            renderer
                .play_endpoint
                .as_ref()
                .and_then(|p| p.watch_endpoint.as_ref())
                .map(|w| w.video_id.clone())
        })
        .or_else(|| {
            // Try to get video ID from overlay (songs often have this)
            renderer
                .overlay
                .as_ref()
                .and_then(|o| o.get("musicItemThumbnailOverlayRenderer"))
                .and_then(|r| r.get("content"))
                .and_then(|c| c.get("musicPlayButtonRenderer"))
                .and_then(|p| p.get("playNavigationEndpoint"))
                .and_then(|n| n.get("watchEndpoint"))
                .and_then(|w| w.get("videoId"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })?;

    let columns = renderer.flex_columns.as_ref()?;

    // First column: title
    let title = columns
        .first()
        .and_then(|c| c.music_responsive_list_item_flex_column_renderer.as_ref())
        .and_then(|r| r.text.as_ref())
        .map(super::types::TextRuns::text)
        .unwrap_or_default();

    if title.is_empty() {
        return None;
    }

    let mut track = Track::new(video_id, title);

    // Second column: artist(s) and album
    if let Some(second_col) = columns.get(1) {
        if let Some(col_renderer) = &second_col.music_responsive_list_item_flex_column_renderer {
            if let Some(text) = &col_renderer.text {
                if let Some(runs) = &text.runs {
                    let mut artists = Vec::new();
                    let mut album: Option<TrackAlbum> = None;

                    for run in runs {
                        if let Some(nav) = &run.navigation_endpoint {
                            if let Some(browse) = &nav.browse_endpoint {
                                let browse_id = &browse.browse_id;
                                if browse_id.starts_with("UC") {
                                    // Artist
                                    artists.push(
                                        TrackArtist::new(&run.text).with_id(browse_id.clone()),
                                    );
                                } else if browse_id.starts_with("MPREb") {
                                    // Album
                                    album =
                                        Some(TrackAlbum::new(&run.text).with_id(browse_id.clone()));
                                }
                            }
                        } else if run.text != " • " && run.text != " & " && run.text != ", " {
                            // Plain text artist name without navigation
                            if artists.is_empty() && album.is_none() {
                                artists.push(TrackArtist::new(&run.text));
                            }
                        }
                    }

                    track.artists = artists;
                    track.album = album;
                }
            }
        }
    }

    // Duration from fixed column
    if let Some(fixed_cols) = &renderer.fixed_columns {
        if let Some(first_fixed) = fixed_cols.first() {
            if let Some(col_renderer) =
                &first_fixed.music_responsive_list_item_fixed_column_renderer
            {
                if let Some(text) = &col_renderer.text {
                    let duration_str = text.text();
                    track.duration = parse_duration(&duration_str);
                }
            }
        }
    }

    // Thumbnails
    track.thumbnails = parse_thumbnails_from_renderer(&renderer.thumbnail);

    // Explicit badge
    if let Some(badges) = &renderer.badges {
        track.is_explicit = badges.iter().any(|b| {
            b.music_inlined_badge_renderer
                .as_ref()
                .and_then(|r| r.icon.as_ref())
                .and_then(|i| i.icon_type.as_ref())
                .is_some_and(|t| t == "MUSIC_EXPLICIT_BADGE")
        });
    }

    Some(track)
}

fn parse_album_from_renderer(renderer: &MusicResponsiveListItemRenderer) -> Option<Album> {
    let browse_id = renderer
        .navigation_endpoint
        .as_ref()
        .and_then(|n| n.browse_endpoint.as_ref())
        .map(|b| b.browse_id.clone())?;

    let columns = renderer.flex_columns.as_ref()?;

    let title = columns
        .first()
        .and_then(|c| c.music_responsive_list_item_flex_column_renderer.as_ref())
        .and_then(|r| r.text.as_ref())
        .map(super::types::TextRuns::text)
        .unwrap_or_default();

    if title.is_empty() {
        return None;
    }

    let mut album = Album::new(browse_id, title);

    // Second column: type, artist, year
    if let Some(second_col) = columns.get(1) {
        if let Some(col_renderer) = &second_col.music_responsive_list_item_flex_column_renderer {
            if let Some(text) = &col_renderer.text {
                if let Some(runs) = &text.runs {
                    for run in runs {
                        let text = run.text.trim();

                        // Check for album type
                        let text_lower = text.to_lowercase();
                        if text_lower == "single" {
                            album.album_type = AlbumType::Single;
                        } else if text_lower == "ep" {
                            album.album_type = AlbumType::EP;
                        } else if text_lower == "album" {
                            album.album_type = AlbumType::Album;
                        }

                        // Check for year (4 digits)
                        if text.len() == 4 {
                            if let Ok(year) = text.parse::<u16>() {
                                if (1900..=2100).contains(&year) {
                                    album.year = Some(year);
                                }
                            }
                        }

                        // Check for artist
                        if let Some(nav) = &run.navigation_endpoint {
                            if let Some(browse) = &nav.browse_endpoint {
                                if browse.browse_id.starts_with("UC") {
                                    album.artists.push(
                                        TrackArtist::new(&run.text).with_id(&browse.browse_id),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    album.thumbnails = parse_thumbnails_from_renderer(&renderer.thumbnail);

    Some(album)
}

fn parse_artist_from_renderer(renderer: &MusicResponsiveListItemRenderer) -> Option<ArtistPreview> {
    let browse_id = renderer
        .navigation_endpoint
        .as_ref()
        .and_then(|n| n.browse_endpoint.as_ref())
        .map(|b| b.browse_id.clone())?;

    let columns = renderer.flex_columns.as_ref()?;

    let name = columns
        .first()
        .and_then(|c| c.music_responsive_list_item_flex_column_renderer.as_ref())
        .and_then(|r| r.text.as_ref())
        .map(super::types::TextRuns::text)
        .unwrap_or_default();

    if name.is_empty() {
        return None;
    }

    let mut artist = ArtistPreview::new(browse_id, name);

    // Second column: subscriber count
    if let Some(second_col) = columns.get(1) {
        if let Some(col_renderer) = &second_col.music_responsive_list_item_flex_column_renderer {
            if let Some(text) = &col_renderer.text {
                let sub_text = text.text();
                if sub_text.contains("subscriber") {
                    artist.subscriber_count = Some(sub_text.replace(" subscribers", ""));
                }
            }
        }
    }

    artist.thumbnails = parse_thumbnails_from_renderer(&renderer.thumbnail);

    Some(artist)
}

fn parse_playlist_from_renderer(renderer: &MusicResponsiveListItemRenderer) -> Option<Playlist> {
    let browse_id = renderer
        .navigation_endpoint
        .as_ref()
        .and_then(|n| n.browse_endpoint.as_ref())
        .map(|b| b.browse_id.clone())?;

    // Remove VL prefix if present
    let id = browse_id
        .strip_prefix("VL")
        .unwrap_or(&browse_id)
        .to_string();

    let columns = renderer.flex_columns.as_ref()?;

    let title = columns
        .first()
        .and_then(|c| c.music_responsive_list_item_flex_column_renderer.as_ref())
        .and_then(|r| r.text.as_ref())
        .map(super::types::TextRuns::text)
        .unwrap_or_default();

    if title.is_empty() {
        return None;
    }

    let mut playlist = Playlist::new(id, title);

    // Second column: author, track count
    if let Some(second_col) = columns.get(1) {
        if let Some(col_renderer) = &second_col.music_responsive_list_item_flex_column_renderer {
            if let Some(text) = &col_renderer.text {
                if let Some(runs) = &text.runs {
                    for run in runs {
                        let text = &run.text;

                        // Check for track count
                        if text.contains("song") || text.contains("track") {
                            if let Some(num_str) = text.split_whitespace().next() {
                                if let Ok(count) = num_str.parse::<u32>() {
                                    playlist.track_count = Some(count);
                                }
                            }
                        }

                        // Check for author
                        if let Some(nav) = &run.navigation_endpoint {
                            if nav.browse_endpoint.is_some() {
                                playlist.author = Some(PlaylistAuthor::new(&run.text));
                            }
                        }
                    }
                }
            }
        }
    }

    playlist.thumbnails = parse_thumbnails_from_renderer(&renderer.thumbnail);

    Some(playlist)
}

fn parse_album_from_two_row(renderer: &MusicTwoRowItemRenderer) -> Option<Album> {
    let browse_id = renderer
        .navigation_endpoint
        .as_ref()
        .and_then(|n| n.browse_endpoint.as_ref())
        .map(|b| b.browse_id.clone())?;

    let title = renderer.title.as_ref()?.text();
    if title.is_empty() {
        return None;
    }

    let mut album = Album::new(browse_id, title);

    if let Some(subtitle) = &renderer.subtitle {
        if let Some(runs) = &subtitle.runs {
            for run in runs {
                let text = run.text.trim();

                // Check for year
                if text.len() == 4 {
                    if let Ok(year) = text.parse::<u16>() {
                        if (1900..=2100).contains(&year) {
                            album.year = Some(year);
                        }
                    }
                }

                // Check for artist
                if let Some(nav) = &run.navigation_endpoint {
                    if let Some(browse) = &nav.browse_endpoint {
                        if browse.browse_id.starts_with("UC") {
                            album
                                .artists
                                .push(TrackArtist::new(&run.text).with_id(&browse.browse_id));
                        }
                    }
                }
            }
        }
    }

    album.thumbnails = parse_thumbnails_from_two_row(&renderer.thumbnail_renderer);

    Some(album)
}

fn parse_artist_from_two_row(renderer: &MusicTwoRowItemRenderer) -> Option<ArtistPreview> {
    let browse_id = renderer
        .navigation_endpoint
        .as_ref()
        .and_then(|n| n.browse_endpoint.as_ref())
        .map(|b| b.browse_id.clone())?;

    let name = renderer.title.as_ref()?.text();
    if name.is_empty() {
        return None;
    }

    let mut artist = ArtistPreview::new(browse_id, name);

    if let Some(subtitle) = &renderer.subtitle {
        let sub_text = subtitle.text();
        if sub_text.contains("subscriber") {
            artist.subscriber_count = Some(sub_text.replace(" subscribers", ""));
        }
    }

    artist.thumbnails = parse_thumbnails_from_two_row(&renderer.thumbnail_renderer);

    Some(artist)
}

fn parse_playlist_from_two_row(renderer: &MusicTwoRowItemRenderer) -> Option<Playlist> {
    let browse_id = renderer
        .navigation_endpoint
        .as_ref()
        .and_then(|n| n.browse_endpoint.as_ref())
        .map(|b| b.browse_id.clone())?;

    let id = browse_id
        .strip_prefix("VL")
        .unwrap_or(&browse_id)
        .to_string();

    let title = renderer.title.as_ref()?.text();
    if title.is_empty() {
        return None;
    }

    let mut playlist = Playlist::new(id, title);

    if let Some(subtitle) = &renderer.subtitle {
        if let Some(runs) = &subtitle.runs {
            for run in runs {
                let text = &run.text;

                if text.contains("song") || text.contains("track") {
                    if let Some(num_str) = text.split_whitespace().next() {
                        if let Ok(count) = num_str.parse::<u32>() {
                            playlist.track_count = Some(count);
                        }
                    }
                }
            }
        }
    }

    playlist.thumbnails = parse_thumbnails_from_two_row(&renderer.thumbnail_renderer);

    Some(playlist)
}

fn parse_thumbnails_from_renderer(thumb: &Option<ThumbnailRenderer>) -> Thumbnails {
    thumb
        .as_ref()
        .and_then(|t| t.music_thumbnail_renderer.as_ref())
        .and_then(|m| m.thumbnail.as_ref())
        .and_then(|t| t.thumbnails.as_ref())
        .map(|thumbs| {
            Thumbnails::new(
                thumbs
                    .iter()
                    .map(|t| Thumbnail::new(&t.url, t.width.unwrap_or(0), t.height.unwrap_or(0)))
                    .collect(),
            )
        })
        .unwrap_or_default()
}

fn parse_thumbnails_from_two_row(thumb: &Option<ThumbnailRenderer>) -> Thumbnails {
    parse_thumbnails_from_renderer(thumb)
}

/// Parse a duration string like "3:45" or "1:23:45" into Duration.
fn parse_duration(s: &str) -> Duration {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("3:45").as_seconds(), 225);
        assert_eq!(parse_duration("1:23:45").as_seconds(), 5025);
        assert_eq!(parse_duration("0:30").as_seconds(), 30);
    }
}
