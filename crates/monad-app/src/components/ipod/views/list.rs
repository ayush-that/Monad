//! List view for iPod (songs, artists, albums, playlists).

use dioxus::prelude::*;

use crate::state::ipod::{IPodScreen, IPodState};
use crate::state::AppState;

/// List view for browsing songs, artists, albums, etc.
#[component]
pub fn ListView() -> Element {
    let app_state = use_context::<AppState>();
    let ipod_state = use_context::<IPodState>();
    let screen = *ipod_state.screen.read();
    let selected = *ipod_state.menu_index.read();

    // Get items based on screen type
    match screen {
        IPodScreen::Songs => {
            let queue = app_state.queue.read();
            let items: Vec<_> = queue
                .items()
                .iter()
                .map(|item| (item.track.title.clone(), item.track.artists_display()))
                .collect();
            drop(queue);

            rsx! {
                div { class: "ipod-list",
                    if items.is_empty() {
                        div { class: "ipod-list__empty",
                            "No songs in queue"
                        }
                    } else {
                        for (index, (title, artist)) in items.iter().enumerate() {
                            div {
                                key: "{index}",
                                class: if index == selected {
                                    "ipod-list__item ipod-list__item--selected"
                                } else {
                                    "ipod-list__item"
                                },
                                div { class: "ipod-list__title", "{title}" }
                                div { class: "ipod-list__subtitle", "{artist}" }
                            }
                        }
                    }
                }
            }
        }
        IPodScreen::Artists => {
            rsx! {
                div { class: "ipod-list",
                    div { class: "ipod-list__empty",
                        "Artists coming soon"
                    }
                }
            }
        }
        IPodScreen::Albums => {
            rsx! {
                div { class: "ipod-list",
                    div { class: "ipod-list__empty",
                        "Albums coming soon"
                    }
                }
            }
        }
        IPodScreen::Playlists => {
            rsx! {
                div { class: "ipod-list",
                    div { class: "ipod-list__empty",
                        "Playlists coming soon"
                    }
                }
            }
        }
        _ => {
            rsx! {
                div { class: "ipod-list" }
            }
        }
    }
}
