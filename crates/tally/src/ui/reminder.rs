//! Habit reminders (design turn 15): the REMINDER row in the new-habit
//! form and the habit detail (15a, 15c), and the sheet both open — on/off,
//! time, weekday chips (15b), and the in-sheet notice when the system is
//! blocking notifications (15e). Hidden where reminders can't be
//! delivered ([`reminders::SUPPORTED`]).

use chrono::NaiveTime;
use dioxus::prelude::*;

use super::Overlays;
use crate::preferences::{Language, Preferences, WeekStart};
use crate::reminders::{self, Reminder, quiet_rule, week_days, weekday_name};
use crate::store::{Data, Schedule};

/// Which habit the reminder sheet edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReminderTarget {
    /// The habit being created in the add form (kept as a draft).
    NewHabit,
    Habit(u64),
}

/// "Remind me · 19:00 · MON – FRI ›", plus the quiet rule when on.
pub fn reminder_row(
    reminder: Option<Reminder>,
    schedule: Schedule,
    mut overlays: Overlays,
    target: ReminderTarget,
    week_start: WeekStart,
    lang: Language,
) -> Element {
    if !reminders::SUPPORTED {
        return rsx! {};
    }
    let t = lang.strings();
    let summary = reminder.map_or_else(
        || t.reminder_off.to_uppercase(),
        |r| r.summary(week_start, lang),
    );
    let on = reminder.is_some_and(|r| r.enabled);
    rsx! {
        div { class: "reminder-block",
            div { class: "how-label", {t.reminder_label} }
            button {
                class: "reminder-row",
                r#type: "button",
                onclick: move |_| overlays.open_reminder(target, reminder),
                span { class: "reminder-copy",
                    span { class: "reminder-name", {t.remind_me} }
                    span { class: "reminder-sum", "{summary}" }
                }
                span { class: "chev", aria_hidden: "true", "›" }
            }
            if on {
                div { class: "opt-hint", {quiet_rule(schedule, t)} }
            }
        }
    }
}

pub fn reminder_sheet(
    mut data: Signal<Data>,
    mut overlays: Overlays,
    preferences: Signal<Preferences>,
    allowed: Signal<bool>,
) -> Element {
    let Some(target) = (overlays.reminder)() else {
        return rsx! {};
    };
    let week_start = preferences().week_start;
    let lang = preferences().language;
    let t = lang.strings();
    let mut draft = overlays.reminder_draft;
    let d = draft();
    let schedule = match target {
        ReminderTarget::NewHabit => (overlays.sched_draft)().schedule(),
        ReminderTarget::Habit(id) => data()
            .habits
            .iter()
            .find(|h| h.id == id)
            .map(|h| h.schedule)
            .unwrap_or_default(),
    };
    let blocked = d.enabled && !allowed();
    let state = match (d.enabled, blocked) {
        (false, _) => t.reminder_off,
        (true, false) => t.reminder_on,
        (true, true) => t.reminder_paused,
    };
    let no_days = !d.days.contains(&true);

    let save = move |_| {
        let reminder = draft();
        match target {
            ReminderTarget::NewHabit => overlays.new_reminder.set(Some(reminder)),
            ReminderTarget::Habit(id) => data.with_mut(|d| {
                d.set_reminder(id, Some(reminder));
                d.save();
            }),
        }
        overlays.dismiss();
    };

    rsx! {
        div { class: "overlay", onclick: move |_| overlays.dismiss(),
            div {
                class: "sheet reminder-sheet",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "reminder-title",
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
                header { class: "rem-head",
                    h2 { id: "reminder-title", {t.reminder_title} }
                    button {
                        class: "rem-close",
                        aria_label: t.close_reminder,
                        onclick: move |_| overlays.dismiss(),
                        svg {
                            width: "18",
                            height: "18",
                            view_box: "0 0 24 24",
                            fill: "none",
                            stroke: "currentColor",
                            stroke_width: "2.5",
                            "aria-hidden": "true",
                            path { d: "M18 6L6 18M6 6l12 12" }
                        }
                    }
                }
                if blocked {
                    section { class: "rem-blocked", aria_label: t.notifications_off,
                        div { class: "rem-blocked-label", {t.notifications_off} }
                        p { {t.notifications_off_body} }
                        button {
                            class: "btn-quiet",
                            onclick: move |_| reminders::open_settings(),
                            {t.open_system_settings}
                        }
                    }
                }
                div { class: "rem-toggle",
                    div {
                        strong { {t.remind_me} }
                        span { class: "rem-state", {state} }
                    }
                    button {
                        class: if d.enabled { "theme-switch on" } else { "theme-switch" },
                        aria_label: t.reminder_enabled_aria,
                        aria_pressed: d.enabled,
                        onclick: move |_| {
                            let enabled = !draft().enabled;
                            draft.with_mut(|r| r.enabled = enabled);
                            if enabled && !reminders::notifications_allowed() {
                                reminders::request_permission();
                            }
                        },
                        span {}
                    }
                }
                label { class: if d.enabled { "rem-time" } else { "rem-time rem-off" },
                    span { class: "how-label", {t.reminder_time} }
                    input {
                        r#type: "time",
                        required: true,
                        disabled: !d.enabled,
                        value: "{d.time_label()}",
                        oninput: move |e| {
                            if let Ok(time) = NaiveTime::parse_from_str(&e.value(), "%H:%M") {
                                draft.with_mut(|r| r.time = time);
                            }
                        },
                    }
                }
                div { class: if d.enabled { "rem-days" } else { "rem-days rem-off" },
                    div { class: "how-label", {t.reminder_days} }
                    div {
                        class: "rem-chips",
                        role: "group",
                        aria_label: t.reminder_days_aria,
                        for day in week_days(week_start) {
                            button {
                                class: if d.on(day) { "rem-chip on" } else { "rem-chip" },
                                aria_label: weekday_name(day, "%A", lang),
                                aria_pressed: d.on(day),
                                disabled: !d.enabled,
                                onclick: move |_| draft.with_mut(|r| r.toggle_day(day)),
                                {
                                    weekday_name(day, "%a", lang)
                                        .chars()
                                        .next()
                                        .map(|c| c.to_uppercase().to_string())
                                        .unwrap_or_default()
                                }
                            }
                        }
                    }
                    if d.enabled {
                        div { class: "opt-hint", {quiet_rule(schedule, t)} }
                    }
                }
                button {
                    class: "btn rem-save",
                    disabled: d.enabled && no_days,
                    onclick: save,
                    {t.save_reminder}
                }
            }
        }
    }
}
