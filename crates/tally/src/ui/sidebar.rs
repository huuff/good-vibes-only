//! Desktop-only summary column: today's completion, the week's bars, and
//! the best streak across all habits. Hidden below 900px by CSS.

use dioxus::prelude::*;

use crate::i18n::fill;
use crate::preferences::Preferences;
use crate::store::Data;

pub fn sidebar(data: Signal<Data>, preferences: Signal<Preferences>) -> Element {
    let s = data().summary_with_week_start(preferences().week_start);
    let lang = preferences().language;
    let t = lang.strings();
    let today = crate::clock::today();
    #[allow(clippy::manual_checked_ops)]
    let pct = if s.total == 0 {
        0
    } else {
        (s.done * 100 + s.total / 2) / s.total
    };
    let left = s.total - s.done;
    let note = if s.total == 0 {
        t.no_habits_yet.to_string()
    } else if left == 0 {
        t.all_done.to_string()
    } else {
        fill(t.left_before_midnight, &[&left])
    };

    rsx! {
        aside { class: "side",
            div { class: "side-block",
                div { class: "side-label", {t.completion} }
                div { class: "side-pct",
                    "{pct}"
                    span { "%" }
                }
                div { class: "side-note", "{note}" }
            }
            div { class: "side-block",
                div { class: "side-label", {t.this_week} }
                div { class: "bars",
                    for (day , frac) in &s.week {
                        div {
                            key: "{day}",
                            class: if *day == today { "today" } else { "" },
                            style: "height:{frac * 100.0:.0}%",
                        }
                    }
                }
                div { class: "bars-days",
                    for (day , _) in &s.week {
                        span {
                            key: "{day}",
                            class: if *day == today { "today" } else { "" },
                            {day.format_localized("%a", lang.locale())
                                .to_string()
                                .chars()
                                .take(2)
                                .collect::<String>()
                                .to_uppercase()}
                        }
                    }
                }
            }
            div { class: "side-block",
                div { class: "side-label", {t.best_streak} }
                if let Some((n, name)) = s.best {
                    div { class: "best",
                        span { class: "best-n", "{n}" }
                        span { class: "best-name", "{name}" }
                    }
                } else {
                    div { class: "best",
                        span { class: "best-name", "—" }
                    }
                }
            }
        }
    }
}
