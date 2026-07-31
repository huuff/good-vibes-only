//! The Today ledger: date header with the done/total meter, then the
//! habit list split into DUE TODAY and NOT DUE TODAY (design option 3b).
//! Each row: checkbox, name + schedule/progress line, a 14-day dot strip
//! (desktop only), and the streak numeral.

use chrono::Local;
use dioxus::prelude::*;

use super::Overlays;
use crate::store::{Data, Habit, Schedule};

/// One ledger row. `undue` rows render dimmed with either a dashed box
/// (nothing to do yet) or a filled dark check (period target already
/// met, just not today).
fn row(habit: Habit, mut data: Signal<Data>, mut overlays: Overlays, undue: bool) -> Element {
    let today = Local::now().date_naive();
    let done = habit.done_today();
    let (status, accent) = habit.status_on(today);
    let box_class = match (done, undue, habit.schedule) {
        (true, _, _) => "box done",
        (false, true, Schedule::EveryNDays { .. }) => "box ghost",
        (false, true, _) => "box period",
        (false, false, _) => "box",
    };
    let checked = done || box_class == "box period";
    rsx! {
        div {
            key: "{habit.id}",
            class: if undue { "row undue" } else { "row" },
            onclick: move |_| overlays.open_detail(habit.id),
            button {
                class: box_class,
                aria_pressed: done,
                aria_label: "Mark {habit.name} done today",
                title: "Done today — tap to toggle",
                onclick: move |e| {
                    e.stop_propagation();
                    data.with_mut(|d| {
                        d.toggle(habit.id, Local::now().date_naive());
                        d.save();
                    });
                },
                if checked {
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
                div { class: if accent { "note accent" } else { "note" }, "{status}" }
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
                class: if done || undue { "streak-n" } else { "streak-n dim" },
                "{habit.streak()}"
            }
        }
    }
}

pub fn ledger(data: Signal<Data>, overlays: Overlays) -> Element {
    let summary = data().summary();
    let today = Local::now().date_naive();
    let date = Local::now().format("%a %-d %b %Y").to_string();
    let pct = if summary.total == 0 {
        0.0
    } else {
        summary.done as f64 * 100.0 / summary.total as f64
    };
    let (due, later): (Vec<Habit>, Vec<Habit>) =
        data().habits.into_iter().partition(|h| h.due_on(today));
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
        if !due.is_empty() || !later.is_empty() {
            div { class: "col-head",
                span { class: "ch-box" }
                span { class: "ch-name", "HABIT" }
                span { class: "ch-days", "LAST 14 DAYS" }
                span { class: "ch-streak", "STREAK" }
            }
        }
        div { class: "ledger",
            if due.is_empty() && later.is_empty() {
                p { class: "empty",
                    "Nothing here yet. Tap + to add a habit, then tick it off each day you do it."
                }
            }
            if !later.is_empty() && !due.is_empty() {
                div { class: "sec-head", "DUE TODAY" }
            }
            for habit in due {
                {row(habit, data, overlays, false)}
            }
            if !later.is_empty() {
                div { class: "sec-head", "NOT DUE TODAY" }
            }
            for habit in later {
                {row(habit, data, overlays, true)}
            }
        }
    }
}
