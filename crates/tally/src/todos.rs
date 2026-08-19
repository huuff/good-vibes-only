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
}
