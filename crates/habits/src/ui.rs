//! The app UI. State is a single [`Data`] signal, persisted to localStorage
//! after every mutation.

use chrono::Local;
use dioxus::prelude::*;

use crate::store::{Data, FORMATION_DAYS};

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

pub fn app() -> Element {
    let mut data = use_signal(Data::load);
    let mut adding = use_signal(|| false);
    let mut new_name = use_signal(String::new);
    let mut confirm_delete = use_signal(|| None::<u64>);

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
                                title: "Undo today's last tap",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    data.with_mut(|d| {
                                        d.undo(habit.id);
                                        d.save();
                                    });
                                },
                                "↩"
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
