//! Habit data and persistence. Storage goes through [`crate::persist`]
//! (localStorage on web, files in the app data dir on Android/native) —
//! the app is fully client-side and works offline.
//!
//! Storage schema v2: each habit is a set of days it was done (binary —
//! either a day counts or it doesn't) plus an optional free-text note.

use crate::persist;
use chrono::{Datelike, Days, Local, NaiveDate};
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

/// How often a habit is meant to happen (Loop Habit Tracker's model).
/// Every schedule is "hit a target within a period": a day, a rolling
/// N-day window, or the calendar week (Monday–Sunday).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Schedule {
    #[default]
    Daily,
    /// One check-in per rolling `n`-day window.
    EveryNDays { n: u32 },
    /// `times` check-ins per calendar week, Monday through Sunday.
    TimesPerWeek { times: u32 },
    /// `times` check-ins in any rolling `days`-day window.
    TimesInDays { times: u32, days: u32 },
}

impl Schedule {
    /// Short uppercase label: EVERY DAY, EVERY 3 DAYS, WEEKLY, 2×/WEEK,
    /// 2× IN 5 DAYS.
    pub fn label(&self) -> String {
        match *self {
            Schedule::Daily => "EVERY DAY".into(),
            Schedule::EveryNDays { n } => format!("EVERY {n} DAYS"),
            Schedule::TimesPerWeek { times: 1 } => "WEEKLY".into(),
            Schedule::TimesPerWeek { times } => format!("{times}×/WEEK"),
            Schedule::TimesInDays { times, days } => format!("{times}× IN {days} DAYS"),
        }
    }
}

/// The Monday of the week containing `day`.
fn week_start(day: NaiveDate) -> NaiveDate {
    day - Days::new(day.weekday().num_days_from_monday() as u64)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Habit {
    pub id: u64,
    pub name: String,
    /// Optional meta line shown under the name ("06:30 · 5 KM"). Empty
    /// means none.
    #[serde(default)]
    pub note: String,
    /// How often. Absent in data written before schedules existed, so it
    /// defaults to daily — which is what every habit effectively was.
    #[serde(default)]
    pub schedule: Schedule,
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

    /// Check-ins in `[start, end]`, inclusive.
    fn count_between(&self, start: NaiveDate, end: NaiveDate) -> u32 {
        self.days.range(start..=end).count() as u32
    }

    /// The first day of the schedule period that ends on `end`: `end`
    /// itself for daily, `end - (len - 1)` for the rolling windows, the
    /// Monday of `end`'s week for per-week.
    fn period_start(&self, end: NaiveDate) -> Option<NaiveDate> {
        let back = |len: u32| end.checked_sub_days(Days::new(u64::from(len.max(1)) - 1));
        match self.schedule {
            Schedule::Daily => Some(end),
            Schedule::EveryNDays { n } => back(n),
            Schedule::TimesInDays { days, .. } => back(days),
            Schedule::TimesPerWeek { .. } => Some(week_start(end)),
        }
    }

    /// The schedule's check-in target per period.
    fn target(&self) -> u32 {
        match self.schedule {
            Schedule::Daily | Schedule::EveryNDays { .. } => 1,
            Schedule::TimesPerWeek { times } | Schedule::TimesInDays { times, .. } => times.max(1),
        }
    }

    /// Whether the period containing `day` already has its target met,
    /// counting check-ins up to and including `day`.
    pub fn satisfied_on(&self, day: NaiveDate) -> bool {
        self.period_start(day)
            .is_some_and(|start| self.count_between(start, day) >= self.target())
    }

    /// Whether the habit belongs in the DUE list on `day`: its period
    /// target isn't met yet, or it was checked off that very day (a
    /// just-done habit stays in the due list, checked).
    pub fn due_on(&self, day: NaiveDate) -> bool {
        self.done_on(day) || !self.satisfied_on(day)
    }

    /// The first day after `day` on which the habit becomes due again,
    /// assuming no further check-ins. Every schedule runs out within its
    /// own window length, so the search is short; None only if the search
    /// runs off the calendar.
    pub fn next_due(&self, day: NaiveDate) -> Option<NaiveDate> {
        (1..=366)
            .filter_map(|ahead| day.checked_add_days(Days::new(ahead)))
            .find(|&d| !self.satisfied_on(d))
    }

    /// Consecutive satisfied periods counting back from `day`. Rolling
    /// windows are chopped into back-to-back blocks anchored at `day`.
    /// The period containing `day` is still open, so missing it (yet)
    /// doesn't zero the streak — it just doesn't count. Daily reduces to
    /// the classic "consecutive done-days, unticked today forgiven".
    pub fn streak_on(&self, day: NaiveDate) -> usize {
        let mut streak = 0;
        let mut end = day;
        while let Some(start) = self.period_start(end) {
            if self.count_between(start, end) >= self.target() {
                streak += 1;
            } else if end != day {
                break;
            }
            match start.checked_sub_days(Days::new(1)) {
                Some(prev) => end = prev,
                None => break,
            }
        }
        streak
    }

    /// The ledger's second line: the note (or EVERY DAY) for daily
    /// habits, otherwise schedule + progress. True asks for the accent
    /// color (an in-progress flexible target).
    pub fn status_on(&self, day: NaiveDate) -> (String, bool) {
        let next = || {
            self.next_due(day)
                .map(|d| {
                    format!(
                        " · NEXT {}",
                        d.format("%a %-d %b").to_string().to_uppercase()
                    )
                })
                .unwrap_or_default()
        };
        let count = self
            .period_start(day)
            .map_or(0, |start| self.count_between(start, day));
        match self.schedule {
            Schedule::Daily if self.note.is_empty() => ("EVERY DAY".into(), false),
            Schedule::Daily => (self.note.clone(), false),
            Schedule::EveryNDays { .. } if self.satisfied_on(day) && !self.done_on(day) => {
                (format!("{}{}", self.schedule.label(), next()), false)
            }
            Schedule::EveryNDays { .. } => (self.schedule.label(), false),
            Schedule::TimesPerWeek { times } if count >= times => {
                (format!("{} · DONE THIS WEEK", self.schedule.label()), false)
            }
            Schedule::TimesPerWeek { times } => {
                (format!("{count} OF {times} THIS WEEK · DUE BY SUN"), true)
            }
            Schedule::TimesInDays { times, .. } if count >= times => {
                (format!("{}{}", self.schedule.label(), next()), false)
            }
            Schedule::TimesInDays { times, days } => {
                (format!("{count} OF {times} IN {days} DAYS"), true)
            }
        }
    }

    /// [`Self::streak_on`] as of today.
    pub fn streak(&self) -> usize {
        self.streak_on(Local::now().date_naive())
    }

    /// The longest run of consecutive satisfied periods ever, found by
    /// re-measuring the streak as of each check-in. Quadratic in the
    /// number of check-ins, which for a personal tracker is nothing.
    pub fn best_streak(&self) -> usize {
        self.days
            .iter()
            .map(|&day| self.streak_on(day))
            .max()
            .unwrap_or(0)
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
        if let Some(data) = persist::get(KEY) {
            return data;
        }
        // First run on v2: migrate v1 if present, else start empty. The
        // save "commits" the migration; if it fails, we just re-migrate
        // on the next load. (On Android the v1 key never exists.)
        let migrated: Self = persist::get(V1_KEY).map(Self::from_v1).unwrap_or_default();
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
                    schedule: Schedule::Daily,
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
        persist::set(KEY, self);
    }

    pub fn add(&mut self, name: &str, note: &str, schedule: Schedule) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        self.habits.push(Habit {
            id: self.next_id,
            name: name.to_string(),
            note: note.trim().to_string(),
            schedule,
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

    pub fn set_schedule(&mut self, id: u64, schedule: Schedule) {
        if let Some(habit) = self.habits.iter_mut().find(|h| h.id == id) {
            habit.schedule = schedule;
        }
    }

    pub fn delete(&mut self, id: u64) {
        self.habits.retain(|h| h.id != id);
    }
}

/// Whole-collection numbers for the header and the desktop sidebar. With
/// schedules, only habits *due* on a day are counted for that day —
/// that's what the design's "2/4" header means with six habits.
pub struct Summary {
    pub done: usize,
    /// Habits due today.
    pub total: usize,
    /// The last 7 days (oldest first) with the fraction of the habits due
    /// that day that were done.
    pub week: Vec<(NaiveDate, f64)>,
    /// Best streak ever across all habits, with the habit's name.
    pub best: Option<(usize, String)>,
    /// "DAY N": days since the earliest recorded day, 1-based.
    pub day_number: Option<u64>,
}

impl Data {
    pub fn summary(&self) -> Summary {
        let today = Local::now().date_naive();
        let week = (0..7)
            .rev()
            .filter_map(|back| today.checked_sub_days(Days::new(back)))
            .map(|day| {
                let due = self.habits.iter().filter(|h| h.due_on(day)).count();
                let done = self.habits.iter().filter(|h| h.done_on(day)).count();
                (
                    day,
                    if due == 0 {
                        0.0
                    } else {
                        done as f64 / due as f64
                    },
                )
            })
            .collect();
        Summary {
            done: self.habits.iter().filter(|h| h.done_today()).count(),
            total: self.habits.iter().filter(|h| h.due_on(today)).count(),
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
            schedule: Schedule::Daily,
            days: days_back.iter().map(|&b| day(b)).collect(),
        }
    }

    fn nd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn on(schedule: Schedule, days: &[NaiveDate]) -> Habit {
        Habit {
            id: 0,
            name: "test".into(),
            note: String::new(),
            schedule,
            days: days.iter().copied().collect(),
        }
    }

    /// Friday 31 Jul 2026 — the design doc's reference day. Its week runs
    /// Mon 27 Jul – Sun 2 Aug.
    fn fri() -> NaiveDate {
        nd(2026, 7, 31)
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
        data.add("t", "", Schedule::Daily);
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
        data.add("  Run  ", "  06:30 · 5 KM  ", Schedule::Daily);
        data.add("   ", "note", Schedule::Daily);
        assert_eq!(data.habits.len(), 1);
        assert_eq!(data.habits[0].name, "Run");
        assert_eq!(data.habits[0].note, "06:30 · 5 KM");
    }

    #[test]
    fn rename_and_set_note() {
        let mut data = Data::default();
        data.add("Stretch", "", Schedule::Daily);
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
        data.add("Stretch", "", Schedule::Daily);
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

    #[test]
    fn schedule_labels() {
        assert_eq!(Schedule::Daily.label(), "EVERY DAY");
        assert_eq!(Schedule::EveryNDays { n: 3 }.label(), "EVERY 3 DAYS");
        assert_eq!(Schedule::TimesPerWeek { times: 1 }.label(), "WEEKLY");
        assert_eq!(Schedule::TimesPerWeek { times: 2 }.label(), "2×/WEEK");
        assert_eq!(
            Schedule::TimesInDays { times: 2, days: 5 }.label(),
            "2× IN 5 DAYS"
        );
    }

    #[test]
    fn schedule_defaults_to_daily_when_absent_in_stored_data() {
        let json = r#"{"id":0,"name":"Run","days":[]}"#;
        let h: Habit = serde_json::from_str(json).unwrap();
        assert_eq!(h.schedule, Schedule::Daily);
    }

    #[test]
    fn schedule_round_trips_through_serde() {
        for schedule in [
            Schedule::Daily,
            Schedule::EveryNDays { n: 3 },
            Schedule::TimesPerWeek { times: 2 },
            Schedule::TimesInDays { times: 2, days: 5 },
        ] {
            let h = on(schedule, &[]);
            let json = serde_json::to_string(&h).unwrap();
            let back: Habit = serde_json::from_str(&json).unwrap();
            assert_eq!(back.schedule, schedule);
        }
    }

    #[test]
    fn daily_habits_are_always_due() {
        let done = on(Schedule::Daily, &[fri()]);
        assert!(done.satisfied_on(fri()));
        assert!(done.due_on(fri()));
        let idle = on(Schedule::Daily, &[]);
        assert!(!idle.satisfied_on(fri()));
        assert!(idle.due_on(fri()));
    }

    #[test]
    fn every_n_days_is_satisfied_while_the_window_holds_a_checkin() {
        let h = on(Schedule::EveryNDays { n: 3 }, &[nd(2026, 7, 30)]);
        assert!(!h.due_on(nd(2026, 7, 31)));
        assert!(!h.due_on(nd(2026, 8, 1)));
        // The check-in ages out of the 3-day window: due again.
        assert!(h.due_on(nd(2026, 8, 2)));
        // Never done: due.
        assert!(on(Schedule::EveryNDays { n: 3 }, &[]).due_on(fri()));
        // Done on the day itself: satisfied but still listed as due.
        assert!(h.satisfied_on(nd(2026, 7, 30)));
        assert!(h.due_on(nd(2026, 7, 30)));
    }

    #[test]
    fn times_per_week_counts_the_calendar_week() {
        let twice = Schedule::TimesPerWeek { times: 2 };
        // 1 of 2 by Friday: still due.
        assert!(on(twice, &[nd(2026, 7, 27)]).due_on(fri()));
        // 2 of 2: done for the week.
        assert!(!on(twice, &[nd(2026, 7, 27), nd(2026, 7, 29)]).due_on(fri()));
        // Last week's check-ins don't count toward this week.
        assert!(on(twice, &[nd(2026, 7, 25), nd(2026, 7, 26)]).due_on(fri()));
    }

    #[test]
    fn times_in_days_uses_a_rolling_window() {
        let s = Schedule::TimesInDays { times: 2, days: 5 };
        let h = on(s, &[nd(2026, 7, 28), nd(2026, 7, 30)]);
        // Both check-ins inside [27 Jul, 31 Jul].
        assert!(!h.due_on(fri()));
        // By 2 Aug the window is [29 Jul, 2 Aug]: only one left.
        assert!(h.due_on(nd(2026, 8, 2)));
    }

    #[test]
    fn next_due_is_the_day_the_window_breaks() {
        let h = on(Schedule::EveryNDays { n: 3 }, &[nd(2026, 7, 30)]);
        assert_eq!(h.next_due(fri()), Some(nd(2026, 8, 2)));
        let h = on(
            Schedule::TimesInDays { times: 2, days: 5 },
            &[nd(2026, 7, 28), nd(2026, 7, 30)],
        );
        assert_eq!(h.next_due(fri()), Some(nd(2026, 8, 2)));
    }

    #[test]
    fn every_n_days_streak_counts_consecutive_blocks() {
        let s = Schedule::EveryNDays { n: 3 };
        // Blocks back from Fri 31 Jul: [29–31], [26–28], [23–25], [20–22].
        let h = on(s, &[nd(2026, 7, 30), nd(2026, 7, 27), nd(2026, 7, 24)]);
        assert_eq!(h.streak_on(fri()), 3);
        // Current block still open: not done yet doesn't zero it.
        let h = on(s, &[nd(2026, 7, 27), nd(2026, 7, 24)]);
        assert_eq!(h.streak_on(fri()), 2);
        // A fully missed block breaks it.
        let h = on(s, &[nd(2026, 7, 24)]);
        assert_eq!(h.streak_on(fri()), 0);
        // Two check-ins in one block count once.
        let h = on(s, &[nd(2026, 7, 30), nd(2026, 7, 29)]);
        assert_eq!(h.streak_on(fri()), 1);
    }

    #[test]
    fn weekly_streak_counts_calendar_weeks() {
        let weekly = Schedule::TimesPerWeek { times: 1 };
        let h = on(weekly, &[nd(2026, 7, 29), nd(2026, 7, 22), nd(2026, 7, 15)]);
        assert_eq!(h.streak_on(fri()), 3);
        // This week still open: not done yet doesn't zero it.
        let h = on(weekly, &[nd(2026, 7, 22), nd(2026, 7, 15)]);
        assert_eq!(h.streak_on(fri()), 2);
        // A missed week breaks it.
        let h = on(weekly, &[nd(2026, 7, 29), nd(2026, 7, 15)]);
        assert_eq!(h.streak_on(fri()), 1);
        // 2×/week: last week hit, this week only 1 so far.
        let twice = Schedule::TimesPerWeek { times: 2 };
        let h = on(twice, &[nd(2026, 7, 29), nd(2026, 7, 23), nd(2026, 7, 21)]);
        assert_eq!(h.streak_on(fri()), 1);
    }

    #[test]
    fn daily_streak_on_matches_the_old_day_counting() {
        assert_eq!(on(Schedule::Daily, &[]).streak_on(fri()), 0);
        let h = on(
            Schedule::Daily,
            &[nd(2026, 7, 31), nd(2026, 7, 30), nd(2026, 7, 29)],
        );
        assert_eq!(h.streak_on(fri()), 3);
        // Today unticked: counted from yesterday.
        let h = on(Schedule::Daily, &[nd(2026, 7, 30), nd(2026, 7, 29)]);
        assert_eq!(h.streak_on(fri()), 2);
    }

    #[test]
    fn best_streak_works_for_flexible_schedules() {
        let weekly = Schedule::TimesPerWeek { times: 1 };
        // Weeks of Jun 29 and Jul 6 back to back, then a gap, then Jul 27.
        let h = on(weekly, &[nd(2026, 7, 1), nd(2026, 7, 8), nd(2026, 7, 29)]);
        assert_eq!(h.best_streak(), 2);
    }

    #[test]
    fn status_lines_follow_the_design() {
        // Daily: the note wins, else EVERY DAY.
        assert_eq!(
            on(Schedule::Daily, &[]).status_on(fri()),
            ("EVERY DAY".to_string(), false)
        );
        let mut h = on(Schedule::Daily, &[]);
        h.note = "06:30 · 5 KM".into();
        assert_eq!(h.status_on(fri()).0, "06:30 · 5 KM");

        // Weekly target in progress: accent progress line.
        let h = on(Schedule::TimesPerWeek { times: 2 }, &[nd(2026, 7, 27)]);
        assert_eq!(
            h.status_on(fri()),
            ("1 OF 2 THIS WEEK · DUE BY SUN".to_string(), true)
        );
        // Weekly target met.
        let h = on(Schedule::TimesPerWeek { times: 1 }, &[nd(2026, 7, 27)]);
        assert_eq!(
            h.status_on(fri()),
            ("WEEKLY · DONE THIS WEEK".to_string(), false)
        );
        let h = on(
            Schedule::TimesPerWeek { times: 2 },
            &[nd(2026, 7, 27), nd(2026, 7, 30)],
        );
        assert_eq!(
            h.status_on(fri()),
            ("2×/WEEK · DONE THIS WEEK".to_string(), false)
        );

        // Every N days: plain while due, next date once satisfied.
        let h = on(Schedule::EveryNDays { n: 3 }, &[]);
        assert_eq!(h.status_on(fri()), ("EVERY 3 DAYS".to_string(), false));
        let h = on(Schedule::EveryNDays { n: 3 }, &[nd(2026, 7, 30)]);
        assert_eq!(
            h.status_on(fri()),
            ("EVERY 3 DAYS · NEXT SUN 2 AUG".to_string(), false)
        );

        // Rolling window: accent progress while short of the target.
        let h = on(
            Schedule::TimesInDays { times: 2, days: 5 },
            &[nd(2026, 7, 30)],
        );
        assert_eq!(h.status_on(fri()), ("1 OF 2 IN 5 DAYS".to_string(), true));
        let h = on(
            Schedule::TimesInDays { times: 2, days: 5 },
            &[nd(2026, 7, 28), nd(2026, 7, 30)],
        );
        assert_eq!(
            h.status_on(fri()),
            ("2× IN 5 DAYS · NEXT SUN 2 AUG".to_string(), false)
        );
    }

    #[test]
    fn add_stores_the_schedule_and_set_schedule_updates_it() {
        let mut data = Data::default();
        data.add("Run", "", Schedule::EveryNDays { n: 2 });
        let id = data.habits[0].id;
        assert_eq!(data.habits[0].schedule, Schedule::EveryNDays { n: 2 });
        data.set_schedule(id, Schedule::TimesPerWeek { times: 3 });
        assert_eq!(data.habits[0].schedule, Schedule::TimesPerWeek { times: 3 });
        // Unknown id: no-op, no panic.
        data.set_schedule(id + 1, Schedule::Daily);
    }

    #[test]
    fn summary_counts_only_due_habits() {
        let daily_done = habit(&[0]);
        let daily_todo = habit(&[]);
        // Done yesterday on an every-3-days schedule: not due today.
        let mut spaced = habit(&[1]);
        spaced.schedule = Schedule::EveryNDays { n: 3 };
        let s = data_with(vec![daily_done, spaced, daily_todo]).summary();
        assert_eq!((s.done, s.total), (1, 2));
    }

    #[test]
    fn summary_week_fractions_use_what_was_due_each_day() {
        let mut spaced = habit(&[1]);
        spaced.schedule = Schedule::EveryNDays { n: 3 };
        let s = data_with(vec![habit(&[]), spaced]).summary();
        // Yesterday: daily due + spaced due-and-done → 1 of 2.
        assert_eq!(s.week[5], (day(1), 0.5));
        // Today: spaced is covered by yesterday, only the daily is due.
        assert_eq!(s.week[6], (day(0), 0.0));
    }
}
