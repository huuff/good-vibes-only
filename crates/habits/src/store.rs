//! Habit data and persistence. Everything lives in the browser's
//! localStorage — the app is fully client-side and works offline.
//!
//! Storage schema v2: each habit is a set of days it was done (binary —
//! either a day counts or it doesn't) plus an optional free-text note.

use chrono::{Days, Local, NaiveDate};
use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const KEY: &str = "habits/v2";
const V1_KEY: &str = "habits/v1";

/// Days before today that can still be edited from the calendar — enough
/// to backfill a forgotten day or two without making history rewritable
/// wholesale.
pub const EDIT_WINDOW_DAYS: u64 = 7;

/// Whether `day` is still within the edit window
/// (today or up to [`EDIT_WINDOW_DAYS`] back).
pub fn editable(day: NaiveDate) -> bool {
    let today = Local::now().date_naive();
    day <= today
        && today
            .checked_sub_days(Days::new(EDIT_WINDOW_DAYS))
            .is_some_and(|floor| day >= floor)
}

/// The v1 storage schema (timestamped ticks), kept only so existing data
/// can be migrated. The v1 key is never written again, and is left in
/// place as a free backup.
pub(crate) mod v1 {
    use chrono::{DateTime, Utc};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct Habit {
        pub id: u64,
        pub name: String,
        pub ticks: Vec<DateTime<Utc>>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Data {
        pub next_id: u64,
        pub habits: Vec<Habit>,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Habit {
    pub id: u64,
    pub name: String,
    /// Optional meta line shown under the name ("06:30 · 5 KM"). Empty
    /// means none.
    #[serde(default)]
    pub note: String,
    /// The days this habit was done.
    pub days: BTreeSet<NaiveDate>,
}

impl Habit {
    pub fn done_on(&self, day: NaiveDate) -> bool {
        self.days.contains(&day)
    }

    pub fn done_today(&self) -> bool {
        self.done_on(Local::now().date_naive())
    }

    /// Consecutive done-days counting back from today (or from yesterday
    /// if today isn't done yet — an unticked today doesn't zero it).
    pub fn streak(&self) -> usize {
        let today = Local::now().date_naive();
        let mut day = if self.done_on(today) {
            today
        } else {
            match today.checked_sub_days(Days::new(1)) {
                Some(d) => d,
                None => return 0,
            }
        };
        let mut streak = 0;
        while self.done_on(day) {
            streak += 1;
            match day.checked_sub_days(Days::new(1)) {
                Some(d) => day = d,
                None => break,
            }
        }
        streak
    }

    /// The longest run of consecutive done-days ever.
    pub fn best_streak(&self) -> usize {
        let mut best = 0;
        let mut run = 0;
        let mut prev: Option<NaiveDate> = None;
        for &day in &self.days {
            run = match prev {
                Some(p) if p.succ_opt() == Some(day) => run + 1,
                _ => 1,
            };
            best = best.max(run);
            prev = Some(day);
        }
        best
    }

    /// The last `n` days (oldest first), true where done.
    pub fn history(&self, n: u64) -> Vec<(NaiveDate, bool)> {
        let today = Local::now().date_naive();
        (0..n)
            .rev()
            .filter_map(|back| today.checked_sub_days(Days::new(back)))
            .map(|day| (day, self.done_on(day)))
            .collect()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Data {
    next_id: u64,
    pub habits: Vec<Habit>,
}

impl Data {
    pub fn load() -> Self {
        if let Ok(data) = LocalStorage::get(KEY) {
            return data;
        }
        // First run on v2: migrate v1 if present, else start empty. The
        // save "commits" the migration; if it fails, we just re-migrate
        // on the next load.
        let migrated: Self = LocalStorage::get(V1_KEY)
            .map(Self::from_v1)
            .unwrap_or_default();
        if !migrated.habits.is_empty() {
            migrated.save();
        }
        migrated
    }

    /// v1 stored every completion as a UTC timestamp; a v2 day is done if
    /// it had at least one tick (interpreted in local time, as the UI
    /// always displayed them).
    pub(crate) fn from_v1(old: v1::Data) -> Self {
        Self {
            next_id: old.next_id,
            habits: old
                .habits
                .into_iter()
                .map(|h| Habit {
                    id: h.id,
                    name: h.name,
                    note: String::new(),
                    days: h
                        .ticks
                        .iter()
                        .map(|t| t.with_timezone(&Local).date_naive())
                        .collect(),
                })
                .collect(),
        }
    }

    pub fn save(&self) {
        // Ignoring the error: quota exhaustion is the only realistic failure,
        // and there is nowhere better to report it than the next page load.
        let _ = LocalStorage::set(KEY, self);
    }

    pub fn add(&mut self, name: &str, note: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.habits.push(Habit {
            id: self.next_id,
            name: name.to_string(),
            note: note.trim().to_string(),
            days: BTreeSet::new(),
        });
        self.next_id += 1;
    }

    /// Flip `day` between done and not done. Days outside the edit window
    /// (including the future) are ignored.
    pub fn toggle(&mut self, id: u64, day: NaiveDate) {
        if !editable(day) {
            return;
        }
        if let Some(habit) = self.habits.iter_mut().find(|h| h.id == id)
            && !habit.days.remove(&day)
        {
            habit.days.insert(day);
        }
    }

    pub fn rename(&mut self, id: u64, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(habit) = self.habits.iter_mut().find(|h| h.id == id) {
            habit.name = name.to_string();
        }
    }

    /// Set the note; an empty (or whitespace) note clears it.
    pub fn set_note(&mut self, id: u64, note: &str) {
        if let Some(habit) = self.habits.iter_mut().find(|h| h.id == id) {
            habit.note = note.trim().to_string();
        }
    }

    pub fn delete(&mut self, id: u64) {
        self.habits.retain(|h| h.id != id);
    }
}

/// Whole-collection numbers for the header and the desktop sidebar.
pub struct Summary {
    pub done: usize,
    pub total: usize,
    /// The last 7 days (oldest first) with the fraction of habits done.
    pub week: Vec<(NaiveDate, f64)>,
    /// Best streak ever across all habits, with the habit's name.
    pub best: Option<(usize, String)>,
    /// "DAY N": days since the earliest recorded day, 1-based.
    pub day_number: Option<u64>,
}

impl Data {
    pub fn summary(&self) -> Summary {
        let today = Local::now().date_naive();
        let total = self.habits.len();
        let week = (0..7)
            .rev()
            .filter_map(|back| today.checked_sub_days(Days::new(back)))
            .map(|day| {
                let done = self.habits.iter().filter(|h| h.done_on(day)).count();
                (
                    day,
                    if total == 0 {
                        0.0
                    } else {
                        done as f64 / total as f64
                    },
                )
            })
            .collect();
        Summary {
            done: self.habits.iter().filter(|h| h.done_today()).count(),
            total,
            week,
            best: self
                .habits
                .iter()
                .map(|h| (h.best_streak(), h.name.clone()))
                .filter(|&(streak, _)| streak > 0)
                .max_by_key(|&(streak, _)| streak),
            day_number: self
                .habits
                .iter()
                .filter_map(|h| h.days.first())
                .min()
                .map(|&first| (today - first).num_days().max(0) as u64 + 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Utc};

    fn day(back: u64) -> NaiveDate {
        Local::now().date_naive() - Days::new(back)
    }

    fn tick(days_back: u64) -> DateTime<Utc> {
        let day = Local::now().date_naive() - Days::new(days_back);
        day.and_hms_opt(12, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .unwrap()
            .to_utc()
    }

    fn habit(days_back: &[u64]) -> Habit {
        Habit {
            id: 0,
            name: "test".into(),
            note: String::new(),
            days: days_back.iter().map(|&b| day(b)).collect(),
        }
    }

    #[test]
    fn done_on_and_done_today() {
        let h = habit(&[0, 2]);
        assert!(h.done_today());
        assert!(h.done_on(day(2)));
        assert!(!h.done_on(day(1)));
        assert!(!habit(&[1]).done_today());
    }

    #[test]
    fn streak_counts_consecutive_days() {
        assert_eq!(habit(&[]).streak(), 0);
        assert_eq!(habit(&[0, 1, 2]).streak(), 3);
        // Today not yet done: the streak survives, counted from yesterday.
        assert_eq!(habit(&[1, 2]).streak(), 2);
        // A gap breaks it.
        assert_eq!(habit(&[0, 2, 3]).streak(), 1);
    }

    #[test]
    fn best_streak_finds_longest_run_ever() {
        assert_eq!(habit(&[]).best_streak(), 0);
        assert_eq!(habit(&[0]).best_streak(), 1);
        // Current run of 3, but an older run of 4 wins.
        assert_eq!(habit(&[0, 1, 2, 5, 6, 7, 8]).best_streak(), 4);
    }

    #[test]
    fn history_is_oldest_first_with_done_flags() {
        let h = habit(&[0, 13]);
        let hist = h.history(14);
        assert_eq!(hist.len(), 14);
        assert_eq!(hist[0], (day(13), true));
        assert_eq!(hist[13], (day(0), true));
        assert!(hist[1..13].iter().all(|&(_, done)| !done));
    }

    #[test]
    fn toggle_flips_within_window_only() {
        let mut data = Data::default();
        data.add("t", "");
        let id = data.habits[0].id;
        let today = Local::now().date_naive();

        data.toggle(id, today);
        assert!(data.habits[0].done_on(today));
        data.toggle(id, today);
        assert!(!data.habits[0].done_on(today));

        // Backfill inside the window works; outside (and future) is ignored.
        data.toggle(id, day(EDIT_WINDOW_DAYS));
        assert!(data.habits[0].done_on(day(EDIT_WINDOW_DAYS)));
        data.toggle(id, day(EDIT_WINDOW_DAYS + 1));
        data.toggle(id, today + Days::new(1));
        assert_eq!(data.habits[0].days.len(), 1);
    }

    #[test]
    fn editable_covers_today_through_window_floor() {
        let today = Local::now().date_naive();
        assert!(editable(today));
        assert!(editable(today - Days::new(EDIT_WINDOW_DAYS)));
        assert!(!editable(today - Days::new(EDIT_WINDOW_DAYS + 1)));
        assert!(!editable(today + Days::new(1)));
    }

    #[test]
    fn add_trims_name_and_note_and_rejects_empty_names() {
        let mut data = Data::default();
        data.add("  Run  ", "  06:30 · 5 KM  ");
        data.add("   ", "note");
        assert_eq!(data.habits.len(), 1);
        assert_eq!(data.habits[0].name, "Run");
        assert_eq!(data.habits[0].note, "06:30 · 5 KM");
    }

    #[test]
    fn rename_and_set_note() {
        let mut data = Data::default();
        data.add("Stretch", "");
        let id = data.habits[0].id;

        data.rename(id, "  Morning stretch  ");
        assert_eq!(data.habits[0].name, "Morning stretch");
        // Whitespace-only rename: rejected, name untouched.
        data.rename(id, "   ");
        assert_eq!(data.habits[0].name, "Morning stretch");

        data.set_note(id, "  EVENING ");
        assert_eq!(data.habits[0].note, "EVENING");
        // But an empty note is a valid way to clear it.
        data.set_note(id, "  ");
        assert_eq!(data.habits[0].note, "");

        // Unknown id: no-op, no panic.
        data.rename(id + 1, "Other");
        data.set_note(id + 1, "x");
    }

    #[test]
    fn delete_removes_the_habit() {
        let mut data = Data::default();
        data.add("Stretch", "");
        let id = data.habits[0].id;
        data.delete(id);
        assert!(data.habits.is_empty());
    }

    #[test]
    fn from_v1_collapses_ticks_to_days_and_keeps_ids() {
        let old = v1::Data {
            next_id: 7,
            habits: vec![
                v1::Habit {
                    id: 3,
                    name: "Run".into(),
                    // Two ticks on the same day collapse to one done-day.
                    ticks: vec![tick(0), tick(0), tick(2)],
                },
                v1::Habit {
                    id: 5,
                    name: "Read".into(),
                    ticks: vec![],
                },
            ],
        };
        let data = Data::from_v1(old);
        assert_eq!(data.habits.len(), 2);
        let run = &data.habits[0];
        assert_eq!(
            (run.id, run.name.as_str(), run.note.as_str()),
            (3, "Run", "")
        );
        assert_eq!(run.days.len(), 2);
        assert!(run.done_today() && run.done_on(day(2)));
        assert!(data.habits[1].days.is_empty());
        // next_id is preserved so migrated ids can't collide with new ones.
        data_next_id_is(data, 7);
    }

    fn data_next_id_is(data: Data, expected: u64) {
        // next_id is private; round-trip through serde to check it.
        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains(&format!("\"next_id\":{expected}")));
    }

    #[test]
    fn v1_json_still_deserializes() {
        // Shape written by the old code: private next_id + habits with ticks.
        let json =
            r#"{"next_id":2,"habits":[{"id":0,"name":"Run","ticks":["2026-07-28T05:30:00Z"]}]}"#;
        let old: v1::Data = serde_json::from_str(json).unwrap();
        assert_eq!(old.next_id, 2);
        assert_eq!(old.habits[0].ticks.len(), 1);
    }

    fn data_with(habits: Vec<Habit>) -> Data {
        let mut data = Data::default();
        for (i, mut h) in habits.into_iter().enumerate() {
            h.id = i as u64;
            data.habits.push(h);
        }
        data
    }

    #[test]
    fn summary_counts_todays_completions() {
        let data = data_with(vec![habit(&[0]), habit(&[1]), habit(&[0, 1])]);
        let s = data.summary();
        assert_eq!((s.done, s.total), (2, 3));
    }

    #[test]
    fn summary_week_is_per_day_fractions_oldest_first() {
        let data = data_with(vec![habit(&[0, 6]), habit(&[0])]);
        let s = data.summary();
        assert_eq!(s.week.len(), 7);
        assert_eq!(s.week[0], (day(6), 0.5));
        assert_eq!(s.week[6], (day(0), 1.0));
        assert!(s.week[1..6].iter().all(|&(_, f)| f == 0.0));
        // No habits: all-zero fractions rather than division by zero.
        assert!(
            Data::default()
                .summary()
                .week
                .iter()
                .all(|&(_, f)| f == 0.0)
        );
    }

    #[test]
    fn summary_best_is_the_longest_run_ever() {
        let mut a = habit(&[0, 1, 4, 5, 6, 7, 8]); // current 2, best 5
        a.name = "A".into();
        let mut b = habit(&[0, 1, 2]); // best 3
        b.name = "B".into();
        let s = data_with(vec![a, b]).summary();
        assert_eq!(s.best, Some((5, "A".into())));
        // No recorded days anywhere: no best streak.
        assert_eq!(data_with(vec![habit(&[])]).summary().best, None);
    }

    #[test]
    fn summary_day_number_counts_from_first_recorded_day() {
        assert_eq!(
            data_with(vec![habit(&[9]), habit(&[2])])
                .summary()
                .day_number,
            Some(10)
        );
        assert_eq!(data_with(vec![habit(&[0])]).summary().day_number, Some(1));
        assert_eq!(data_with(vec![habit(&[])]).summary().day_number, None);
    }
}
