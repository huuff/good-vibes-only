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
                "HABITS"
            }
            button {
                class: if page() == Page::Todos { "rail-tab on" } else { "rail-tab" },
                onclick: move |_| page.set(Page::Todos),
                "TODOS"
            }
            button {
                class: if page() == Page::Settings { "rail-tab on" } else { "rail-tab" },
                onclick: move |_| page.set(Page::Settings),
                "SETTINGS"
            }
            if page() != Page::Settings {
                div { class: "rail-new",
                    button { onclick: move |_| if page() == Page::Todos { overlays.open_add_todo() } else { overlays.open_add() },
                        span { class: "plus-sign", "+" }
                        if page() == Page::Todos { "NEW TODO" } else { "NEW HABIT" }
                    }
                }
            }
        }
    }
}

pub fn bottom_bar(mut page: Signal<Page>, mut overlays: Overlays) -> Element {
    rsx! {
        if page() != Page::Settings {
            button {
                class: if page() == Page::Todos { "fab-new todo-fab" } else { "fab-new" },
                aria_label: if page() == Page::Todos { "Create new todo" } else { "Create new habit" },
                title: if page() == Page::Todos { "Create new todo" } else { "Create new habit" },
                onclick: move |_| if page() == Page::Todos { overlays.open_add_todo() } else { overlays.open_add() },
                "+"
            }
        }
        div { class: "bar",
            button {
                class: if page() == Page::Today { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Today),
                "HABITS"
            }
            button {
                class: if page() == Page::Todos { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Todos),
                "TODOS"
            }
            button {
                class: if page() == Page::Settings { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Settings),
                "SETTINGS"
            }
        }
    }
}
