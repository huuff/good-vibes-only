//! Navigation chrome: the desktop rail and mobile bottom bar.

use dioxus::prelude::*;

use super::{Overlays, Page};
use crate::preferences::Language;

pub fn rail(mut page: Signal<Page>, mut overlays: Overlays, lang: Language) -> Element {
    let t = lang.strings();
    rsx! {
        div { class: "rail",
            div { class: "brand",
                "TALLY"
                span { class: "brand-dot", "." }
            }
            button {
                class: if page() == Page::Today { "rail-tab on" } else { "rail-tab" },
                onclick: move |_| page.set(Page::Today),
                {t.habits_tab}
            }
            button {
                class: if page() == Page::Todos { "rail-tab on" } else { "rail-tab" },
                onclick: move |_| page.set(Page::Todos),
                {t.todos_tab}
            }
            button {
                class: if page() == Page::Settings { "rail-tab on" } else { "rail-tab" },
                onclick: move |_| page.set(Page::Settings),
                {t.settings_tab}
            }
            if page() != Page::Settings {
                div { class: "rail-new",
                    button { onclick: move |_| if page() == Page::Todos { overlays.open_add_todo() } else { overlays.open_add() },
                        span { class: "plus-sign", "+" }
                        if page() == Page::Todos { {t.new_todo} } else { {t.new_habit} }
                    }
                }
            }
        }
    }
}

pub fn bottom_bar(mut page: Signal<Page>, mut overlays: Overlays, lang: Language) -> Element {
    let t = lang.strings();
    rsx! {
        if page() != Page::Settings {
            button {
                class: if page() == Page::Todos { "fab-new todo-fab" } else { "fab-new" },
                aria_label: if page() == Page::Todos { t.create_new_todo } else { t.create_new_habit },
                title: if page() == Page::Todos { t.create_new_todo } else { t.create_new_habit },
                onclick: move |_| if page() == Page::Todos { overlays.open_add_todo() } else { overlays.open_add() },
                "+"
            }
        }
        div { class: "bar",
            button {
                class: if page() == Page::Today { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Today),
                {t.habits_tab}
            }
            button {
                class: if page() == Page::Todos { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Todos),
                {t.todos_tab}
            }
            button {
                class: if page() == Page::Settings { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Settings),
                {t.settings_tab}
            }
        }
    }
}
