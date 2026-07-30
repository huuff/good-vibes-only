//! Navigation chrome: the desktop rail and the mobile bottom bar. Only
//! TODAY is live — HABITS / STATS / SETTINGS exist in the design but have
//! no screens yet, so they render muted and inert.

use dioxus::prelude::*;

use super::Overlays;

const TABS: [&str; 3] = ["TODAY", "HABITS", "STATS"];

pub fn rail(mut overlays: Overlays) -> Element {
    rsx! {
        div { class: "rail",
            div { class: "brand",
                "TALLY"
                span { class: "brand-dot", "." }
            }
            for tab in TABS {
                span {
                    class: if tab == "TODAY" { "rail-tab on" } else { "rail-tab" },
                    aria_disabled: tab != "TODAY",
                    "{tab}"
                }
            }
            span { class: "rail-tab", aria_disabled: true, "SETTINGS" }
            div { class: "rail-new",
                button { onclick: move |_| overlays.open_add(),
                    span { class: "plus-sign", "+" }
                    "NEW HABIT"
                }
            }
        }
    }
}

pub fn bottom_bar(mut overlays: Overlays) -> Element {
    rsx! {
        div { class: "bar",
            for tab in TABS {
                span {
                    class: if tab == "TODAY" { "bar-tab on" } else { "bar-tab" },
                    aria_disabled: tab != "TODAY",
                    "{tab}"
                }
            }
            button {
                class: "bar-new",
                title: "New habit",
                onclick: move |_| overlays.open_add(),
                "+"
            }
        }
    }
}
