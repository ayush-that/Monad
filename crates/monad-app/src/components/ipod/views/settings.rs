//! Settings view for iPod.

use dioxus::prelude::*;

use crate::services::AudioService;
use crate::state::ipod::{ColorTheme, IPodState};
use crate::state::player::RepeatMode;
use crate::state::AppState;

/// Settings view with theme and playback options.
#[component]
pub fn SettingsView() -> Element {
    let ipod_state = use_context::<IPodState>();
    let current_theme = *ipod_state.theme.read();

    rsx! {
        div { class: "ipod-settings",
            // Theme Section
            div { class: "ipod-settings__section",
                div { class: "ipod-settings__header", "Color Theme" }
                div { class: "ipod-settings__list",
                    for theme in ColorTheme::all().iter() {
                        SettingsThemeItem {
                            key: "{theme.name()}",
                            theme: *theme,
                            is_current: *theme == current_theme,
                        }
                    }
                }
            }

            // Playback Section
            div { class: "ipod-settings__section",
                div { class: "ipod-settings__header", "Playback" }
                div { class: "ipod-settings__list",
                    ShuffleToggle {}
                    RepeatToggle {}
                }
            }

            // Volume Section
            div { class: "ipod-settings__section",
                div { class: "ipod-settings__header", "Volume" }
                div { class: "ipod-settings__list",
                    VolumeControl {}
                }
            }
        }
    }
}

/// Individual theme item in settings.
#[component]
fn SettingsThemeItem(theme: ColorTheme, is_current: bool) -> Element {
    let ipod_state = use_context::<IPodState>();
    let mut ipod_theme = ipod_state.theme;

    let name = theme.name();

    rsx! {
        div {
            class: "ipod-settings__item",
            onclick: move |_| {
                *ipod_theme.write() = theme;
            },
            div { class: "ipod-settings__item-content",
                span { class: "ipod-settings__color-preview ipod-settings__color-preview--{name.to_lowercase()}" }
                span { class: "ipod-settings__item-label", "{name}" }
            }
            if is_current {
                span { class: "ipod-settings__checkmark", "✓" }
            }
        }
    }
}

/// Shuffle toggle setting.
#[component]
fn ShuffleToggle() -> Element {
    let mut app_state = use_context::<AppState>();
    let shuffle = *app_state.player.shuffle.read();

    rsx! {
        div {
            class: "ipod-settings__item",
            onclick: move |_| {
                app_state.player.toggle_shuffle();
            },
            div { class: "ipod-settings__item-content",
                svg {
                    width: "20",
                    height: "20",
                    view_box: "0 0 24 24",
                    fill: if shuffle { "#4ade80" } else { "#666" },
                    path {
                        d: "M10.59 9.17L5.41 4 4 5.41l5.17 5.17 1.42-1.41zM14.5 4l2.04 2.04L4 18.59 5.41 20 17.96 7.46 20 9.5V4h-5.5zm.33 9.41l-1.41 1.41 3.13 3.13L14.5 20H20v-5.5l-2.04 2.04-3.13-3.13z"
                    }
                }
                span { class: "ipod-settings__item-label", "Shuffle" }
            }
            span { class: "ipod-settings__toggle-value",
                if shuffle { "On" } else { "Off" }
            }
        }
    }
}

/// Repeat mode toggle setting.
#[component]
fn RepeatToggle() -> Element {
    let mut app_state = use_context::<AppState>();
    let repeat = *app_state.player.repeat.read();

    let repeat_text = match repeat {
        RepeatMode::Off => "Off",
        RepeatMode::All => "All",
        RepeatMode::One => "One",
    };

    let icon_color = if repeat == RepeatMode::Off {
        "#666"
    } else {
        "#4ade80"
    };

    rsx! {
        div {
            class: "ipod-settings__item",
            onclick: move |_| {
                app_state.player.cycle_repeat();
            },
            div { class: "ipod-settings__item-content",
                svg {
                    width: "20",
                    height: "20",
                    view_box: "0 0 24 24",
                    fill: "{icon_color}",
                    path {
                        d: "M7 7h10v3l4-4-4-4v3H5v6h2V7zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4z"
                    }
                }
                span { class: "ipod-settings__item-label", "Repeat" }
            }
            span { class: "ipod-settings__toggle-value", "{repeat_text}" }
        }
    }
}

/// Volume control slider.
#[component]
fn VolumeControl() -> Element {
    let app_state = use_context::<AppState>();
    let audio = use_context::<Signal<AudioService>>();
    let volume = *app_state.player.volume.read();
    let muted = *app_state.player.muted.read();

    let display_volume = if muted { 0.0 } else { volume };
    let volume_percent = (display_volume * 100.0) as i32;

    // Get the signals we need to mutate
    let mut player_volume = app_state.player.volume;
    let mut player_muted = app_state.player.muted;

    rsx! {
        div { class: "ipod-settings__volume-control",
            // Mute toggle
            div {
                class: "ipod-settings__mute-btn",
                onclick: move |_| {
                    let is_muted = *player_muted.read();
                    *player_muted.write() = !is_muted;
                },
                svg {
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: if muted { "#ef4444" } else { "#fff" },
                    if muted {
                        path {
                            d: "M16.5 12c0-1.77-1.02-3.29-2.5-4.03v2.21l2.45 2.45c.03-.2.05-.41.05-.63zm2.5 0c0 .94-.2 1.82-.54 2.64l1.51 1.51C20.63 14.91 21 13.5 21 12c0-4.28-2.99-7.86-7-8.77v2.06c2.89.86 5 3.54 5 6.71zM4.27 3L3 4.27 7.73 9H3v6h4l5 5v-6.73l4.25 4.25c-.67.52-1.42.93-2.25 1.18v2.06c1.38-.31 2.63-.95 3.69-1.81L19.73 21 21 19.73l-9-9L4.27 3zM12 4L9.91 6.09 12 8.18V4z"
                        }
                    } else {
                        path {
                            d: "M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"
                        }
                    }
                }
            }

            // Volume slider
            div { class: "ipod-settings__volume-slider",
                // Decrease button
                button {
                    class: "ipod-settings__volume-btn",
                    onclick: move |_| {
                        let current = *player_volume.read();
                        let new_vol = (current - 0.1).max(0.0);
                        *player_volume.write() = new_vol;
                        audio.read().set_volume(new_vol);
                    },
                    "−"
                }

                // Volume bar
                div { class: "ipod-settings__volume-bar-container",
                    div { class: "ipod-settings__volume-bar",
                        div {
                            class: "ipod-settings__volume-fill",
                            style: "width: {display_volume * 100.0}%"
                        }
                    }
                    span { class: "ipod-settings__volume-text", "{volume_percent}%" }
                }

                // Increase button
                button {
                    class: "ipod-settings__volume-btn",
                    onclick: move |_| {
                        let current = *player_volume.read();
                        let new_vol = (current + 0.1).min(1.0);
                        *player_volume.write() = new_vol;
                        audio.read().set_volume(new_vol);
                    },
                    "+"
                }
            }
        }
    }
}
