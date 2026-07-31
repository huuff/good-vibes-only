//! The HOW OFTEN? schedule picker (design option 3a): four choices —
//! every day, every N days, N times per week, N times in M days — with
//! −/+ steppers for the numbers and a hint line explaining the pick.

use dioxus::prelude::*;

use crate::store::Schedule;

/// Which picker row is selected. Kept separate from the numbers so
/// switching rows doesn't forget what was dialed in on another row.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Daily,
    EveryN,
    PerWeek,
    InDays,
}

/// Draft state behind the picker; collapses to a [`Schedule`] on save.
#[derive(Clone, Copy, PartialEq)]
pub struct ScheduleDraft {
    pub kind: Kind,
    pub every_n: u32,
    pub per_week: u32,
    pub times: u32,
    pub window: u32,
}

impl Default for ScheduleDraft {
    fn default() -> Self {
        Self {
            kind: Kind::Daily,
            every_n: 3,
            per_week: 2,
            times: 2,
            window: 5,
        }
    }
}

impl ScheduleDraft {
    pub fn from_schedule(s: Schedule) -> Self {
        let mut d = Self::default();
        match s {
            Schedule::Daily => d.kind = Kind::Daily,
            Schedule::EveryNDays { n } => {
                d.kind = Kind::EveryN;
                d.every_n = n;
            }
            Schedule::TimesPerWeek { times } => {
                d.kind = Kind::PerWeek;
                d.per_week = times;
            }
            Schedule::TimesInDays { times, days } => {
                d.kind = Kind::InDays;
                d.times = times;
                d.window = days;
            }
        }
        d
    }

    pub fn schedule(self) -> Schedule {
        match self.kind {
            Kind::Daily => Schedule::Daily,
            Kind::EveryN => Schedule::EveryNDays { n: self.every_n },
            Kind::PerWeek => Schedule::TimesPerWeek {
                times: self.per_week,
            },
            Kind::InDays => Schedule::TimesInDays {
                times: self.times,
                days: self.window,
            },
        }
    }

    fn hint(self) -> String {
        match self.kind {
            Kind::Daily => "Tick it off every single day.".into(),
            Kind::EveryN => format!(
                "One check-in every {} days, whenever suits you.",
                self.every_n
            ),
            Kind::PerWeek if self.per_week == 1 => {
                "One check-in any day between Monday and Sunday keeps the week.".into()
            }
            Kind::PerWeek => format!(
                "Any {} check-ins between Monday and Sunday count. The streak keeps \
                 going as long as each week hits its target.",
                self.per_week
            ),
            Kind::InDays if self.times == 1 => {
                format!("One check-in within any {} consecutive days.", self.window)
            }
            Kind::InDays => format!(
                "Any {} check-ins within any {} consecutive days.",
                self.times, self.window
            ),
        }
    }
}

/// The check mark drawn inside a selected option box.
fn check() -> Element {
    rsx! {
        svg {
            width: "13",
            height: "13",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "#f3f2f2",
            stroke_width: "3.5",
            path { d: "M4 12.5l5 5L20 6.5" }
        }
    }
}

/// A − value + row, clamped to `min..=max`.
fn stepper(
    label: &'static str,
    value: u32,
    min: u32,
    max: u32,
    set: impl FnMut(u32) + Clone + 'static,
) -> Element {
    let mut dec = set.clone();
    let mut inc = set;
    rsx! {
        div { class: "step",
            span { class: "step-label", "{label}" }
            div { class: "stepper",
                button {
                    aria_label: "Fewer {label}",
                    disabled: value <= min,
                    onclick: move |_| dec(value.saturating_sub(1).max(min)),
                    "−"
                }
                span { class: "step-val", "{value}" }
                button {
                    aria_label: "More {label}",
                    disabled: value >= max,
                    onclick: move |_| inc((value + 1).min(max)),
                    "+"
                }
            }
        }
    }
}

/// One selectable option row. The numeric panel, if any, is rendered by
/// the caller right below the selected row (buttons don't nest).
fn option_row(mut draft: Signal<ScheduleDraft>, kind: Kind, label: Element) -> Element {
    let on = draft().kind == kind;
    rsx! {
        button {
            class: if on { "opt on" } else { "opt" },
            role: "radio",
            aria_checked: on,
            onclick: move |_| draft.with_mut(|d| d.kind = kind),
            span { class: "opt-box",
                if on {
                    {check()}
                }
            }
            span { class: "opt-label", {label} }
        }
    }
}

pub fn schedule_picker(mut draft: Signal<ScheduleDraft>) -> Element {
    let d = draft();
    rsx! {
        div { class: "how-label", "HOW OFTEN?" }
        div { class: "opts", role: "radiogroup", aria_label: "How often",
            {option_row(draft, Kind::Daily, rsx! { "Every day" })}
            {option_row(
                draft,
                Kind::EveryN,
                rsx! {
                    "Every "
                    span { class: "opt-num", "{d.every_n}" }
                    " days"
                },
            )}
            if d.kind == Kind::EveryN {
                div { class: "opt-panel",
                    {stepper("DAYS", d.every_n, 2, 90, move |v| draft.with_mut(|d| d.every_n = v))}
                }
            }
            {option_row(
                draft,
                Kind::PerWeek,
                rsx! {
                    span { class: "opt-num", "{d.per_week}" }
                    if d.per_week == 1 {
                        " time per week"
                    } else {
                        " times per week"
                    }
                },
            )}
            if d.kind == Kind::PerWeek {
                div { class: "opt-panel",
                    {stepper("TIMES", d.per_week, 1, 7, move |v| draft.with_mut(|d| d.per_week = v))}
                }
            }
            {option_row(
                draft,
                Kind::InDays,
                rsx! {
                    span { class: "opt-num", "{d.times}" }
                    if d.times == 1 {
                        " time in "
                    } else {
                        " times in "
                    }
                    span { class: "opt-num", "{d.window}" }
                    " days"
                },
            )}
            if d.kind == Kind::InDays {
                div { class: "opt-panel",
                    {stepper(
                        "TIMES",
                        d.times,
                        1,
                        d.window,
                        move |v| draft.with_mut(|d| d.times = v),
                    )}
                    {stepper(
                        "DAYS",
                        d.window,
                        2,
                        90,
                        move |v| {
                            draft
                                .with_mut(|d| {
                                    d.window = v;
                                    d.times = d.times.min(v);
                                })
                        },
                    )}
                }
            }
        }
        div { class: "opt-hint", {d.hint()} }
    }
}
