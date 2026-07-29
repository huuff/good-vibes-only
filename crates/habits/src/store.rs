//! Habit data and persistence. Everything lives in the browser's
//! localStorage — the app is fully client-side and works offline.

use chrono::{DateTime, Days, Local, NaiveDate, Utc};
use gloo_storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};

const KEY: &str = "habits/v1";

/// Days of practice after which a behaviour is likely automatic: the median
/// from Lally et al. 2010 (Eur. J. Soc. Psychol.), confirmed by the 2024
/// meta-analysis of health-behaviour habit formation (median 59–66 days).
pub const FORMATION_DAYS: usize = 66;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Habit {
    pub id: u64,
    pub name: String,
    /// Every recorded completion, in UTC.
    pub ticks: Vec<DateTime<Utc>>,
}

impl Habit {
    fn ticks_on(&self, day: NaiveDate) -> usize {
        self.ticks
            .iter()
            .filter(|t| t.with_timezone(&Local).date_naive() == day)
            .count()
    }

    pub fn today_count(&self) -> usize {
        self.ticks_on(Local::now().date_naive())
    }

    /// Consecutive days with at least one tick, counting back from today
    /// (or from yesterday if today is still unticked).
    pub fn streak(&self) -> usize {
        let today = Local::now().date_naive();
        let mut day = if self.ticks_on(today) > 0 {
            today
        } else {
            match today.checked_sub_days(Days::new(1)) {
                Some(d) => d,
                None => return 0,
            }
        };
        let mut streak = 0;
        while self.ticks_on(day) > 0 {
            streak += 1;
            match day.checked_sub_days(Days::new(1)) {
                Some(d) => day = d,
                None => break,
            }
        }
        streak
    }

    /// Habit strength in day-equivalents, capped at [`FORMATION_DAYS`].
    /// Each practiced day adds one. A single missed day is free (Lally et
    /// al. found the odd miss doesn't affect formation), but every further
    /// consecutive idle day erodes half a day. Today never counts as a
    /// miss — it isn't over yet.
    pub fn strength(&self) -> f64 {
        let days: std::collections::HashSet<NaiveDate> = self
            .ticks
            .iter()
            .map(|t| t.with_timezone(&Local).date_naive())
            .collect();
        let Some(&first) = days.iter().min() else {
            return 0.0;
        };
        let today = Local::now().date_naive();
        let mut strength: f64 = 0.0;
        let mut idle = 0u32;
        let mut day = first;
        while day <= today {
            if days.contains(&day) {
                idle = 0;
                strength = (strength + 1.0).min(FORMATION_DAYS as f64);
            } else if day < today {
                idle += 1;
                if idle > 1 {
                    strength = (strength - 0.5).max(0.0);
                }
            }
            match day.succ_opt() {
                Some(next) => day = next,
                None => break,
            }
        }
        strength
    }

    /// The last seven days (oldest first), true for days with at least one tick.
    pub fn week(&self) -> Vec<(NaiveDate, bool)> {
        let today = Local::now().date_naive();
        (0..7)
            .rev()
            .filter_map(|back| today.checked_sub_days(Days::new(back)))
            .map(|day| (day, self.ticks_on(day) > 0))
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
        LocalStorage::get(KEY).unwrap_or_default()
    }

    pub fn save(&self) {
        // Ignoring the error: quota exhaustion is the only realistic failure,
        // and there is nowhere better to report it than the next page load.
        let _ = LocalStorage::set(KEY, self);
    }

    pub fn add(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.habits.push(Habit {
            id: self.next_id,
            name: name.to_string(),
            ticks: Vec::new(),
        });
        self.next_id += 1;
    }

    pub fn record(&mut self, id: u64) {
        if let Some(habit) = self.habits.iter_mut().find(|h| h.id == id) {
            habit.ticks.push(Utc::now());
        }
    }

    /// Remove the most recent tick, but only if it was made today (fat-finger undo).
    pub fn undo(&mut self, id: u64) {
        let today = Local::now().date_naive();
        if let Some(habit) = self.habits.iter_mut().find(|h| h.id == id)
            && let Some(last) = habit.ticks.last()
            && last.with_timezone(&Local).date_naive() == today
        {
            habit.ticks.pop();
        }
    }

    pub fn delete(&mut self, id: u64) {
        self.habits.retain(|h| h.id != id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tick at noon local time `days_back` days ago, so date boundaries and
    /// DST can't skew which local day it lands on.
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
            ticks: days_back.iter().map(|&d| tick(d)).collect(),
        }
    }

    #[test]
    fn today_count_only_counts_today() {
        let h = habit(&[0, 0, 1]);
        assert_eq!(h.today_count(), 2);
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
    fn strength_grows_per_practiced_day() {
        assert_eq!(habit(&[]).strength(), 0.0);
        // Several ticks on one day still count as one practiced day, and
        // today being unticked so far costs nothing.
        assert_eq!(habit(&[1, 1, 2, 3]).strength(), 3.0);
    }

    #[test]
    fn strength_erodes_after_grace_day() {
        // One missed day between practiced days is free...
        assert_eq!(habit(&[0, 2]).strength(), 2.0);
        // ...but a 10-day break (days 10..1 idle before an unticked today)
        // erodes half a day for each idle day past the first: 9 × 0.5.
        let ten_practiced: Vec<u64> = (11..=20).collect();
        assert_eq!(habit(&ten_practiced).strength(), 10.0 - 4.5);
        // Strength never goes below zero.
        assert_eq!(habit(&[400]).strength(), 0.0);
    }

    #[test]
    fn week_marks_days_with_ticks() {
        let days: Vec<bool> = habit(&[0, 6]).week().iter().map(|&(_, d)| d).collect();
        assert_eq!(days, [true, false, false, false, false, false, true]);
    }

    #[test]
    fn add_record_undo_delete() {
        let mut data = Data::default();
        data.add("  Stretch  ");
        data.add("   "); // whitespace-only is rejected
        assert_eq!(data.habits.len(), 1);
        assert_eq!(data.habits[0].name, "Stretch");
        let id = data.habits[0].id;

        data.record(id);
        data.record(id);
        assert_eq!(data.habits[0].ticks.len(), 2);

        data.undo(id);
        assert_eq!(data.habits[0].ticks.len(), 1);

        data.delete(id);
        assert!(data.habits.is_empty());
    }
}
