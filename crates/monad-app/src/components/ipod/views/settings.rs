//! Settings view for iPod.

use dioxus::prelude::*;

use crate::state::ipod::{ColorTheme, IPodState};

/// Settings view with theme options.
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
