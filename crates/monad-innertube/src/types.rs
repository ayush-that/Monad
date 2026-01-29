//! InnerTube-specific types and response structures.

use serde::{Deserialize, Serialize};

/// Search filter for narrowing results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchFilter {
    #[default]
    All,
    Songs,
    Videos,
    Albums,
    Artists,
    Playlists,
    CommunityPlaylists,
    FeaturedPlaylists,
}

impl SearchFilter {
    /// Get the params value for this filter.
    pub const fn params(&self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Songs => Some("EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D"),
            Self::Videos => Some("EgWKAQIQAWoKEAkQChAFEAMQBA%3D%3D"),
            Self::Albums => Some("EgWKAQIYAWoKEAkQChAFEAMQBA%3D%3D"),
            Self::Artists => Some("EgWKAQIgAWoKEAkQChAFEAMQBA%3D%3D"),
            Self::Playlists => Some("EgeKAQQoAEABagwQDhAKEAMQBRAJEAQ%3D"),
            Self::CommunityPlaylists => Some("EgeKAQQoAEABagwQDhAKEAkQBRADEAQ%3D"),
            Self::FeaturedPlaylists => Some("EgeKAQQoADgBagwQDhAKEAMQBRAJEAQ%3D"),
        }
    }
}

/// Search result containing different content types.
#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    pub songs: Vec<monad_core::Track>,
    pub videos: Vec<monad_core::Track>,
    pub albums: Vec<monad_core::Album>,
    pub artists: Vec<monad_core::types::ArtistPreview>,
    pub playlists: Vec<monad_core::Playlist>,
    pub continuation: Option<String>,
}

impl SearchResults {
    pub const fn is_empty(&self) -> bool {
        self.songs.is_empty()
            && self.videos.is_empty()
            && self.albums.is_empty()
            && self.artists.is_empty()
            && self.playlists.is_empty()
    }
}

/// Request body for `InnerTube` endpoints.
#[derive(Debug, Clone, Serialize)]
pub struct InnerTubeRequest<T> {
    pub context: crate::ClientContext,
    #[serde(flatten)]
    pub payload: T,
}

impl<T> InnerTubeRequest<T> {
    pub const fn new(context: crate::ClientContext, payload: T) -> Self {
        Self { context, payload }
    }
}

/// Search request payload.
#[derive(Debug, Clone, Serialize)]
pub struct SearchPayload {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
}

/// Browse request payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePayload {
    pub browse_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
}

/// Player request payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPayload {
    pub video_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlist_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_check_ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub racy_check_ok: Option<bool>,
}

/// Next (queue/related) request payload.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NextPayload {
    pub video_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlist_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlist_set_video_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_audio_only: Option<bool>,
}

/// Raw `InnerTube` response for search.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSearchResponse {
    pub contents: Option<SearchContents>,
    pub continuation_contents: Option<ContinuationContents>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchContents {
    pub tabbed_search_results_renderer: Option<TabbedSearchResultsRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabbedSearchResultsRenderer {
    pub tabs: Vec<SearchTab>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchTab {
    pub tab_renderer: Option<TabRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabRenderer {
    pub content: Option<TabContent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabContent {
    pub section_list_renderer: Option<SectionListRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionListRenderer {
    pub contents: Option<Vec<SectionContent>>,
    pub continuations: Option<Vec<Continuation>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionContent {
    pub music_shelf_renderer: Option<MusicShelfRenderer>,
    pub music_card_shelf_renderer: Option<MusicCardShelfRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicShelfRenderer {
    pub title: Option<TextRuns>,
    pub contents: Option<Vec<ShelfItem>>,
    pub continuations: Option<Vec<Continuation>>,
    pub bottom_endpoint: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicCardShelfRenderer {
    pub title: Option<TextRuns>,
    pub subtitle: Option<TextRuns>,
    pub thumbnail: Option<ThumbnailRenderer>,
    pub contents: Option<Vec<ShelfItem>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfItem {
    pub music_responsive_list_item_renderer: Option<MusicResponsiveListItemRenderer>,
    pub music_two_row_item_renderer: Option<MusicTwoRowItemRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicResponsiveListItemRenderer {
    pub flex_columns: Option<Vec<FlexColumn>>,
    pub fixed_columns: Option<Vec<FixedColumn>>,
    pub thumbnail: Option<ThumbnailRenderer>,
    pub overlay: Option<serde_json::Value>,
    pub navigation_endpoint: Option<NavigationEndpoint>,
    pub badges: Option<Vec<Badge>>,
    pub play_endpoint: Option<PlayEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicTwoRowItemRenderer {
    pub title: Option<TextRuns>,
    pub subtitle: Option<TextRuns>,
    pub thumbnail_renderer: Option<ThumbnailRenderer>,
    pub navigation_endpoint: Option<NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlexColumn {
    pub music_responsive_list_item_flex_column_renderer: Option<FlexColumnRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlexColumnRenderer {
    pub text: Option<TextRuns>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixedColumn {
    pub music_responsive_list_item_fixed_column_renderer: Option<FixedColumnRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixedColumnRenderer {
    pub text: Option<TextRuns>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRuns {
    pub runs: Option<Vec<TextRun>>,
}

impl TextRuns {
    pub fn text(&self) -> String {
        self.runs
            .as_ref()
            .map(|runs| runs.iter().map(|r| r.text.as_str()).collect::<String>())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextRun {
    pub text: String,
    pub navigation_endpoint: Option<NavigationEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailRenderer {
    pub music_thumbnail_renderer: Option<MusicThumbnailRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicThumbnailRenderer {
    pub thumbnail: Option<ThumbnailContainer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailContainer {
    pub thumbnails: Option<Vec<ThumbnailItem>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailItem {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigationEndpoint {
    pub browse_endpoint: Option<BrowseEndpoint>,
    pub watch_endpoint: Option<WatchEndpoint>,
    pub search_endpoint: Option<SearchEndpointData>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseEndpoint {
    pub browse_id: String,
    pub browse_endpoint_context_supported_configs: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchEndpoint {
    pub video_id: String,
    pub playlist_id: Option<String>,
    pub params: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchEndpointData {
    pub query: String,
    pub params: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayEndpoint {
    pub watch_endpoint: Option<WatchEndpoint>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Badge {
    pub music_inlined_badge_renderer: Option<MusicInlinedBadgeRenderer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicInlinedBadgeRenderer {
    pub icon: Option<Icon>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
    pub icon_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Continuation {
    pub next_continuation_data: Option<NextContinuationData>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextContinuationData {
    pub continuation: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuationContents {
    pub music_shelf_continuation: Option<MusicShelfContinuation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicShelfContinuation {
    pub contents: Option<Vec<ShelfItem>>,
    pub continuations: Option<Vec<Continuation>>,
}

/// Raw `InnerTube` response for player.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPlayerResponse {
    pub playability_status: Option<PlayabilityStatus>,
    pub streaming_data: Option<StreamingData>,
    pub video_details: Option<VideoDetails>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayabilityStatus {
    pub status: String,
    pub reason: Option<String>,
    pub playable_in_embed: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingData {
    pub formats: Option<Vec<Format>>,
    pub adaptive_formats: Option<Vec<Format>>,
    pub expires_in_seconds: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub itag: u32,
    pub url: Option<String>,
    pub signature_cipher: Option<String>,
    pub cipher: Option<String>,
    pub mime_type: String,
    pub bitrate: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub content_length: Option<String>,
    pub quality: Option<String>,
    pub audio_quality: Option<String>,
    pub audio_sample_rate: Option<String>,
    pub audio_channels: Option<u8>,
    pub approx_duration_ms: Option<String>,
}

impl Format {
    /// Check if this is an audio-only format.
    pub fn is_audio_only(&self) -> bool {
        self.mime_type.starts_with("audio/")
    }

    /// Get the content length as u64.
    pub fn content_length_u64(&self) -> Option<u64> {
        self.content_length.as_ref()?.parse().ok()
    }

    /// Get the sample rate as u32.
    pub fn sample_rate_u32(&self) -> Option<u32> {
        self.audio_sample_rate.as_ref()?.parse().ok()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoDetails {
    pub video_id: String,
    pub title: String,
    pub length_seconds: Option<String>,
    pub channel_id: Option<String>,
    pub author: Option<String>,
    pub thumbnail: Option<ThumbnailContainer>,
    pub view_count: Option<String>,
    pub is_live_content: Option<bool>,
}

/// Raw `InnerTube` response for browse.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawBrowseResponse {
    pub header: Option<serde_json::Value>,
    pub contents: Option<serde_json::Value>,
    pub continuation_contents: Option<serde_json::Value>,
}
