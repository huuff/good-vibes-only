//! The Today ledger: date header with the done/total meter, then one
//! strong list of habit rows — checkbox, name + note, a 14-day dot strip
//! (desktop only), and the streak numeral.

use chrono::Local;
use dioxus::prelude::*;

use super::Overlays;
use crate::store::Data;

pub fn ledger(mut data: Signal<Data>, mut overlays: Overlays) -> Element {
    let summary = data().summary();
    let date = Local::now().format("%a %-d %b %Y").to_string();
    let pct = if summary.total == 0 {
        0.0
    } else {
        summary.done as f64 * 100.0 / summary.total as f64
    };
    rsx! {
        div { class: "head",
            div { class: "head-row",
                span { class: "head-date", "{date}" }
                if let Some(n) = summary.day_number {
                    span { class: "head-day", "DAY {n}" }
                }
            }
            h1 { class: "title", "Today" }
            div { class: "head-score",
                span { class: "score",
                    "{summary.done}"
                    span { class: "of", "/{summary.total}" }
                }
                div { class: "meter", div { style: "width:{pct:.0}%" } }
            }
        }
        if !data().habits.is_empty() {
            div { class: "col-head",
                span { class: "ch-box" }
                span { class: "ch-name", "HABIT" }
                span { class: "ch-days", "LAST 14 DAYS" }
                span { class: "ch-streak", "STREAK" }
            }
        }
        div { class: "ledger",
            if data().habits.is_empty() {
                p { class: "empty",
                    "Nothing here yet. Tap + to add a habit, then tick it off each day you do it."
                }
            }
            for habit in data().habits {
                div {
                    key: "{habit.id}",
                    class: "row",
                    onclick: move |_| overlays.open_detail(habit.id),
                    button {
                        class: if habit.done_today() { "box done" } else { "box" },
                        aria_pressed: habit.done_today(),
                        aria_label: "Mark {habit.name} done today",
                        title: "Done today — tap to toggle",
                        onclick: move |e| {
                            e.stop_propagation();
                            data.with_mut(|d| {
                                d.toggle(habit.id, Local::now().date_naive());
                                d.save();
                            });
                        },
                        if habit.done_today() {
                            svg {
                                width: "16",
                                height: "16",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "#f3f2f2",
                                stroke_width: "3.5",
                                path { d: "M4 12.5l5 5L20 6.5" }
                            }
                        }
                    }
                    button {
                        class: "row-name",
                        onclick: move |e| {
                            e.stop_propagation();
                            overlays.open_detail(habit.id)
                        },
                        div { class: "name", "{habit.name}" }
                        if !habit.note.is_empty() {
                            div { class: "note", "{habit.note}" }
                        }
                    }
                    div { class: "dots",
                        for (i , (day , done)) in habit.history(14).into_iter().enumerate() {
                            span {
                                class: match (done, i == 13) {
                                    (true, true) => "dot done today",
                                    (true, false) => "dot done",
                                    (false, true) => "dot today",
                                    (false, false) => "dot",
                                },
                                title: "{day}",
                            }
                        }
                    }
                    span {
                        class: if habit.done_today() { "streak-n" } else { "streak-n dim" },
                        "{habit.streak()}"
                    }
                }
            }
        }
    }
}
