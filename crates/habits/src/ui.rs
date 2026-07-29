//! The app UI. State is a single [`Data`] signal, persisted to localStorage
//! after every mutation.

use chrono::{Datelike, Local, Months, NaiveDate};
use dioxus::prelude::*;

use crate::store::{Data, FORMATION_DAYS, editable};

/// Tracked through the dioxus asset system (not inlined in index.html) so
/// `dx serve` hot-reloads style edits without a rebuild.
static CSS: Asset = asset!("/assets/style.css");

/// Today's tap count: a plain number, dimmed em dash when still untouched.
fn today_count(count: usize) -> Element {
    rsx! {
        span {
            class: if count == 0 { "count dim" } else { "count" },
            title: "{count} today",
            if count == 0 {
                "—"
            } else {
                "{count}"
            }
        }
    }
}

/// The days of the month containing `month`, Monday-first, with leading
/// `None`s so indices line up with a 7-column grid.
fn month_cells(month: NaiveDate) -> Vec<Option<NaiveDate>> {
    let first = month.with_day(1).expect("every month has a day 1");
    let mut cells = vec![None; first.weekday().num_days_from_monday() as usize];
    let mut day = first;
    while day.month() == first.month() {
        cells.push(Some(day));
        match day.succ_opt() {
            Some(next) => day = next,
            None => break,
        }
    }
    cells
}

/// Month calendar for one habit: per-day tick counts as green shading, plus
/// a − / + stepper to correct the last [`crate::store::EDIT_WINDOW_DAYS`]
/// days. Older days are view-only.
fn calendar_sheet(
    mut data: Signal<Data>,
    mut open: Signal<Option<u64>>,
    mut month: Signal<NaiveDate>,
    mut sel: Signal<Option<NaiveDate>>,
) -> Element {
    let Some(id) = open() else {
        return rsx! {};
    };
    let Some(habit) = data().habits.into_iter().find(|h| h.id == id) else {
        return rsx! {};
    };
    let today = Local::now().date_naive();
    let shown = month().with_day(1).expect("every month has a day 1");
    let this_month = today.with_day(1).expect("every month has a day 1");

    let day_cells: Vec<Element> = month_cells(shown)
        .into_iter()
        .map(|cell| {
            let Some(day) = cell else {
                return rsx! {
                    span { class: "cal-blank" }
                };
            };
            let count = habit.ticks_on(day);
            let mut cls = String::from("cal-day");
            if count >= 3 {
                cls.push_str(" hot");
            }
            if day == today {
                cls.push_str(" today");
            }
            if sel() == Some(day) {
                cls.push_str(" sel");
            }
            if day > today {
                cls.push_str(" off");
            }
            let shade = if count > 0 {
                format!(
                    "background: rgba(156, 207, 143, {:.2})",
                    0.15 * count.min(4) as f64
                )
            } else {
                String::new()
            };
            rsx! {
                button {
                    class: "{cls}",
                    style: "{shade}",
                    disabled: day > today,
                    title: "{count} × {day}",
                    onclick: move |_| sel.set(Some(day)),
                    "{day.day()}"
                }
            }
        })
        .collect();

    let editor = match sel() {
        None => rsx! {},
        Some(day) => {
            let count = habit.ticks_on(day);
            rsx! {
                div { class: "cal-edit",
                    span { class: "cal-date", {day.format("%a %-d %b").to_string()} }
                    if editable(day) {
                        button {
                            class: "mini",
                            disabled: count == 0,
                            title: "One less that day",
                            onclick: move |_| {
                                data.with_mut(|d| {
                                    d.unrecord_on(id, day);
                                    d.save();
                                });
                            },
                            "−"
                        }
                        span { class: "cal-count", "{count}" }
                        button {
                            class: "mini",
                            title: "One more that day",
                            onclick: move |_| {
                                data.with_mut(|d| {
                                    d.record_on(id, day);
                                    d.save();
                                });
                            },
                            "+"
                        }
                    } else {
                        span { class: "cal-count", "{count}" }
                        span { class: "cal-lock", "view only" }
                    }
                }
            }
        }
    };

    rsx! {
        div { class: "overlay", onclick: move |_| open.set(None),
            div { class: "sheet", onclick: move |e| e.stop_propagation(),
                div { class: "handle" }
                h2 { "{habit.name}" }
                div { class: "cal-nav",
                    button {
                        class: "mini",
                        title: "Earlier month",
                        onclick: move |_| month.set(shown - Months::new(1)),
                        "‹"
                    }
                    span { class: "cal-title", {shown.format("%B %Y").to_string()} }
                    button {
                        class: "mini",
                        title: "Later month",
                        disabled: shown >= this_month,
                        onclick: move |_| month.set(shown + Months::new(1)),
                        "›"
                    }
                }
                div { class: "cal-grid",
                    for wd in ["M", "T", "W", "T", "F", "S", "S"] {
                        span { class: "cal-wd", "{wd}" }
                    }
                    {day_cells.into_iter()}
                }
                {editor}
            }
        }
    }
}

pub fn app() -> Element {
    let mut data = use_signal(Data::load);
    let mut adding = use_signal(|| false);
    let mut new_name = use_signal(String::new);
    let mut confirm_delete = use_signal(|| None::<u64>);
    let mut calendar = use_signal(|| None::<u64>);
    let mut cal_month = use_signal(|| Local::now().date_naive());
    let mut cal_day = use_signal(|| None::<NaiveDate>);

    let mut add = move || {
        let name = new_name();
        if name.trim().is_empty() {
            return;
        }
        data.with_mut(|d| {
            d.add(&name);
            d.save();
        });
        new_name.set(String::new());
        adding.set(false);
    };

    let today = Local::now().format("%A, %-d %B").to_string();

    rsx! {
        document::Stylesheet { href: CSS }
        div { class: "wrap",
            header {
                h1 { "Habits" }
                div { class: "head-right",
                    p { class: "date", "{today}" }
                    button {
                        class: "plus",
                        title: "New habit",
                        onclick: move |_| {
                            new_name.set(String::new());
                            adding.set(true);
                        },
                        "+"
                    }
                }
            }
            if data().habits.is_empty() && !adding() {
                p { class: "empty", "Nothing here yet. Tap + to add a habit, then tap it every time you do it." }
            }
            ul { class: "list",
                for habit in data().habits {
                    li { key: "{habit.id}",
                        div {
                            class: "card",
                            role: "button",
                            title: format!(
                                "habit strength {:.0} of ~{FORMATION_DAYS} days — grows each practiced day, fades a little over long breaks",
                                habit.strength(),
                            ),
                            onclick: move |_| {
                                confirm_delete.set(None);
                                data.with_mut(|d| {
                                    d.record(habit.id);
                                    d.save();
                                });
                            },
                            div { class: "card-top",
                                span { class: "name", "{habit.name}" }
                                {today_count(habit.today_count())}
                            }
                            div { class: "card-bottom",
                                div { class: "week",
                                    for (day , done) in habit.week() {
                                        span {
                                            class: if done { "dot done" } else { "dot" },
                                            title: "{day}",
                                        }
                                    }
                                }
                                if habit.streak() > 1 {
                                    span { class: "streak", "🔥 {habit.streak()}d" }
                                }
                                span { class: "total", "{habit.ticks.len()} total" }
                            }
                            div {
                                class: if habit.strength() >= FORMATION_DAYS as f64 { "root rooted" } else { "root" },
                                style: format!(
                                    "width:{:.1}%",
                                    habit.strength() * 100.0 / FORMATION_DAYS as f64,
                                ),
                            }
                        }
                        div { class: "actions",
                            button {
                                class: "mini",
                                title: "History calendar",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    let today = Local::now().date_naive();
                                    cal_month.set(today);
                                    cal_day.set(Some(today));
                                    calendar.set(Some(habit.id));
                                },
                                "▦"
                            }
                            if confirm_delete() == Some(habit.id) {
                                button {
                                    class: "mini danger",
                                    title: "Really delete",
                                    onclick: move |e| {
                                        e.stop_propagation();
                                        confirm_delete.set(None);
                                        data.with_mut(|d| {
                                            d.delete(habit.id);
                                            d.save();
                                        });
                                    },
                                    "sure?"
                                }
                            } else {
                                button {
                                    class: "mini",
                                    title: "Delete habit",
                                    onclick: move |e| {
                                        e.stop_propagation();
                                        confirm_delete.set(Some(habit.id));
                                    },
                                    "✕"
                                }
                            }
                        }
                    }
                }
            }
            button {
                class: "fab",
                title: "New habit",
                onclick: move |_| {
                    new_name.set(String::new());
                    adding.set(true);
                },
                "+"
            }
            {calendar_sheet(data, calendar, cal_month, cal_day)}
            if adding() {
                div { class: "overlay", onclick: move |_| adding.set(false),
                    div { class: "sheet", onclick: move |e| e.stop_propagation(),
                        div { class: "handle" }
                        h2 { "New habit" }
                        div { class: "add",
                            input {
                                value: "{new_name}",
                                placeholder: "Name it…",
                                enterkeyhint: "done",
                                onmounted: move |e| async move {
                                    let _ = e.data().set_focus(true).await;
                                },
                                oninput: move |e| new_name.set(e.value()),
                                onkeydown: move |e| {
                                    if e.key() == Key::Enter {
                                        add();
                                    } else if e.key() == Key::Escape {
                                        adding.set(false);
                                    }
                                },
                            }
                            button {
                                class: "add-btn",
                                disabled: new_name().trim().is_empty(),
                                onclick: move |_| add(),
                                "Add"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_cells_pad_to_weekday_columns() {
        // July 2026 starts on a Wednesday: two leading blanks, then 31 days.
        let cells = month_cells(NaiveDate::from_ymd_opt(2026, 7, 15).unwrap());
        assert_eq!(cells.len(), 2 + 31);
        assert!(cells[..2].iter().all(Option::is_none));
        assert_eq!(cells[2], NaiveDate::from_ymd_opt(2026, 7, 1));
        assert_eq!(cells[32], NaiveDate::from_ymd_opt(2026, 7, 31));
    }
}
