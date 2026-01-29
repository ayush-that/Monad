//! Now Playing view for iPod.

use dioxus::document::eval;
use dioxus::prelude::*;
use monad_lyrics::{Lyrics, LyricsClient};
use tracing::{debug, info};

use crate::state::player::PlaybackStatus;
use crate::state::AppState;

/// Now Playing view showing album art and track info.
#[component]
pub fn NowPlayingView() -> Element {
    let app_state = use_context::<AppState>();
    let current_track = app_state.player.current_track.read();
    let status = *app_state.player.status.read();
    let position = *app_state.player.position.read();

    // State for toggling between artwork and lyrics
    let mut show_lyrics = use_signal(|| false);

    // State for fetched lyrics
    let mut lyrics: Signal<Option<Lyrics>> = use_signal(|| None);
    let mut lyrics_error: Signal<Option<String>> = use_signal(|| None);
    let mut lyrics_loading = use_signal(|| false);

    // Track the last track ID we fetched lyrics for
    let mut last_track_id: Signal<Option<String>> = use_signal(|| None);

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
        (t.id.clone(), t.title.clone(), artist, t.hq_thumbnail_url())
    });

    // Fetch lyrics when track changes
    if let Some((ref track_id, ref title, ref artist, _)) = track_data {
        let should_fetch = last_track_id.read().as_ref() != Some(track_id);

        if should_fetch {
            *last_track_id.write() = Some(track_id.clone());
            *lyrics.write() = None;
            *lyrics_error.write() = None;
            *lyrics_loading.write() = true;

            let title = title.clone();
            let artist = artist.clone();

            spawn(async move {
                info!("Fetching lyrics for: {} - {}", artist, title);
                let client = LyricsClient::new();

                match client.fetch(&artist, &title, None, None).await {
                    Ok(fetched_lyrics) => {
                        info!("Got {} lyric lines", fetched_lyrics.lines.len());
                        *lyrics.write() = Some(fetched_lyrics);
                        *lyrics_error.write() = None;
                    }
                    Err(e) => {
                        debug!("Failed to fetch lyrics: {}", e);
                        *lyrics_error.write() = Some("Lyrics not available".to_string());
                    }
                }
                *lyrics_loading.write() = false;
            });
        }
    }

    rsx! {
        div { class: "ipod-now-playing",
            if let Some((_, title, artist, thumbnail)) = track_data {
                // Clickable area to toggle between artwork and lyrics
                div {
                    class: "ipod-now-playing__content",
                    onclick: move |_| {
                        let current = *show_lyrics.read();
                        *show_lyrics.write() = !current;
                    },

                    if *show_lyrics.read() {
                        // Lyrics view
                        LyricsView {
                            lyrics: lyrics.read().clone(),
                            loading: *lyrics_loading.read(),
                            error: lyrics_error.read().clone(),
                            position: position,
                        }
                    } else {
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

/// Lyrics display component.
#[component]
fn LyricsView(
    lyrics: Option<Lyrics>,
    loading: bool,
    error: Option<String>,
    position: f64,
) -> Element {
    // Track the last scrolled-to index to avoid excessive scrolling
    let mut last_scroll_index: Signal<Option<usize>> = use_signal(|| None);

    if loading {
        return rsx! {
            div { class: "ipod-lyrics ipod-lyrics--loading",
                div { class: "ipod-lyrics__message", "Loading lyrics..." }
            }
        };
    }

    if let Some(err) = error {
        return rsx! {
            div { class: "ipod-lyrics ipod-lyrics--error",
                div { class: "ipod-lyrics__message", "{err}" }
            }
        };
    }

    let Some(lyrics) = lyrics else {
        return rsx! {
            div { class: "ipod-lyrics ipod-lyrics--empty",
                div { class: "ipod-lyrics__message", "No lyrics" }
            }
        };
    };

    // Find the current line index
    let current_index = lyrics.line_index_at(position);

    // Auto-scroll to current lyric when it changes
    if current_index != *last_scroll_index.read() {
        *last_scroll_index.write() = current_index;

        if let Some(idx) = current_index {
            // Use eval to scroll the element into view
            let scroll_script = format!(
                r#"
                const el = document.getElementById('lyric-line-{idx}');
                if (el) {{
                    el.scrollIntoView({{ behavior: 'smooth', block: 'center' }});
                }}
                "#
            );
            spawn(async move {
                let _ = eval(&scroll_script).await;
            });
        }
    }

    let lines = &lyrics.lines;

    rsx! {
        div { class: "ipod-lyrics",
            // Spacer to allow first line to be centered
            div { class: "ipod-lyrics__spacer" }

            for (i, line) in lines.iter().enumerate() {
                {
                    let is_current = current_index == Some(i);
                    let is_past = current_index.map_or(false, |idx| i < idx);
                    let class = if is_current {
                        "ipod-lyrics__line ipod-lyrics__line--current"
                    } else if is_past {
                        "ipod-lyrics__line ipod-lyrics__line--past"
                    } else {
                        "ipod-lyrics__line"
                    };

                    rsx! {
                        div {
                            key: "{i}",
                            id: "lyric-line-{i}",
                            class: "{class}",
                            "{line.text}"
                        }
                    }
                }
            }

            // Spacer to allow last line to be centered
            div { class: "ipod-lyrics__spacer" }
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
