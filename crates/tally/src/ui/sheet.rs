//! Overlay sheets: the per-habit detail (month calendar with binary
//! toggling inside the edit window, name/schedule editing, delete behind
//! a two-tap confirm) and the add-habit form with its schedule picker.

use chrono::{Datelike, Days, Months, NaiveDate};
use dioxus::prelude::*;

use super::Overlays;
use super::schedule::{ScheduleDraft, schedule_picker};
use crate::i18n::{Strings, fill};
use crate::preferences::{Language, Preferences, WeekStart};
use crate::store::{Data, editable};

/// The days of the month containing `month`, padded for the configured
/// first weekday so indices line up with a 7-column grid.
fn month_cells(month: NaiveDate, week_start: WeekStart) -> Vec<Option<NaiveDate>> {
    let first = month.with_day(1).expect("every month has a day 1");
    let first_index = first.weekday().num_days_from_monday();
    let week_index = week_start.weekday().num_days_from_monday();
    let mut cells = vec![None; ((first_index + 7 - week_index) % 7) as usize];
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

pub fn detail_sheet(
    mut data: Signal<Data>,
    mut overlays: Overlays,
    preferences: Signal<Preferences>,
) -> Element {
    let Some(id) = (overlays.detail)() else {
        return rsx! {};
    };
    let Some(habit) = data().habits.into_iter().find(|h| h.id == id) else {
        return rsx! {};
    };

    let today = crate::clock::today();
    let shown = (overlays.month)()
        .with_day(1)
        .expect("every month has a day 1");
    let this_month = today.with_day(1).expect("every month has a day 1");
    // Entering edit mode, shared by the name button and the schedule
    // chip — both edit the same form.
    let mut start_edit = {
        let name = habit.name.clone();
        let schedule = habit.schedule;
        move || {
            overlays.name_draft.set(name.clone());
            overlays
                .sched_draft
                .set(ScheduleDraft::from_schedule(schedule));
            overlays.target_draft.set(habit.sticking_target.max(1));
            overlays.editing.set(true);
        }
    };
    let mut start_edit2 = start_edit.clone();

    let mut save = move || {
        if (overlays.name_draft)().trim().is_empty() {
            return;
        }
        data.with_mut(|d| {
            d.rename(id, &(overlays.name_draft)());
            d.set_schedule(id, (overlays.sched_draft)().schedule());
            d.set_sticking_target(id, (overlays.target_draft)());
            d.save();
        });
        overlays.editing.set(false);
    };

    let week_start = preferences().week_start;
    let lang = preferences().language;
    let t = lang.strings();
    // First letter of each localized weekday name, starting from the
    // configured first day. The anchor is just any known Monday.
    let monday_anchor = NaiveDate::from_ymd_opt(2026, 1, 5).expect("valid date");
    let offset = u64::from(week_start.weekday().num_days_from_monday());
    let weekday_labels: Vec<String> = (0..7)
        .map(|i| {
            (monday_anchor + Days::new(offset + i))
                .format_localized("%a", lang.locale())
                .to_string()
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default()
        })
        .collect();
    let day_cells: Vec<Element> = month_cells(shown, week_start)
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
                    title: if editable(day) { fill(t.cal_day_toggle, &[&day]) } else { day.to_string() },
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
                div { class: "sheet-label", {t.habit_label} }
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
                        {schedule_picker(overlays.sched_draft, week_start, lang)}
                        {target_picker(overlays.target_draft, lang)}
                        button {
                            class: "btn",
                            disabled: (overlays.name_draft)().trim().is_empty(),
                            onclick: move |_| save(),
                            {t.save}
                        }
                    }
                } else {
                    button {
                        class: "sheet-name",
                        title: t.edit_name_schedule,
                        onclick: move |_| start_edit(),
                        "{habit.name} ✎"
                    }
                    button {
                        class: "sheet-sched",
                        title: t.change_schedule,
                        onclick: move |_| start_edit2(),
                        "{habit.schedule.label(lang)} ✎"
                    }
                    div { class: "sheet-stats",
                        {fill(
                            t.streak_best,
                            &[
                                &habit.streak_on_with_week_start(today, week_start),
                                &habit.best_streak_with_week_start(week_start),
                            ],
                        )}
                        " · "
                        {fill(t.strength, &[&((habit.strength_on(today) * 100.0).round() as u32)])}
                    }
                    div { class: "sheet-progress",
                        div { class: "sheet-progress-copy",
                            span { {t.building} }
                            strong {
                                {fill(t.reps, &[&habit.repetitions(), &habit.sticking_target.max(1)])}
                            }
                        }
                        div { class: "habit-progress large",
                            div { style: "width:{habit.sticking_progress() * 100.0:.2}%" }
                        }
                        p {
                            if habit.sticking_goal_reached() {
                                {t.milestone_reached}
                            } else {
                                {t.milestone_default}
                            }
                        }
                    }
                }
                div { class: "cal-nav",
                    button {
                        class: "btn-quiet",
                        title: t.earlier_month,
                        onclick: move |_| overlays.month.set(shown - Months::new(1)),
                        "‹"
                    }
                    span { class: "cal-title",
                        {shown.format_localized("%B %Y", lang.locale()).to_string()}
                    }
                    button {
                        class: "btn-quiet",
                        title: t.later_month,
                        disabled: shown >= this_month,
                        onclick: move |_| overlays.month.set(shown + Months::new(1)),
                        "›"
                    }
                }
                div { class: "cal-grid",
                    for wd in weekday_labels {
                        span { class: "cal-wd", "{wd}" }
                    }
                    {day_cells.into_iter()}
                }
                div { class: "sheet-del",
                    if (overlays.confirm)() {
                        button {
                            class: "btn-quiet danger",
                            title: t.really_delete,
                            onclick: move |_| {
                                overlays.dismiss();
                                data.with_mut(|d| {
                                    d.delete(id);
                                    d.save();
                                });
                            },
                            {t.sure}
                        }
                    } else {
                        button {
                            class: "btn-quiet",
                            title: t.delete_habit_title,
                            onclick: move |_| overlays.confirm.set(true),
                            {t.delete_habit}
                        }
                    }
                }
            }
        }
    }
}

pub fn add_sheet(
    mut data: Signal<Data>,
    mut overlays: Overlays,
    preferences: Signal<Preferences>,
) -> Element {
    if !(overlays.adding)() {
        return rsx! {};
    }
    let lang = preferences().language;
    let t = lang.strings();

    let mut add = move || {
        let name = (overlays.name_draft)();
        if name.trim().is_empty() {
            return;
        }
        data.with_mut(|d| {
            d.add(
                &name,
                (overlays.sched_draft)().schedule(),
                (overlays.target_draft)(),
            );
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
                div { class: "sheet-label", {t.new_habit_label} }
                div { class: "form",
                    input {
                        class: "input",
                        value: "{overlays.name_draft}",
                        placeholder: t.name_placeholder,
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
                    {schedule_picker(overlays.sched_draft, preferences().week_start, lang)}
                    {target_picker(overlays.target_draft, lang)}
                    button {
                        class: "btn",
                        disabled: (overlays.name_draft)().trim().is_empty(),
                        onclick: move |_| add(),
                        {t.create_habit}
                    }
                }
            }
        }
    }
}

fn target_picker(mut target: Signal<u32>, lang: Language) -> Element {
    let t: &'static Strings = lang.strings();
    rsx! {
        div { class: "target-picker",
            div { class: "target-copy",
                span { class: "how-label", {t.sticking_milestone} }
                span { class: "target-hint", {t.target_hint} }
            }
            div { class: "num-step target-step",
                button {
                    type: "button",
                    aria_label: t.decrease_milestone,
                    disabled: target() <= 1,
                    onclick: move |_| target.with_mut(|n| *n = n.saturating_sub(1).max(1)),
                    "−"
                }
                span { class: "num-val", "{target}" }
                button {
                    type: "button",
                    aria_label: t.increase_milestone,
                    onclick: move |_| target.with_mut(|n| *n = n.saturating_add(1).min(999)),
                    "+"
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
        let cells = month_cells(
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
            WeekStart::Monday,
        );
        assert_eq!(cells.len(), 2 + 31);
        assert!(cells[..2].iter().all(Option::is_none));
        assert_eq!(cells[2], NaiveDate::from_ymd_opt(2026, 7, 1));
        assert_eq!(cells[32], NaiveDate::from_ymd_opt(2026, 7, 31));

        // Sunday-first moves Wednesday to column three.
        let sunday = month_cells(
            NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
            WeekStart::Sunday,
        );
        assert_eq!(sunday[..3], [None, None, None]);
        assert_eq!(sunday[3], NaiveDate::from_ymd_opt(2026, 7, 1));
    }
}
