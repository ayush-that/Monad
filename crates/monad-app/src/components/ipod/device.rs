//! Main iPod device component.

use std::time::Duration;

use dioxus::prelude::*;

use super::{ClickWheel, Screen};
use crate::state::battery::BatteryState;
use crate::state::ipod::IPodState;

/// Main iPod device wrapper.
/// This creates the iconic iPod form factor with metallic body.
#[component]
pub fn IPodDevice() -> Element {
    // Initialize iPod navigation state
    let ipod_state = use_context_provider(IPodState::new);
    let theme = *ipod_state.theme.read();
    let theme_class = theme.css_class();

    // Initialize battery state
    let battery_state = use_context_provider(BatteryState::new);

    // Refresh battery every 30 seconds
    let mut battery_info = battery_state.info;
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if let Some(info) = crate::state::battery::get_battery_info() {
                battery_info.set(info);
            }
        }
    });

    rsx! {
        div { class: "ipod-device {theme_class}",
            // Metallic body background (handled by CSS)

            // Screen section (top)
            div { class: "ipod-device__screen-area",
                Screen {}
            }

            // Click wheel section (bottom)
            div { class: "ipod-device__wheel-area",
                ClickWheel {}
            }
        }
    }
}
