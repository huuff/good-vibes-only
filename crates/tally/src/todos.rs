//! Persisted, one-off tasks for the Todos tab.

use crate::persist;
use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

const KEY: &str = "todos/v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Todo {
    pub id: u64,
    pub name: String,
    pub target_date: Option<NaiveDate>,
    pub target_time: Option<NaiveTime>,
    pub done: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodoData {
    next_id: u64,
    pub todos: Vec<Todo>,
}

impl TodoData {
    pub fn load() -> Self {
        persist::get(KEY).unwrap_or_default()
    }
    pub fn save(&self) {
        persist::set(KEY, self);
    }

    pub fn add(
        &mut self,
        name: &str,
        target_date: Option<NaiveDate>,
        target_time: Option<NaiveTime>,
    ) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.todos.push(Todo {
            id: self.next_id,
            name: name.to_string(),
            target_date,
            target_time: target_date.and(target_time),
            done: false,
        });
        self.next_id += 1;
    }

    pub fn toggle(&mut self, id: u64) {
        if let Some(todo) = self.todos.iter_mut().find(|todo| todo.id == id) {
            todo.done = !todo.done;
        }
    }

    /// Replace a todo's name/date/time. Empty names are rejected (like
    /// habit renames); a time without a date is discarded (like `add`).
    pub fn update(
        &mut self,
        id: u64,
        name: &str,
        target_date: Option<NaiveDate>,
        target_time: Option<NaiveTime>,
    ) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(todo) = self.todos.iter_mut().find(|todo| todo.id == id) {
            todo.name = name.to_string();
            todo.target_date = target_date;
            todo.target_time = target_date.and(target_time);
        }
    }

    pub fn delete(&mut self, id: u64) {
        self.todos.retain(|todo| todo.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undated_todos_discard_a_time() {
        let mut data = TodoData::default();
        data.add("Call the dentist", None, NaiveTime::from_hms_opt(10, 30, 0));
        assert_eq!(data.todos[0].target_time, None);
    }

    #[test]
    fn toggles_completion() {
        let mut data = TodoData::default();
        data.add("Send invoice", None, None);
        data.toggle(0);
        assert!(data.todos[0].done);
    }

    #[test]
    fn update_replaces_fields_and_keeps_done() {
        let mut data = TodoData::default();
        let date = NaiveDate::from_ymd_opt(2026, 8, 21);
        data.add("Send invoice", date, NaiveTime::from_hms_opt(10, 0, 0));
        data.toggle(0);

        data.update(
            0,
            "Send the invoice",
            None,
            NaiveTime::from_hms_opt(9, 0, 0),
        );
        let todo = &data.todos[0];
        assert_eq!(todo.name, "Send the invoice");
        // Clearing the date also drops the time, and done survives.
        assert_eq!((todo.target_date, todo.target_time), (None, None));
        assert!(todo.done);

        // Empty names are rejected; unknown ids are a no-op.
        data.update(0, "   ", date, None);
        assert_eq!(data.todos[0].name, "Send the invoice");
        data.update(99, "Ghost", None, None);
        assert_eq!(data.todos.len(), 1);
    }

    #[test]
    fn delete_removes_the_todo() {
        let mut data = TodoData::default();
        data.add("Send invoice", None, None);
        data.delete(0);
        assert!(data.todos.is_empty());
    }
}
