//! Desktop-only summary column: today's completion, the week's bars, and
//! the best streak across all habits. Hidden below 900px by CSS.

use dioxus::prelude::*;

use crate::store::Data;

pub fn sidebar(data: Signal<Data>) -> Element {
    let s = data().summary();
    #[allow(clippy::manual_checked_ops)]
    let pct = if s.total == 0 {
        0
    } else {
        (s.done * 100 + s.total / 2) / s.total
    };
    let left = s.total - s.done;
    let note = if s.total == 0 {
        "NO HABITS YET".to_string()
    } else if left == 0 {
        "ALL DONE".to_string()
    } else {
        format!("{left} LEFT BEFORE MIDNIGHT")
    };

    rsx! {
        aside { class: "side",
            div { class: "side-block",
                div { class: "side-label", "Completion" }
                div { class: "side-pct",
                    "{pct}"
                    span { "%" }
                }
                div { class: "side-note", "{note}" }
            }
            div { class: "side-block",
                div { class: "side-label", "This week" }
                div { class: "bars",
                    for (i , (day , frac)) in s.week.iter().enumerate() {
                        div {
                            key: "{day}",
                            class: if i == 6 { "today" } else { "" },
                            style: "height:{frac * 100.0:.0}%",
                        }
                    }
                }
                div { class: "bars-days",
                    for (i , (day , _)) in s.week.iter().enumerate() {
                        span {
                            key: "{day}",
                            class: if i == 6 { "today" } else { "" },
                            {day.format("%a").to_string()[..2].to_uppercase()}
                        }
                    }
                }
            }
            div { class: "side-block",
                div { class: "side-label", "Best streak" }
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
