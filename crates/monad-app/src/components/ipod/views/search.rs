//! Search view for iPod.

use dioxus::prelude::*;
use monad_core::Track;
use monad_innertube::{InnerTubeClient, SearchFilter, SearchResults};
use tracing::info;

use crate::services::AudioService;
use crate::state::ipod::{IPodScreen, IPodState};
use crate::state::player::PlaybackStatus;
use crate::state::AppState;

/// Search view with input and categorized results.
#[component]
pub fn SearchView() -> Element {
    let mut query = use_signal(String::new);
    let mut results = use_signal(SearchResults::default);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    rsx! {
        div { class: "ipod-search",
            // Search input
            div { class: "ipod-search__input-container",
                input {
                    class: "ipod-search__input",
                    r#type: "text",
                    placeholder: "Search...",
                    value: "{query}",
                    oninput: move |evt| {
                        query.set(evt.value().clone());
                    },
                    onkeydown: move |evt| {
                        if evt.key() == Key::Enter {
                            let q = query.read().clone();
                            if !q.is_empty() {
                                spawn(async move {
                                    *loading.write() = true;
                                    *error.write() = None;

                                    match perform_search(&q).await {
                                        Ok(search_results) => {
                                            *results.write() = search_results;
                                        }
                                        Err(e) => {
                                            *error.write() = Some(e.to_string());
                                        }
                                    }

                                    *loading.write() = false;
                                });
                            }
                        }
                    },
                }
            }

            // Results area
            div { class: "ipod-search__results",
                if *loading.read() {
                    div { class: "ipod-search__loading", "Searching..." }
                } else if let Some(err) = error.read().as_ref() {
                    div { class: "ipod-search__error", "Error: {err}" }
                } else if results.read().is_empty() {
                    div { class: "ipod-search__empty",
                        if query.read().is_empty() {
                            "Type to search"
                        } else {
                            "No results"
                        }
                    }
                } else {
                    // Songs category
                    if !results.read().songs.is_empty() {
                        div { class: "ipod-search__category",
                            div { class: "ipod-search__category-header", "Songs" }
                            for track in results.read().songs.iter() {
                                TrackItem {
                                    key: "{track.id}",
                                    track: track.clone(),
                                }
                            }
                        }
                    }

                    // Videos category
                    if !results.read().videos.is_empty() {
                        div { class: "ipod-search__category",
                            div { class: "ipod-search__category-header", "Videos" }
                            for track in results.read().videos.iter() {
                                TrackItem {
                                    key: "{track.id}",
                                    track: track.clone(),
                                }
                            }
                        }
                    }

                    // Albums category
                    if !results.read().albums.is_empty() {
                        div { class: "ipod-search__category",
                            div { class: "ipod-search__category-header", "Albums" }
                            for album in results.read().albums.iter() {
                                div { class: "ipod-search__item ipod-search__item--album",
                                    div { class: "ipod-search__item-title", "{album.title}" }
                                    div { class: "ipod-search__item-artist",
                                        if let Some(year) = album.year {
                                            "{year}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Artists category
                    if !results.read().artists.is_empty() {
                        div { class: "ipod-search__category",
                            div { class: "ipod-search__category-header", "Artists" }
                            for artist in results.read().artists.iter() {
                                div { class: "ipod-search__item ipod-search__item--artist",
                                    div { class: "ipod-search__item-title", "{artist.name}" }
                                    if let Some(subs) = &artist.subscriber_count {
                                        div { class: "ipod-search__item-artist", "{subs} subscribers" }
                                    }
                                }
                            }
                        }
                    }

                    // Playlists category
                    if !results.read().playlists.is_empty() {
                        div { class: "ipod-search__category",
                            div { class: "ipod-search__category-header", "Playlists" }
                            for playlist in results.read().playlists.iter() {
                                div { class: "ipod-search__item ipod-search__item--playlist",
                                    div { class: "ipod-search__item-title", "{playlist.title}" }
                                    if let Some(count) = playlist.track_count {
                                        div { class: "ipod-search__item-artist", "{count} tracks" }
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

/// Perform search using InnerTube.
async fn perform_search(query: &str) -> Result<SearchResults, monad_core::Error> {
    info!("Performing search for: {}", query);
    let client = InnerTubeClient::new()?;
    let results = client.search(query, SearchFilter::All).await?;
    info!(
        "Search returned: {} songs, {} videos, {} albums, {} artists, {} playlists",
        results.songs.len(),
        results.videos.len(),
        results.albums.len(),
        results.artists.len(),
        results.playlists.len()
    );
    Ok(results)
}

/// Playable track item (song or video).
#[component]
fn TrackItem(track: Track) -> Element {
    let mut app_state = use_context::<AppState>();
    let mut ipod_state = use_context::<IPodState>();
    let audio = use_context::<Signal<AudioService>>();

    let title = track.title.clone();
    let artist = track.artists_display();

    rsx! {
        div {
            class: "ipod-search__item",
            onclick: move |_| {
                info!("Track clicked: {} - {}", track.title, track.artists_display());

                let track = track.clone();

                // Set current track and navigate to Now Playing immediately
                app_state.player.set_track(Some(track.clone()));
                *app_state.player.status.write() = PlaybackStatus::Buffering;
                *ipod_state.screen.write() = IPodScreen::NowPlaying;

                // Start playback using tokio spawn
                info!("Starting playback for track: {}", track.id);
                let audio_service = audio.read().clone();
                tokio::spawn(async move {
                    audio_service.play_track(&track).await;
                });
            },
            div { class: "ipod-search__item-title", "{title}" }
            div { class: "ipod-search__item-artist", "{artist}" }
        }
    }
}
