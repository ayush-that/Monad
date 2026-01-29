//! iPod screen component.

use dioxus::prelude::*;

use super::views::{ListView, MenuView, NowPlayingView, SearchView, SettingsView};
use super::StatusBar;
use crate::state::ipod::{IPodScreen, IPodState};

/// iPod LCD screen with status bar and content.
#[component]
pub fn Screen() -> Element {
    let ipod_state = use_context::<IPodState>();
    let screen = *ipod_state.screen.read();

    rsx! {
        div { class: "ipod-screen",
            // Black border
            div { class: "ipod-screen__content",
                // Status bar
                StatusBar {}

                // Screen content based on current view
                div { class: "ipod-screen__view",
                    match screen {
                        IPodScreen::NowPlaying => rsx! { NowPlayingView {} },
                        IPodScreen::Menu | IPodScreen::MusicMenu => rsx! { MenuView {} },
                        IPodScreen::Songs | IPodScreen::Artists |
                        IPodScreen::Albums | IPodScreen::Playlists => rsx! { ListView {} },
                        IPodScreen::Search => rsx! { SearchView {} },
                        IPodScreen::Settings => rsx! { SettingsView {} },
                    }
                }
            }

            // Glass reflection overlay
            div { class: "ipod-screen__glass" }
        }
    }
}
