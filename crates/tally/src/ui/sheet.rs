//! Overlay sheets: the per-habit detail (month calendar with binary
//! toggling inside the edit window, name/schedule editing, delete behind
//! a two-tap confirm) and the add-habit form with its schedule picker.

use chrono::{Datelike, Local, Months, NaiveDate};
use dioxus::prelude::*;

use super::Overlays;
use super::schedule::{ScheduleDraft, schedule_picker};
use crate::store::{Data, editable};

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

pub fn detail_sheet(mut data: Signal<Data>, mut overlays: Overlays) -> Element {
    let Some(id) = (overlays.detail)() else {
        return rsx! {};
    };
    let Some(habit) = data().habits.into_iter().find(|h| h.id == id) else {
        return rsx! {};
    };

    let today = Local::now().date_naive();
    let shown = (overlays.month)()
        .with_day(1)
        .expect("every month has a day 1");
    let this_month = today.with_day(1).expect("every month has a day 1");
    let name = habit.name.clone();

    let mut save = move || {
        if (overlays.name_draft)().trim().is_empty() {
            return;
        }
        data.with_mut(|d| {
            d.rename(id, &(overlays.name_draft)());
            d.set_schedule(id, (overlays.sched_draft)().schedule());
            d.save();
        });
        overlays.editing.set(false);
    };

    let day_cells: Vec<Element> = month_cells(shown)
        .into_iter()
        .map(|cell| {
            let Some(day) = cell else {
                return rsx! {
                    span { class: "cal-blank" }
                };
            };
            let done = habit.done_on(day);
            let mut cls = String::from("cal-day");
            if done {
                cls.push_str(" done");
            }
            if day == today {
                cls.push_str(" today");
            }
            if day > today {
                cls.push_str(" off");
            } else if !editable(day) {
                cls.push_str(" locked");
            }
            rsx! {
                button {
                    class: "{cls}",
                    disabled: day > today,
                    title: if editable(day) { "{day} — tap to toggle" } else { "{day}" },
                    onclick: move |_| {
                        if editable(day) {
                            data.with_mut(|d| {
                                d.toggle(id, day);
                                d.save();
                            });
                        }
                    },
                    "{day.day()}"
                }
            }
        })
        .collect();

    rsx! {
        div { class: "overlay", onclick: move |_| overlays.dismiss(),
            div {
                class: "sheet",
                role: "dialog",
                aria_modal: "true",
                tabindex: "-1",
                onclick: move |e| e.stop_propagation(),
                onmounted: move |e| async move {
                    let _ = e.data().set_focus(true).await;
                },
                onkeydown: move |e| {
                    if e.key() == Key::Escape {
                        overlays.dismiss();
                    }
                },
                div { class: "sheet-label", "Habit" }
                if (overlays.editing)() {
                    div { class: "form",
                        input {
                            class: "input",
                            value: "{overlays.name_draft}",
                            enterkeyhint: "done",
                            onmounted: move |e| async move {
                                let _ = e.data().set_focus(true).await;
                            },
                            oninput: move |e| overlays.name_draft.set(e.value()),
                            onkeydown: move |e| {
                                e.stop_propagation();
                                if e.key() == Key::Enter {
                                    save();
                                } else if e.key() == Key::Escape {
                                    overlays.editing.set(false);
                                }
                            },
                        }
                        {schedule_picker(overlays.sched_draft)}
                        button {
                            class: "btn",
                            disabled: (overlays.name_draft)().trim().is_empty(),
                            onclick: move |_| save(),
                            "SAVE"
                        }
                    }
                } else {
                    button {
                        class: "sheet-name",
                        title: "Edit name and schedule",
                        onclick: move |_| {
                            overlays.name_draft.set(name.clone());
                            overlays.sched_draft.set(ScheduleDraft::from_schedule(habit.schedule));
                            overlays.editing.set(true);
                        },
                        "{habit.name} ✎"
                    }
                    div { class: "sheet-stats",
                        "Streak {habit.streak()} · best {habit.best_streak()} · {habit.schedule.label()}"
                    }
                }
                div { class: "cal-nav",
                    button {
                        class: "btn-quiet",
                        title: "Earlier month",
                        onclick: move |_| overlays.month.set(shown - Months::new(1)),
                        "‹"
                    }
                    span { class: "cal-title", {shown.format("%B %Y").to_string()} }
                    button {
                        class: "btn-quiet",
                        title: "Later month",
                        disabled: shown >= this_month,
                        onclick: move |_| overlays.month.set(shown + Months::new(1)),
                        "›"
                    }
                }
                div { class: "cal-grid",
                    for wd in ["M", "T", "W", "T", "F", "S", "S"] {
                        span { class: "cal-wd", "{wd}" }
                    }
                    {day_cells.into_iter()}
                }
                div { class: "sheet-del",
                    if (overlays.confirm)() {
                        button {
                            class: "btn-quiet danger",
                            title: "Really delete",
                            onclick: move |_| {
                                overlays.dismiss();
                                data.with_mut(|d| {
                                    d.delete(id);
                                    d.save();
                                });
                            },
                            "SURE?"
                        }
                    } else {
                        button {
                            class: "btn-quiet",
                            title: "Delete habit",
                            onclick: move |_| overlays.confirm.set(true),
                            "DELETE HABIT"
                        }
                    }
                }
            }
        }
    }
}

pub fn add_sheet(mut data: Signal<Data>, mut overlays: Overlays) -> Element {
    if !(overlays.adding)() {
        return rsx! {};
    }

    let mut add = move || {
        let name = (overlays.name_draft)();
        if name.trim().is_empty() {
            return;
        }
        data.with_mut(|d| {
            d.add(&name, (overlays.sched_draft)().schedule());
            d.save();
        });
        overlays.dismiss();
    };

    rsx! {
        div { class: "overlay", onclick: move |_| overlays.dismiss(),
            div {
                class: "sheet",
                role: "dialog",
                aria_modal: "true",
                tabindex: "-1",
                onclick: move |e| e.stop_propagation(),
                onmounted: move |e| async move {
                    let _ = e.data().set_focus(true).await;
                },
                onkeydown: move |e| {
                    if e.key() == Key::Escape {
                        overlays.dismiss();
                    }
                },
                div { class: "sheet-label", "New habit" }
                div { class: "form",
                    input {
                        class: "input",
                        value: "{overlays.name_draft}",
                        placeholder: "Name it…",
                        enterkeyhint: "done",
                        onmounted: move |e| async move {
                            let _ = e.data().set_focus(true).await;
                        },
                        oninput: move |e| overlays.name_draft.set(e.value()),
                        onkeydown: move |e| {
                            e.stop_propagation();
                            if e.key() == Key::Enter {
                                add();
                            } else if e.key() == Key::Escape {
                                overlays.dismiss();
                            }
                        },
                    }
                    {schedule_picker(overlays.sched_draft)}
                    button {
                        class: "btn",
                        disabled: (overlays.name_draft)().trim().is_empty(),
                        onclick: move |_| add(),
                        "CREATE HABIT"
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
