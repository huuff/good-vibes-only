//! App settings: appearance and the first day of the calendar week.

use dioxus::prelude::*;

use crate::preferences::{Preferences, WeekStart};

pub fn settings(mut preferences: Signal<Preferences>) -> Element {
    let current = preferences();
    rsx! {
        div { class: "settings-screen",
            div { class: "settings-head",
                h1 { class: "title", "Settings" }
            }
            div { class: "settings-body",
                div { class: "settings-label", "APPEARANCE" }
                div { class: "setting-row appearance-row",
                    div { class: "setting-copy",
                        strong { "Dark mode" }
                        span { if current.dark_mode { "Dark" } else { "Light" } }
                    }
                    button {
                        class: if current.dark_mode { "theme-switch on" } else { "theme-switch" },
                        aria_label: if current.dark_mode { "Dark mode enabled" } else { "Dark mode disabled" },
                        aria_pressed: current.dark_mode,
                        onclick: move |_| preferences.with_mut(|prefs| {
                            prefs.dark_mode = !prefs.dark_mode;
                            prefs.save();
                        }),
                        span {}
                    }
                }
                div { class: "settings-label preferences-label", "PREFERENCES" }
                label { class: "setting-row week-row",
                    strong { "Week starts on" }
                    select {
                        aria_label: "Week starts on",
                        value: current.week_start.value(),
                        onchange: move |event| {
                            if let Some(day) = WeekStart::from_value(&event.value()) {
                                preferences.with_mut(|prefs| {
                                    prefs.week_start = day;
                                    prefs.save();
                                });
                            }
                        },
                        for day in WeekStart::ALL {
                            option {
                                value: day.value(),
                                selected: day == current.week_start,
                                "{day.label()}"
                            }
                        }
                    }
                }
            }
        }
    }
}
