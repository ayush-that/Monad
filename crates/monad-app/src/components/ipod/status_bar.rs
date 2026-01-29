//! iPod status bar component.

use dioxus::prelude::*;

use crate::state::battery::BatteryState;
use crate::state::ipod::IPodState;
use crate::state::player::PlaybackStatus;
use crate::state::AppState;

/// Status bar at top of iPod screen.
/// Shows title, play indicator, and battery.
#[component]
pub fn StatusBar() -> Element {
    let app_state = use_context::<AppState>();
    let ipod_state = use_context::<IPodState>();
    let battery_state = use_context::<BatteryState>();

    let screen = *ipod_state.screen.read();
    let status = *app_state.player.status.read();
    let is_playing = status == PlaybackStatus::Playing;
    let is_paused = status == PlaybackStatus::Paused;

    let battery_info = *battery_state.info.read();
    let battery_percentage = battery_info.percentage;
    let is_charging = battery_info.is_charging;

    let title = screen.title();

    // Battery fill width based on actual percentage
    let battery_fill_style = format!("width: {battery_percentage}%");

    // Battery fill color: green when charging, white otherwise
    let battery_fill_class = if is_charging {
        "ipod-status-bar__battery-fill ipod-status-bar__battery-fill--charging"
    } else {
        "ipod-status-bar__battery-fill"
    };

    rsx! {
        div { class: "ipod-status-bar",
            // Left spacer (for symmetry)
            div { class: "ipod-status-bar__left" }

            // Center title
            span { class: "ipod-status-bar__title", "{title}" }

            // Right status icons
            div { class: "ipod-status-bar__right",
                // Play/Pause indicator
                if is_playing {
                    div { class: "ipod-status-bar__playing",
                        // Play icon (triangle)
                        svg {
                            width: "10",
                            height: "12",
                            view_box: "0 0 10 12",
                            fill: "white",
                            polygon { points: "0,0 10,6 0,12" }
                        }
                    }
                } else if is_paused {
                    div { class: "ipod-status-bar__playing",
                        // Pause icon (two bars)
                        svg {
                            width: "10",
                            height: "12",
                            view_box: "0 0 10 12",
                            fill: "white",
                            rect { x: "0", y: "0", width: "3", height: "12" }
                            rect { x: "7", y: "0", width: "3", height: "12" }
                        }
                    }
                }

                // Battery indicator (green fill when charging)
                div { class: "ipod-status-bar__battery",
                    div {
                        class: "{battery_fill_class}",
                        style: "{battery_fill_style}"
                    }
                }
            }
        }
    }
}
