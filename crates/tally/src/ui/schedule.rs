//! The HOW OFTEN? schedule picker (design option 3a): four choices —
//! every day, every N days, N times per week, N times in M days — with
//! always-visible inline −/+ steppers for the numbers (so it's obvious
//! at a glance they're adjustable) and a hint line explaining the pick.

use dioxus::prelude::*;

use crate::preferences::WeekStart;
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

    fn hint(self, week_start: WeekStart) -> String {
        let week_end = match week_start {
            WeekStart::Monday => "Sunday",
            WeekStart::Sunday => "Saturday",
        };
        match self.kind {
            Kind::Daily => "Tick it off every single day.".into(),
            Kind::EveryN => format!(
                "One check-in every {} days, whenever suits you.",
                self.every_n
            ),
            Kind::PerWeek if self.per_week == 1 => {
                format!(
                    "One check-in any day between {} and {} keeps the week.",
                    week_start.label(),
                    week_end
                )
            }
            Kind::PerWeek => format!(
                "Any {} check-ins between {} and {} count. The streak keeps \
                 going as long as each week hits its target.",
                self.per_week,
                week_start.label(),
                week_end
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

/// An inline −/+ stepper, clamped to `min..=max`. Adjusting a number
/// also selects its row (its clicks don't bubble to the row, so the
/// selection is set here).
fn num_stepper(
    mut draft: Signal<ScheduleDraft>,
    kind: Kind,
    value: u32,
    min: u32,
    max: u32,
    set: impl Fn(&mut ScheduleDraft, u32) + Copy + 'static,
) -> Element {
    rsx! {
        span { class: "num-step",
            button {
                aria_label: "Decrease",
                disabled: value <= min,
                onclick: move |e| {
                    e.stop_propagation();
                    draft.with_mut(|d| {
                        set(d, value.saturating_sub(1).max(min));
                        d.kind = kind;
                    });
                },
                "−"
            }
            span { class: "num-val", "{value}" }
            button {
                aria_label: "Increase",
                disabled: value >= max,
                onclick: move |e| {
                    e.stop_propagation();
                    draft.with_mut(|d| {
                        set(d, (value + 1).min(max));
                        d.kind = kind;
                    });
                },
                "+"
            }
        }
    }
}

/// One selectable option row. A div rather than a button so the inline
/// steppers (buttons) can nest inside.
fn option_row(mut draft: Signal<ScheduleDraft>, kind: Kind, label: Element) -> Element {
    let on = draft().kind == kind;
    rsx! {
        div {
            class: if on { "opt on" } else { "opt" },
            role: "radio",
            aria_checked: on,
            tabindex: "0",
            onclick: move |_| draft.with_mut(|d| d.kind = kind),
            onkeydown: move |e| {
                if e.key() == Key::Enter || e.key() == Key::Character(" ".into()) {
                    e.prevent_default();
                    draft.with_mut(|d| d.kind = kind);
                }
            },
            span { class: "opt-box",
                if on {
                    {check()}
                }
            }
            span { class: "opt-label", {label} }
        }
    }
}

pub fn schedule_picker(draft: Signal<ScheduleDraft>, week_start: WeekStart) -> Element {
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
                    {num_stepper(draft, Kind::EveryN, d.every_n, 2, 90, |d, v| d.every_n = v)}
                    " days"
                },
            )}
            {option_row(
                draft,
                Kind::PerWeek,
                rsx! {
                    {num_stepper(draft, Kind::PerWeek, d.per_week, 1, 7, |d, v| d.per_week = v)}
                    if d.per_week == 1 {
                        " time per week"
                    } else {
                        " times per week"
                    }
                },
            )}
            {option_row(
                draft,
                Kind::InDays,
                rsx! {
                    {num_stepper(draft, Kind::InDays, d.times, 1, d.window, |d, v| d.times = v)}
                    if d.times == 1 {
                        " time in "
                    } else {
                        " times in "
                    }
                    {num_stepper(
                        draft,
                        Kind::InDays,
                        d.window,
                        2,
                        90,
                        |d, v| {
                            d.window = v;
                            d.times = d.times.min(v);
                        },
                    )}
                    " days"
                },
            )}
        }
        div { class: "opt-hint", {d.hint(week_start)} }
    }
}
