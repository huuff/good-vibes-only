//! Navigation chrome: the desktop rail and mobile bottom bar.

use dioxus::prelude::*;

use super::{Overlays, Page};
use crate::preferences::Language;

pub fn rail(
    mut page: Signal<Page>,
    mut overlays: Overlays,
    lang: Language,
    rewards: bool,
) -> Element {
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
            if rewards {
                button {
                    class: if page() == Page::Rewards { "rail-tab on" } else { "rail-tab" },
                    onclick: move |_| page.set(Page::Rewards),
                    {t.rewards_tab}
                }
            }
            button {
                class: if page() == Page::Settings { "rail-tab on" } else { "rail-tab" },
                onclick: move |_| page.set(Page::Settings),
                {t.settings_tab}
            }
            if page() != Page::Settings {
                div { class: "rail-new",
                    button {
                        onclick: move |_| match page() {
                            Page::Todos => overlays.open_add_todo(),
                            Page::Rewards => overlays.open_add_reward(),
                            _ => overlays.open_add(),
                        },
                        span { class: "plus-sign", "+" }
                        match page() {
                            Page::Todos => rsx! { {t.new_todo} },
                            Page::Rewards => rsx! { {t.new_reward} },
                            _ => rsx! { {t.new_habit} },
                        }
                    }
                }
            }
        }
    }
}

pub fn bottom_bar(
    mut page: Signal<Page>,
    mut overlays: Overlays,
    lang: Language,
    rewards: bool,
) -> Element {
    let t = lang.strings();
    let fab_label = match page() {
        Page::Todos => t.create_new_todo,
        Page::Rewards => t.create_new_reward,
        _ => t.create_new_habit,
    };
    rsx! {
        if page() != Page::Settings {
            button {
                class: if page() == Page::Today { "fab-new" } else { "fab-new todo-fab" },
                aria_label: fab_label,
                title: fab_label,
                onclick: move |_| match page() {
                    Page::Todos => overlays.open_add_todo(),
                    Page::Rewards => overlays.open_add_reward(),
                    _ => overlays.open_add(),
                },
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
            if rewards {
                button {
                    class: if page() == Page::Rewards { "bar-tab on" } else { "bar-tab" },
                    onclick: move |_| page.set(Page::Rewards),
                    {t.rewards_tab}
                }
            }
            button {
                class: if page() == Page::Settings { "bar-tab on" } else { "bar-tab" },
                onclick: move |_| page.set(Page::Settings),
                {t.settings_tab}
            }
        }
    }
}
