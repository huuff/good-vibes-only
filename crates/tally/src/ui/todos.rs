//! Date-grouped Todos screen and its add sheet.

use chrono::{Datelike, NaiveDate, NaiveTime};
use dioxus::prelude::*;

use super::Overlays;
use crate::clock;
use crate::todos::{Todo, TodoData};

pub fn todos(data: Signal<TodoData>) -> Element {
    let today = clock::today();
    let snapshot = data();
    let mut due: Vec<_> = snapshot
        .todos
        .iter()
        .filter(|todo| todo.target_date.is_some_and(|date| date <= today))
        .cloned()
        .collect();
    let mut later: Vec<_> = snapshot
        .todos
        .iter()
        .filter(|todo| todo.target_date.is_some_and(|date| date > today))
        .cloned()
        .collect();
    let mut anytime: Vec<_> = snapshot
        .todos
        .iter()
        .filter(|todo| todo.target_date.is_none())
        .cloned()
        .collect();
    due.sort_by_key(|todo| (todo.done, todo.target_time, todo.id));
    later.sort_by_key(|todo| (todo.done, todo.target_date, todo.target_time, todo.id));
    anytime.sort_by_key(|todo| (todo.done, todo.id));

    rsx! {
        section { class: "todos-screen",
            header { class: "todos-head",
                span { class: "head-date", {today.format("%a %-d %b %Y").to_string()} }
                h1 { class: "title", "Todos" }
            }
            div { class: "todo-list",
                if snapshot.todos.is_empty() {
                    div { class: "empty todo-empty",
                        strong { "Nothing on your list." }
                        span { "Add a todo and give it a date, or leave it open-ended." }
                    }
                } else {
                    {todo_group("TODAY", due, data, today)}
                    {todo_group("LATER", later, data, today)}
                    {todo_group("ANYTIME", anytime, data, today)}
                }
            }
        }
    }
}

fn todo_group(
    label: &'static str,
    todos: Vec<Todo>,
    mut data: Signal<TodoData>,
    today: NaiveDate,
) -> Element {
    if todos.is_empty() {
        return rsx! {};
    }
    rsx! {
        section { class: "todo-group",
            h2 { class: "todo-group-label", {label} }
            for todo in todos {
                button {
                    key: "{todo.id}",
                    class: if todo.done { "todo-row done" } else { "todo-row" },
                    aria_label: if todo.done { format!("Mark {} incomplete", todo.name) } else { format!("Mark {} complete", todo.name) },
                    onclick: move |_| { data.write().toggle(todo.id); data().save(); },
                    span { class: if todo.done { "todo-box done" } else { "todo-box" },
                        if todo.done {
                            svg { view_box: "0 0 24 24", path { d: "M4 12.5l5 5L20 6.5" } }
                        }
                    }
                    span { class: "todo-copy",
                        strong { class: "todo-name", {todo.name.clone()} }
                        if let Some(meta) = todo_meta(&todo, today) { span { class: "todo-meta", {meta} } }
                    }
                }
            }
        }
    }
}

fn todo_meta(todo: &Todo, today: NaiveDate) -> Option<String> {
    let date = todo.target_date?;
    let date_label = if date == today {
        "TODAY".to_string()
    } else if date.year() == today.year() {
        date.format("%a %-d %b").to_string().to_uppercase()
    } else {
        date.format("%a %-d %b %Y").to_string().to_uppercase()
    };
    Some(match todo.target_time {
        Some(time) => format!("{date_label} · {}", time.format("%H:%M")),
        None => date_label,
    })
}

pub fn add_sheet(mut data: Signal<TodoData>, mut overlays: Overlays) -> Element {
    if !(overlays.adding_todo)() {
        return rsx! {};
    }
    let valid = !(overlays.todo_name)().trim().is_empty();
    rsx! {
        div { class: "overlay", role: "presentation", onclick: move |_| overlays.dismiss(),
            section { class: "sheet todo-form-sheet", role: "dialog", aria_modal: "true", aria_labelledby: "new-todo-title", onclick: move |event| event.stop_propagation(),
                div { class: "sheet-label", "New todo" }
                h2 { id: "new-todo-title", class: "sheet-name", "What needs doing?" }
                form { class: "form todo-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        if valid {
                            let date = NaiveDate::parse_from_str(&(overlays.todo_date)(), "%Y-%m-%d").ok();
                            let time = NaiveTime::parse_from_str(&(overlays.todo_time)(), "%H:%M").ok();
                            data.write().add(&(overlays.todo_name)(), date, time);
                            data().save();
                            overlays.dismiss();
                        }
                    },
                    label { class: "field-label", r#for: "todo-name", "Todo" }
                    input { id: "todo-name", class: "input", autofocus: true, value: "{overlays.todo_name}", placeholder: "Send project invoice", oninput: move |event| overlays.todo_name.set(event.value()) }
                    div { class: "todo-fields",
                        label { class: "todo-field", span { class: "field-label", "Target date" } input { class: "input", r#type: "date", value: "{overlays.todo_date}", oninput: move |event| overlays.todo_date.set(event.value()) } }
                        label { class: "todo-field", span { class: "field-label", "Time (optional)" } input { class: "input", r#type: "time", disabled: (overlays.todo_date)().is_empty(), value: "{overlays.todo_time}", oninput: move |event| overlays.todo_time.set(event.value()) } }
                    }
                    p { class: "todo-form-hint", "Leave the date empty to keep this todo in Anytime." }
                    button { class: "btn", r#type: "submit", disabled: !valid, "CREATE TODO" }
                }
            }
        }
    }
}
