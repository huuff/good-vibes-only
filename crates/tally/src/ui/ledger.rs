//! The Today ledger: date header with the done/total meter, then the
//! habit list split into DUE and COMPLETED (design option 3b).
//! Each row: checkbox, name + schedule/progress line, a 14-day dot strip
//! (desktop only), and the streak numeral.

use dioxus::prelude::*;

use super::Overlays;
use crate::i18n::fill;
use crate::preferences::{Language, Preferences, WeekStart};
use crate::store::{Data, Habit, Schedule};

#[derive(Debug, PartialEq)]
struct RowAppearance {
    row_class: &'static str,
    box_class: &'static str,
    show_check: bool,
}

impl RowAppearance {
    fn for_day(habit: &Habit, day: chrono::NaiveDate, undue: bool) -> Self {
        let done = habit.done_on(day);
        let completed_week = undue && matches!(habit.schedule, Schedule::TimesPerWeek { .. });
        let box_class = match (done, undue, habit.schedule) {
            (true, _, _) => "box done",
            (false, true, Schedule::EveryNDays { .. }) => "box ghost",
            (false, true, Schedule::TimesPerWeek { .. }) => "box",
            (false, true, _) => "box period",
            (false, false, _) => "box",
        };
        Self {
            row_class: if completed_week {
                "row completed"
            } else if undue {
                "row undue"
            } else {
                "row"
            },
            box_class,
            show_check: done || box_class == "box period",
        }
    }
}

/// One ledger row. Completed weekly targets follow design 5a: the row
/// carries the completion treatment while today's checkbox stays empty.
fn row(
    habit: Habit,
    mut data: Signal<Data>,
    mut overlays: Overlays,
    undue: bool,
    week_start: WeekStart,
    lang: Language,
) -> Element {
    let t = lang.strings();
    let today = crate::clock::today();
    let done = habit.done_today();
    let (status, accent) = habit.status_on_with_week_start(today, week_start, lang);
    let repetitions = habit.repetitions();
    let target = habit.sticking_target.max(1);
    let progress = habit.sticking_progress() * 100.0;
    let appearance = RowAppearance::for_day(&habit, today, undue);
    rsx! {
        div {
            key: "{habit.id}",
            class: appearance.row_class,
            onclick: move |_| overlays.open_detail(habit.id),
            button {
                class: appearance.box_class,
                aria_pressed: done,
                aria_label: fill(t.mark_done, &[&habit.name]),
                title: t.done_today_hint,
                onclick: move |e| {
                    e.stop_propagation();
                    data.with_mut(|d| {
                        d.toggle(habit.id, crate::clock::today());
                        d.save();
                    });
                },
                if appearance.show_check {
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
                div {
                    class: if habit.sticking_goal_reached() { "habit-progress reached" } else { "habit-progress" },
                    title: fill(t.milestone_title, &[&repetitions, &target]),
                    div { style: "width:{progress:.2}%" }
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
                class: if done || undue { "streak-n" } else { "streak-n dim" },
                "{habit.streak_on_with_week_start(today, week_start)}"
            }
        }
    }
}

pub fn ledger(data: Signal<Data>, overlays: Overlays, preferences: Signal<Preferences>) -> Element {
    let week_start = preferences().week_start;
    let lang = preferences().language;
    let t = lang.strings();
    let summary = data().summary_with_week_start(week_start);
    let today = crate::clock::today();
    let date = today
        .format_localized("%a %-d %b %Y", lang.locale())
        .to_string();
    let pct = if summary.total == 0 {
        0.0
    } else {
        summary.done as f64 * 100.0 / summary.total as f64
    };
    let (due, later): (Vec<Habit>, Vec<Habit>) = data()
        .habits
        .into_iter()
        .partition(|h| h.due_on_with_week_start(today, week_start));
    rsx! {
        div { class: "head",
            div { class: "head-row",
                span { class: "head-date", "{date}" }
                if let Some(n) = summary.day_number {
                    span { class: "head-day", {fill(t.day_n, &[&n])} }
                }
            }
            h1 { class: "title", {t.today_title} }
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
                span { class: "ch-name", {t.col_habit} }
                span { class: "ch-days", {t.col_last14} }
                span { class: "ch-streak", {t.col_streak} }
            }
        }
        div { class: "ledger",
            if due.is_empty() && later.is_empty() {
                p { class: "empty", {t.empty_ledger} }
            }
            if !later.is_empty() && !due.is_empty() {
                div { class: "sec-head", {t.sec_due} }
            }
            for habit in due {
                {row(habit, data, overlays, false, week_start, lang)}
            }
            if !later.is_empty() {
                div { class: "sec-head", {t.sec_completed} }
            }
            for habit in later {
                {row(habit, data, overlays, true, week_start, lang)}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn weekly_target_met_uses_completed_row_with_actionable_empty_checkbox() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 31).unwrap();
        let habit = Habit {
            id: 1,
            name: "Review budget".into(),
            schedule: Schedule::TimesPerWeek { times: 1 },
            sticking_target: 30,
            days: BTreeSet::from([NaiveDate::from_ymd_opt(2026, 7, 29).unwrap()]),
        };

        let appearance = RowAppearance::for_day(&habit, today, true);

        assert_eq!(appearance.row_class, "row completed");
        assert_eq!(appearance.box_class, "box");
        assert!(!appearance.show_check);
    }
}
