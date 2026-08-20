//! Date-grouped Todos screen and its add sheet.

use chrono::{Datelike, NaiveDate, NaiveTime};
use dioxus::prelude::*;

use super::Overlays;
use crate::clock;
use crate::i18n::fill;
use crate::preferences::Language;
use crate::todos::{Todo, TodoData};

pub fn todos(data: Signal<TodoData>, lang: Language) -> Element {
    let t = lang.strings();
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
                span { class: "head-date",
                    {today.format_localized("%a %-d %b %Y", lang.locale()).to_string()}
                }
                h1 { class: "title", {t.todos_title} }
            }
            div { class: "todo-list",
                if snapshot.todos.is_empty() {
                    div { class: "empty todo-empty",
                        strong { {t.empty_todos} }
                        span { {t.empty_todos_hint} }
                    }
                } else {
                    {todo_group(t.grp_today, due, data, today, lang)}
                    {todo_group(t.grp_later, later, data, today, lang)}
                    {todo_group(t.grp_anytime, anytime, data, today, lang)}
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
    lang: Language,
) -> Element {
    let t = lang.strings();
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
                    aria_label: if todo.done { fill(t.mark_incomplete, &[&todo.name]) } else { fill(t.mark_complete, &[&todo.name]) },
                    onclick: move |_| { data.write().toggle(todo.id); data().save(); },
                    span { class: if todo.done { "todo-box done" } else { "todo-box" },
                        if todo.done {
                            svg { view_box: "0 0 24 24", path { d: "M4 12.5l5 5L20 6.5" } }
                        }
                    }
                    span { class: "todo-copy",
                        strong { class: "todo-name", {todo.name.clone()} }
                        if let Some(meta) = todo_meta(&todo, today, lang) { span { class: "todo-meta", {meta} } }
                    }
                }
            }
        }
    }
}

fn todo_meta(todo: &Todo, today: NaiveDate, lang: Language) -> Option<String> {
    let date = todo.target_date?;
    let date_label = if date == today {
        lang.strings().grp_today.to_string()
    } else if date.year() == today.year() {
        date.format_localized("%a %-d %b", lang.locale())
            .to_string()
            .to_uppercase()
    } else {
        date.format_localized("%a %-d %b %Y", lang.locale())
            .to_string()
            .to_uppercase()
    };
    Some(match todo.target_time {
        Some(time) => format!("{date_label} · {}", time.format("%H:%M")),
        None => date_label,
    })
}

pub fn add_sheet(mut data: Signal<TodoData>, mut overlays: Overlays, lang: Language) -> Element {
    if !(overlays.adding_todo)() {
        return rsx! {};
    }
    let t = lang.strings();
    let valid = !(overlays.todo_name)().trim().is_empty();
    rsx! {
        div { class: "overlay", role: "presentation", onclick: move |_| overlays.dismiss(),
            section { class: "sheet todo-form-sheet", role: "dialog", aria_modal: "true", aria_labelledby: "new-todo-title", onclick: move |event| event.stop_propagation(),
                div { class: "sheet-label", {t.new_todo_label} }
                h2 { id: "new-todo-title", class: "sheet-name", {t.what_needs_doing} }
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
                    label { class: "field-label", r#for: "todo-name", {t.todo_field} }
                    input { id: "todo-name", class: "input", autofocus: true, value: "{overlays.todo_name}", placeholder: t.todo_placeholder, oninput: move |event| overlays.todo_name.set(event.value()) }
                    div { class: "todo-fields",
                        label { class: "todo-field", span { class: "field-label", {t.target_date} } input { class: "input", r#type: "date", value: "{overlays.todo_date}", oninput: move |event| overlays.todo_date.set(event.value()) } }
                        label { class: "todo-field", span { class: "field-label", {t.time_optional} } input { class: "input", r#type: "time", disabled: (overlays.todo_date)().is_empty(), value: "{overlays.todo_time}", oninput: move |event| overlays.todo_time.set(event.value()) } }
                    }
                    p { class: "todo-form-hint", {t.todo_hint} }
                    button { class: "btn", r#type: "submit", disabled: !valid, {t.create_todo} }
                }
            }
        }
    }
}
