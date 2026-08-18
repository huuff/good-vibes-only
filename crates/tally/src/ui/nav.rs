//! Navigation chrome: the desktop rail and mobile bottom bar.

use dioxus::prelude::*;

use super::{Overlays, Page};

pub fn rail(mut page: Signal<Page>, mut overlays: Overlays) -> Element {
    rsx! {
        div { class: "rail",
            div { class: "brand",
                "TALLY"
                span { class: "brand-dot", "." }
            }
            button {
                class: if page() == Page::Today { "rail-tab on" } else { "rail-tab" },
                onclick: move |_| page.set(Page::Today),
                "TODAY"
            }
            button {
                class: if page() == Page::Settings { "rail-tab on" } else { "rail-tab" },
                onclick: move |_| page.set(Page::Settings),
                "SETTINGS"
            }
            div { class: "rail-new",
                button { onclick: move |_| overlays.open_add(),
                    span { class: "plus-sign", "+" }
                    "NEW HABIT"
                }
            }
        }
    }
}

pub fn bottom_bar(mut page: Signal<Page>, mut overlays: Overlays) -> Element {
    rsx! {
        button {
            class: "fab-new",
            aria_label: "Create new habit",
            title: "Create new habit",
            onclick: move |_| overlays.open_add(),
            "+"
        }
        div { class: "bar",
            button {
                class: if page() == Page::Today { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Today),
                "TODAY"
            }
            button {
                class: if page() == Page::Settings { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Settings),
                "SETTINGS"
            }
        }
    }
}
