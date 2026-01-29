//! Search endpoint implementation.

use monad_core::{Error, Result};
use tracing::debug;

use crate::{
    parser::parse_search_results,
    types::{InnerTubeRequest, RawSearchResponse, SearchFilter, SearchPayload, SearchResults},
    InnerTubeClient,
};

impl InnerTubeClient {
    /// Search `YouTube` Music for content.
    ///
    /// # Arguments
    /// * `query` - The search query string
    /// * `filter` - Optional filter to narrow results to a specific content type
    ///
    /// # Returns
    /// Search results containing songs, videos, albums, artists, and playlists.
    pub async fn search(&self, query: &str, filter: SearchFilter) -> Result<SearchResults> {
        let payload = SearchPayload {
            query: query.to_string(),
            params: filter.params().map(String::from),
            continuation: None,
        };

        let request = InnerTubeRequest::new(self.context.clone(), payload);

        let response: RawSearchResponse = self
            .post("search", &request)
            .await
            .map_err(|e| Error::InnerTube(format!("Search request failed: {e}")))?;

        // Debug: dump raw response structure
        debug!(
            "Raw search response contents present: {}",
            response.contents.is_some()
        );
        if let Some(contents) = &response.contents {
            debug!(
                "Has tabbed_search_results_renderer: {}",
                contents.tabbed_search_results_renderer.is_some()
            );
            if let Some(tabbed) = &contents.tabbed_search_results_renderer {
                debug!("Number of tabs: {}", tabbed.tabs.len());
                for (i, tab) in tabbed.tabs.iter().enumerate() {
                    if let Some(tab_renderer) = &tab.tab_renderer {
                        debug!(
                            "Tab {}: content present: {}",
                            i,
                            tab_renderer.content.is_some()
                        );
                        if let Some(content) = &tab_renderer.content {
                            debug!(
                                "Tab {} section_list_renderer present: {}",
                                i,
                                content.section_list_renderer.is_some()
                            );
                            if let Some(section_list) = &content.section_list_renderer {
                                if let Some(sections) = &section_list.contents {
                                    debug!("Tab {} has {} sections", i, sections.len());
                                    for (j, section) in sections.iter().enumerate() {
                                        debug!("  Section {}: music_shelf_renderer: {}, music_card_shelf_renderer: {}",
                                            j,
                                            section.music_shelf_renderer.is_some(),
                                            section.music_card_shelf_renderer.is_some()
                                        );
                                        if let Some(shelf) = &section.music_shelf_renderer {
                                            let title = shelf
                                                .title
                                                .as_ref()
                                                .map(|t| t.text())
                                                .unwrap_or_default();
                                            let item_count = shelf
                                                .contents
                                                .as_ref()
                                                .map(|c| c.len())
                                                .unwrap_or(0);
                                            debug!(
                                                "    Shelf '{}' has {} items",
                                                title, item_count
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(parse_search_results(&response))
    }

    /// Continue a search with a continuation token.
    pub async fn search_continue(&self, continuation: &str) -> Result<SearchResults> {
        let payload = SearchPayload {
            query: String::new(),
            params: None,
            continuation: Some(continuation.to_string()),
        };

        let request = InnerTubeRequest::new(self.context.clone(), payload);

        let response: RawSearchResponse = self
            .post("search", &request)
            .await
            .map_err(|e| Error::InnerTube(format!("Search continuation failed: {e}")))?;

        Ok(parse_search_results(&response))
    }
}
