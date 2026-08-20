//! Date-grouped Todos screen and its add sheet.

use chrono::{Datelike, NaiveDate, NaiveTime};
use dioxus::prelude::*;

use super::Overlays;
use crate::clock;
use crate::i18n::fill;
use crate::preferences::Language;
use crate::todos::{Todo, TodoData};

pub fn todos(data: Signal<TodoData>, overlays: Overlays, lang: Language) -> Element {
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
                    {todo_group(t.grp_today, due, data, overlays, today, lang)}
                    {todo_group(t.grp_later, later, data, overlays, today, lang)}
                    {todo_group(t.grp_anytime, anytime, data, overlays, today, lang)}
                }
            }
        }
    }
}

fn todo_group(
    label: &'static str,
    todos: Vec<Todo>,
    data: Signal<TodoData>,
    overlays: Overlays,
    today: NaiveDate,
    lang: Language,
) -> Element {
    if todos.is_empty() {
        return rsx! {};
    }
    rsx! {
        section { class: "todo-group",
            h2 { class: "todo-group-label", {label} }
            for todo in todos {
                {todo_row(todo, data, overlays, today, lang)}
            }
        }
    }
}

/// One todo row: the checkbox toggles, the copy area opens the edit
/// sheet — the same split as the habit ledger's rows.
fn todo_row(
    todo: Todo,
    mut data: Signal<TodoData>,
    mut overlays: Overlays,
    today: NaiveDate,
    lang: Language,
) -> Element {
    let t = lang.strings();
    let id = todo.id;
    let done = todo.done;
    let name = todo.name.clone();
    let meta = todo_meta(&todo, today, lang);
    let toggle_label = if done {
        fill(t.mark_incomplete, &[&name])
    } else {
        fill(t.mark_complete, &[&name])
    };
    let edit_title = fill(t.edit_todo, &[&name]);
    let row_todo = todo.clone();
    let mut row_overlays = overlays;
    rsx! {
        div {
            key: "{id}",
            class: if done { "todo-row done" } else { "todo-row" },
            onclick: move |_| row_overlays.open_edit_todo(&row_todo),
            button {
                class: if done { "todo-box done" } else { "todo-box" },
                aria_pressed: done,
                aria_label: toggle_label,
                onclick: move |event| {
                    event.stop_propagation();
                    data.write().toggle(id);
                    data().save();
                },
                if done {
                    svg { view_box: "0 0 24 24", path { d: "M4 12.5l5 5L20 6.5" } }
                }
            }
            button {
                class: "todo-copy",
                title: edit_title,
                onclick: move |event| {
                    event.stop_propagation();
                    overlays.open_edit_todo(&todo);
                },
                strong { class: "todo-name", {name} }
                if let Some(meta) = meta { span { class: "todo-meta", {meta} } }
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

/// The todo form sheet: creates when opened from the FAB, edits (same
/// form, prefilled, plus delete) when opened from a row.
pub fn add_sheet(mut data: Signal<TodoData>, mut overlays: Overlays, lang: Language) -> Element {
    let editing = (overlays.todo_edit)();
    if !(overlays.adding_todo)() && editing.is_none() {
        return rsx! {};
    }
    let t = lang.strings();
    let valid = !(overlays.todo_name)().trim().is_empty();
    rsx! {
        div { class: "overlay", role: "presentation", onclick: move |_| overlays.dismiss(),
            section { class: "sheet todo-form-sheet", role: "dialog", aria_modal: "true", aria_labelledby: "new-todo-title", onclick: move |event| event.stop_propagation(),
                div { class: "sheet-label",
                    if editing.is_some() { {t.edit_todo_label} } else { {t.new_todo_label} }
                }
                h2 { id: "new-todo-title", class: "sheet-name", {t.what_needs_doing} }
                form { class: "form todo-form",
                    onsubmit: move |event| {
                        event.prevent_default();
                        if valid {
                            let date = NaiveDate::parse_from_str(&(overlays.todo_date)(), "%Y-%m-%d").ok();
                            let time = NaiveTime::parse_from_str(&(overlays.todo_time)(), "%H:%M").ok();
                            match editing {
                                Some(id) => data.write().update(id, &(overlays.todo_name)(), date, time),
                                None => data.write().add(&(overlays.todo_name)(), date, time),
                            }
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
                    button { class: "btn", r#type: "submit", disabled: !valid,
                        if editing.is_some() { {t.save} } else { {t.create_todo} }
                    }
                }
                if let Some(id) = editing {
                    div { class: "sheet-del",
                        if (overlays.confirm)() {
                            button {
                                class: "btn-quiet danger",
                                title: t.really_delete,
                                onclick: move |_| {
                                    overlays.dismiss();
                                    data.write().delete(id);
                                    data().save();
                                },
                                {t.sure}
                            }
                        } else {
                            button {
                                class: "btn-quiet",
                                title: t.delete_todo_title,
                                onclick: move |_| overlays.confirm.set(true),
                                {t.delete_todo}
                            }
                        }
                    }
                }
            }
        }
    }
}
