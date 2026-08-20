//! App settings: appearance, the first day of the calendar week, and the
//! UI language.

use dioxus::prelude::*;

use crate::preferences::{Language, Preferences, WeekStart};

pub fn settings(mut preferences: Signal<Preferences>, system_dark: Signal<bool>) -> Element {
    let current = preferences();
    let t = current.language.strings();
    let dark = current.dark_mode.unwrap_or(system_dark());
    let appearance = match current.dark_mode {
        None if dark => t.system_dark,
        None => t.system_light,
        Some(true) => t.dark,
        Some(false) => t.light,
    };
    let week_label = |day: WeekStart| match day {
        WeekStart::Monday => t.monday,
        WeekStart::Sunday => t.sunday,
    };
    rsx! {
        div { class: "settings-screen",
            div { class: "settings-head",
                h1 { class: "title", {t.settings_title} }
            }
            div { class: "settings-body",
                div { class: "settings-label", {t.appearance} }
                div { class: "setting-row appearance-row",
                    div { class: "setting-copy",
                        strong { {t.dark_mode} }
                        span { "{appearance}" }
                    }
                    button {
                        class: if dark { "theme-switch on" } else { "theme-switch" },
                        aria_label: if dark { t.dark_mode_on } else { t.dark_mode_off },
                        aria_pressed: dark,
                        onclick: move |_| preferences.with_mut(|prefs| {
                            prefs.dark_mode = Some(!dark);
                            prefs.save();
                        }),
                        span {}
                    }
                }
                div { class: "settings-label preferences-label", {t.preferences_label} }
                label { class: "setting-row week-row",
                    strong { {t.week_starts_on} }
                    select {
                        aria_label: t.week_starts_on,
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
                                {week_label(day)}
                            }
                        }
                    }
                }
                label { class: "setting-row week-row",
                    strong { {t.language} }
                    select {
                        aria_label: t.language,
                        value: current.language.value(),
                        onchange: move |event| {
                            if let Some(lang) = Language::from_value(&event.value()) {
                                preferences.with_mut(|prefs| {
                                    prefs.language = lang;
                                    prefs.save();
                                });
                            }
                        },
                        for lang in Language::ALL {
                            option {
                                value: lang.value(),
                                selected: lang == current.language,
                                {lang.native_name()}
                            }
                        }
                    }
                }
            }
        }
    }
}
