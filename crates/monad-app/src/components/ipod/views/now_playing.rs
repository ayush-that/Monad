//! Now Playing view for iPod.

use dioxus::prelude::*;

use crate::state::player::PlaybackStatus;
use crate::state::AppState;

/// Now Playing view showing album art and track info.
#[component]
pub fn NowPlayingView() -> Element {
    let app_state = use_context::<AppState>();
    let current_track = app_state.player.current_track.read();
    let status = *app_state.player.status.read();

    // Check if buffering
    let is_buffering = status == PlaybackStatus::Buffering;

    // Clone track data to release borrow
    let track_data = current_track.as_ref().map(|t| {
        // Strip "Song, " or "Video, " prefix from artist display
        let artist = t.artists_display();
        let artist = artist
            .strip_prefix("Song, ")
            .or_else(|| artist.strip_prefix("Video, "))
            .unwrap_or(&artist)
            .to_string();
        (t.title.clone(), artist, t.hq_thumbnail_url())
    });

    rsx! {
        div { class: "ipod-now-playing",
            if let Some((title, artist, thumbnail)) = track_data {
                // Album artwork
                div { class: "ipod-now-playing__artwork",
                    if !thumbnail.is_empty() {
                        img {
                            src: "{thumbnail}",
                            alt: "{title}",
                        }
                    } else {
                        div { class: "ipod-now-playing__artwork-placeholder",
                            MusicIcon {}
                        }
                    }

                    // Buffering overlay
                    if is_buffering {
                        div { class: "ipod-now-playing__buffering",
                            div { class: "ipod-now-playing__buffering-spinner" }
                        }
                    }
                }

                // Song info overlay
                div { class: "ipod-now-playing__info",
                    h2 { class: "ipod-now-playing__title", "{title}" }
                    p { class: "ipod-now-playing__artist", "{artist}" }
                }
            } else {
                // No track playing
                div { class: "ipod-now-playing__artwork-placeholder",
                    MusicIcon {}
                }
                div { class: "ipod-now-playing__info",
                    h2 { class: "ipod-now-playing__title", "No Track" }
                    p { class: "ipod-now-playing__artist", "Select music to play" }
                }
            }
        }
    }
}

/// Music note icon.
#[component]
fn MusicIcon() -> Element {
    rsx! {
        svg {
            width: "60",
            height: "60",
            view_box: "0 0 24 24",
            fill: "#666",
            path {
                d: "M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"
            }
        }
    }
}
