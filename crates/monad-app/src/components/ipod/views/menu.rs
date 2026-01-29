//! Menu view for iPod.

use dioxus::prelude::*;

use crate::state::ipod::IPodState;

/// Classic iPod menu list.
#[component]
pub fn MenuView() -> Element {
    let ipod_state = use_context::<IPodState>();
    let screen = *ipod_state.screen.read();
    let selected = *ipod_state.menu_index.read();
    let items = screen.menu_items();

    rsx! {
        div { class: "ipod-menu",
            for (index, item) in items.iter().enumerate() {
                MenuItem {
                    key: "{item.label}",
                    label: item.label,
                    target: item.target,
                    selected: index == selected,
                    index: index,
                }
            }
        }
    }
}

use crate::state::ipod::IPodScreen;

/// Individual menu item component.
#[component]
fn MenuItem(label: &'static str, target: IPodScreen, selected: bool, index: usize) -> Element {
    let ipod_state = use_context::<IPodState>();

    // Clone signals for use in closures
    let mut screen = ipod_state.screen;
    let mut history = ipod_state.history;
    let mut menu_index = ipod_state.menu_index;
    let mut menu_index_hover = ipod_state.menu_index;

    rsx! {
        div {
            class: if selected {
                "ipod-menu__item ipod-menu__item--selected"
            } else {
                "ipod-menu__item"
            },
            onclick: move |_| {
                // Navigate to target
                let current = *screen.read();
                history.write().push(current);
                *screen.write() = target;
                *menu_index.write() = 0;
            },
            onmouseenter: move |_| {
                *menu_index_hover.write() = index;
            },
            span { class: "ipod-menu__label", "{label}" }
            span { class: "ipod-menu__arrow", ">" }
        }
    }
}
