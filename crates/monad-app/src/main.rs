//! # Monad
//!
//! The fastest `YouTube` Music client, built with Rust and Dioxus.
//! Now with an iPod-inspired UI.

// RSX macros generate code that triggers these warnings incorrectly
#![allow(unused_qualifications)]
#![allow(clippy::use_self)]

mod components;
mod services;
mod state;

use anyhow::Result;
use components::IPodDevice;
use dioxus::desktop::tao::window::Icon;
use dioxus::desktop::{Config, WindowBuilder};
use dioxus::prelude::*;
use services::audio::{use_audio_event_sync, use_audio_service};
use state::AppState;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// iPod device dimensions (scaled to 80%)
const IPOD_WIDTH: f64 = 286.0;
const IPOD_HEIGHT: f64 = 560.0;

/// Load the app icon from embedded PNG.
fn load_icon() -> Option<Icon> {
    let icon_bytes = include_bytes!("../assets/icons/icon.png");
    let img = image::load_from_memory(icon_bytes).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Icon::from_rgba(img.into_raw(), width, height).ok()
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "monad=debug,monad_app=debug,monad_audio=info".into()),
        )
        .init();

    info!("Starting Monad v{}", env!("CARGO_PKG_VERSION"));

    // Load app icon
    let icon = load_icon();

    // Configure window to match iPod dimensions (fixed size, borderless)
    let mut window_builder = WindowBuilder::new()
        .with_title("Monad")
        .with_inner_size(dioxus::desktop::LogicalSize::new(IPOD_WIDTH, IPOD_HEIGHT))
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(true);

    if let Some(icon) = icon {
        window_builder = window_builder.with_window_icon(Some(icon));
    }

    let config = Config::new()
        .with_window(window_builder)
        .with_disable_context_menu(true)
        .with_menu(None);

    // Launch the Dioxus app with custom config
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(App);

    Ok(())
}

/// Main application component - iPod-style UI.
#[component]
fn App() -> Element {
    // Initialize global state
    let app_state = use_context_provider(AppState::new);

    // Initialize audio service
    let audio_service = use_audio_service();

    // Provide audio service to context for other components
    use_context_provider(|| audio_service);

    // Set up audio event synchronization
    use_audio_event_sync(audio_service, app_state.clone());

    rsx! {
        // Inject CSS
        style { {include_str!("../assets/styles.css")} }

        // iPod device interface
        div { class: "app",
            IPodDevice {}
        }
    }
}
