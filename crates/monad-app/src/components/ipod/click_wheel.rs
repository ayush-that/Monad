//! iPod click wheel component.

use dioxus::prelude::*;

use crate::services::AudioService;
use crate::state::ipod::{IPodScreen, IPodState};
use crate::state::player::PlaybackStatus;
use crate::state::AppState;

/// The iconic iPod click wheel with control buttons.
#[component]
pub fn ClickWheel() -> Element {
    rsx! {
        div { class: "ipod-wheel",
            // Outer ring
            div { class: "ipod-wheel__ring",
                // MENU button (top)
                MenuButton {}

                // Previous button (left)
                PreviousButton {}

                // Next button (right)
                NextButton {}

                // Play/Pause button (bottom)
                PlayPauseButton {}

                // Center select button
                SelectButton {}
            }
        }
    }
}

/// MENU button at top of wheel.
#[component]
fn MenuButton() -> Element {
    let mut ipod_state = use_context::<IPodState>();

    rsx! {
        button {
            class: "ipod-wheel__btn ipod-wheel__btn--menu",
            onclick: move |_| {
                let screen = *ipod_state.screen.read();
                if screen == IPodScreen::NowPlaying {
                    ipod_state.navigate(IPodScreen::Menu);
                } else {
                    ipod_state.go_back();
                }
            },
            "MENU"
        }
    }
}

/// Previous track button (left).
#[component]
fn PreviousButton() -> Element {
    let mut app_state = use_context::<AppState>();
    let mut ipod_state = use_context::<IPodState>();
    let audio = use_context::<Signal<AudioService>>();

    rsx! {
        button {
            class: "ipod-wheel__btn ipod-wheel__btn--prev",
            onclick: move |_| {
                let screen = *ipod_state.screen.read();

                match screen {
                    IPodScreen::NowPlaying => {
                        // Previous track
                        app_state.previous_track();
                        if let Some(track) = app_state.player.current_track.read().as_ref() {
                            let track = track.clone();
                            *app_state.player.status.write() = PlaybackStatus::Buffering;
                            let audio = audio;
                            spawn(async move {
                                audio.read().play_track(&track).await;
                            });
                        }
                    }
                    IPodScreen::Menu | IPodScreen::MusicMenu |
                    IPodScreen::Songs | IPodScreen::Artists |
                    IPodScreen::Albums | IPodScreen::Playlists => {
                        // Move selection up
                        ipod_state.select_previous();
                    }
                    _ => {}
                }
            },
            // Previous icon (double left arrow)
            svg {
                width: "20",
                height: "14",
                view_box: "0 0 20 14",
                fill: "#666",
                // First arrow
                polygon { points: "10,0 10,14 0,7" }
                // Second arrow
                polygon { points: "20,0 20,14 10,7" }
            }
        }
    }
}

/// Next track button (right).
#[component]
fn NextButton() -> Element {
    let mut app_state = use_context::<AppState>();
    let mut ipod_state = use_context::<IPodState>();
    let audio = use_context::<Signal<AudioService>>();

    rsx! {
        button {
            class: "ipod-wheel__btn ipod-wheel__btn--next",
            onclick: move |_| {
                let screen = *ipod_state.screen.read();
                let max_items = screen.menu_items().len();

                match screen {
                    IPodScreen::NowPlaying => {
                        // Next track
                        app_state.next_track();
                        if let Some(track) = app_state.player.current_track.read().as_ref() {
                            let track = track.clone();
                            *app_state.player.status.write() = PlaybackStatus::Buffering;
                            let audio = audio;
                            spawn(async move {
                                audio.read().play_track(&track).await;
                            });
                        }
                    }
                    IPodScreen::Menu | IPodScreen::MusicMenu => {
                        // Move selection down
                        ipod_state.select_next(max_items);
                    }
                    IPodScreen::Songs | IPodScreen::Artists |
                    IPodScreen::Albums | IPodScreen::Playlists => {
                        // For list views, we'll use queue length later
                        ipod_state.select_next(100);
                    }
                    _ => {}
                }
            },
            // Next icon (double right arrow)
            svg {
                width: "20",
                height: "14",
                view_box: "0 0 20 14",
                fill: "#666",
                // First arrow
                polygon { points: "0,0 10,7 0,14" }
                // Second arrow
                polygon { points: "10,0 20,7 10,14" }
            }
        }
    }
}

/// Play/Pause button at bottom.
#[component]
fn PlayPauseButton() -> Element {
    let mut app_state = use_context::<AppState>();
    let audio = use_context::<Signal<AudioService>>();

    let status = *app_state.player.status.read();

    rsx! {
        button {
            class: "ipod-wheel__btn ipod-wheel__btn--play",
            onclick: move |_| {
                match status {
                    PlaybackStatus::Playing => {
                        audio.read().pause();
                        app_state.player.pause();
                    }
                    PlaybackStatus::Paused => {
                        audio.read().play();
                        app_state.player.play();
                    }
                    _ => {
                        if let Some(track) = app_state.player.current_track.read().as_ref() {
                            let track = track.clone();
                            *app_state.player.status.write() = PlaybackStatus::Buffering;
                            let audio = audio;
                            spawn(async move {
                                audio.read().play_track(&track).await;
                            });
                        }
                    }
                }
            },
            // Play/Pause icon
            svg {
                width: "24",
                height: "14",
                view_box: "0 0 24 14",
                fill: "#666",
                // Play triangle
                polygon { points: "0,0 8,7 0,14" }
                // Pause bars
                rect { x: "12", y: "0", width: "4", height: "14" }
                rect { x: "20", y: "0", width: "4", height: "14" }
            }
        }
    }
}

/// Center select button.
#[component]
fn SelectButton() -> Element {
    let mut app_state = use_context::<AppState>();
    let mut ipod_state = use_context::<IPodState>();
    let audio = use_context::<Signal<AudioService>>();

    rsx! {
        button {
            class: "ipod-wheel__center",
            onclick: move |_| {
                let screen = *ipod_state.screen.read();

                match screen {
                    IPodScreen::NowPlaying => {
                        // Toggle play/pause
                        let status = *app_state.player.status.read();
                        match status {
                            PlaybackStatus::Playing => {
                                audio.read().pause();
                                app_state.player.pause();
                            }
                            PlaybackStatus::Paused => {
                                audio.read().play();
                                app_state.player.play();
                            }
                            _ => {
                                if let Some(track) = app_state.player.current_track.read().as_ref() {
                                    let track = track.clone();
                                    *app_state.player.status.write() = PlaybackStatus::Buffering;
                                    let audio = audio;
                                    spawn(async move {
                                        audio.read().play_track(&track).await;
                                    });
                                }
                            }
                        }
                    }
                    IPodScreen::Menu | IPodScreen::MusicMenu => {
                        // Select menu item
                        ipod_state.select();
                    }
                    IPodScreen::Songs => {
                        // Play selected song (will implement later)
                    }
                    _ => {}
                }
            },
        }
    }
}
