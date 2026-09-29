//! Habit reminders (design turn 15): one per habit, a time plus the
//! weekdays it applies to. A reminder only fires while the habit is still
//! due in its current period — once the period's target is met it stays
//! quiet until the next one — so "only when due" is behavior, not a
//! setting.
//!
//! Delivery needs the OS to wake the app, so reminders only exist on
//! Android ([`SUPPORTED`]); the scheduling/notification glue is in
//! [`crate::android`]. Everything here is plain date math, testable
//! natively.

use chrono::{Datelike, Days, NaiveDate, NaiveDateTime, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::i18n::{Strings, fill};
use crate::preferences::{Language, WeekStart};
use crate::store::{Data, Habit, Schedule};

/// Whether this build can deliver reminders at all. The reminder UI is
/// hidden elsewhere: a browser tab can't be woken at a set time.
pub const SUPPORTED: bool = cfg!(target_os = "android");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reminder {
    /// Switched off keeps the time and days for when it's turned back on.
    pub enabled: bool,
    pub time: NaiveTime,
    /// Monday first, indexed by [`Weekday::num_days_from_monday`].
    pub days: [bool; 7],
}

impl Default for Reminder {
    fn default() -> Self {
        Self {
            enabled: true,
            time: NaiveTime::from_hms_opt(19, 0, 0).expect("valid time"),
            days: [true; 7],
        }
    }
}

impl Reminder {
    pub fn applies_on(&self, day: NaiveDate) -> bool {
        self.on(day.weekday())
    }

    pub fn on(&self, weekday: Weekday) -> bool {
        self.days[weekday.num_days_from_monday() as usize]
    }

    pub fn toggle_day(&mut self, weekday: Weekday) {
        let slot = &mut self.days[weekday.num_days_from_monday() as usize];
        *slot = !*slot;
    }

    /// Whether a reminder at `time` on `day` should be shown: switched
    /// on, and the habit's period still open. A snoozed reminder ("Later")
    /// already passed the weekday check when it first fired.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn should_notify(
        &self,
        habit: &Habit,
        day: NaiveDate,
        week_first: WeekStart,
        snoozed: bool,
    ) -> bool {
        self.enabled
            && (snoozed || self.applies_on(day))
            && !habit.satisfied_on_with_week_start(day, week_first)
    }

    /// The first reminder moment strictly after `after`, assuming no
    /// further check-ins. None when switched off or no day is picked.
    pub fn next_fire(
        &self,
        habit: &Habit,
        after: NaiveDateTime,
        week_first: WeekStart,
    ) -> Option<NaiveDateTime> {
        if !self.enabled || !self.days.contains(&true) {
            return None;
        }
        // Rolling windows can hold a met target for up to 90 days (the
        // schedule picker's maximum), so look a little further than that.
        (0..=100)
            .filter_map(|ahead| after.date().checked_add_days(Days::new(ahead)))
            .map(|day| day.and_time(self.time))
            .find(|&at| {
                at > after
                    && self.applies_on(at.date())
                    && !habit.satisfied_on_with_week_start(at.date(), week_first)
            })
    }

    pub fn time_label(&self) -> String {
        self.time.format("%H:%M").to_string()
    }

    /// "EVERY DAY", "MON – FRI" or "MON, WED, FRI", in week order.
    pub fn days_label(&self, week_first: WeekStart, lang: Language) -> String {
        let week = week_days(week_first);
        let picked: Vec<Weekday> = week.iter().copied().filter(|&d| self.on(d)).collect();
        let short = |d: Weekday| {
            weekday_name(d, "%a", lang)
                .trim_end_matches('.')
                .to_uppercase()
        };
        if picked.len() == 7 {
            return lang.strings().sched_every_day.into();
        }
        let first = week.iter().position(|&d| d == picked[0]);
        let contiguous =
            first.is_some_and(|start| week[start..].iter().take(picked.len()).eq(picked.iter()));
        if picked.len() >= 3 && contiguous {
            format!("{} – {}", short(picked[0]), short(picked[picked.len() - 1]))
        } else {
            picked.into_iter().map(short).collect::<Vec<_>>().join(", ")
        }
    }

    /// "19:00 · MON – FRI", or "OFF" when switched off.
    pub fn summary(&self, week_first: WeekStart, lang: Language) -> String {
        if self.enabled {
            format!(
                "{} · {}",
                self.time_label(),
                self.days_label(week_first, lang)
            )
        } else {
            lang.strings().reminder_off.to_uppercase()
        }
    }
}

/// The per-habit rule, stated where the reminder is set: "Quiet once
/// this week's 3 check-ins are done."
pub fn quiet_rule(schedule: Schedule, t: &Strings) -> String {
    match schedule {
        Schedule::Daily => t.quiet_daily.into(),
        Schedule::EveryNDays { n } => fill(t.quiet_every_n, &[&n]),
        Schedule::TimesPerWeek { times: 1 } => t.quiet_week_one.into(),
        Schedule::TimesPerWeek { times } => fill(t.quiet_week, &[&times]),
        Schedule::TimesInDays { times, days } => fill(t.quiet_window, &[&days, &times]),
    }
}

/// The seven weekdays starting from the configured first day.
pub fn week_days(week_first: WeekStart) -> [Weekday; 7] {
    let mut day = week_first.weekday();
    std::array::from_fn(|_| {
        let this = day;
        day = day.succ();
        this
    })
}

/// A localized weekday name via chrono (`%a` short, `%A` full).
pub fn weekday_name(weekday: Weekday, format: &str, lang: Language) -> String {
    // Any known Monday; offset to the wanted weekday.
    let monday = NaiveDate::from_ymd_opt(2026, 1, 5).expect("valid date");
    (monday + Days::new(u64::from(weekday.num_days_from_monday())))
        .format_localized(format, lang.locale())
        .to_string()
}

/// Bumped whenever habit data is written outside the UI (a notification's
/// Done action), so a running UI knows to reload from storage.
static EXTERNAL_WRITES: AtomicU64 = AtomicU64::new(0);

pub fn external_writes() -> u64 {
    EXTERNAL_WRITES.load(Ordering::Relaxed)
}

/// Re-arm every habit's next reminder alarm from the current data, and
/// clear any shown reminder whose habit no longer needs it. Called after
/// every habit save, so check-ins, schedule edits and deletes all keep
/// the alarms honest.
pub fn sync(data: &Data) {
    if !SUPPORTED {
        return;
    }
    let week_first = crate::preferences::Preferences::load().week_start;
    let now = crate::clock::now();
    for habit in &data.habits {
        let reminder = habit.reminder.filter(|r| r.enabled);
        match reminder.and_then(|r| r.next_fire(habit, now, week_first)) {
            Some(at) => platform::schedule(habit.id, at),
            None => platform::cancel(habit.id),
        }
        if reminder.is_none() || habit.satisfied_on_with_week_start(now.date(), week_first) {
            platform::dismiss(habit.id);
        }
    }
}

/// A reminder alarm went off. Shows the notification if the habit still
/// needs it, then arms the next one (a snooze is one-shot and doesn't).
#[cfg(target_os = "android")]
pub fn on_alarm(id: u64, snoozed: bool) {
    let data = Data::load();
    let prefs = crate::preferences::Preferences::load();
    let Some(habit) = data.habits.iter().find(|h| h.id == id) else {
        return;
    };
    let Some(reminder) = habit.reminder else {
        return;
    };
    let now = crate::clock::now();
    let week_first = prefs.week_start;
    if reminder.should_notify(habit, now.date(), week_first, snoozed) {
        let t = prefs.language.strings();
        let (status, _) = habit.status_on_with_week_start(now.date(), week_first, prefs.language);
        platform::notify(
            id,
            t,
            &habit.name,
            &status,
            // "Later" re-fires once, then the reminder rests for the day.
            !snoozed,
        );
    }
    if !snoozed {
        // A minute's margin, so an alarm delivered a hair early can't
        // re-arm itself for the same moment.
        let after = now + chrono::TimeDelta::minutes(1);
        match reminder.next_fire(habit, after, week_first) {
            Some(at) => platform::schedule(id, at),
            None => platform::cancel(id),
        }
    }
}

/// The notification's Done action: check today in, without opening the
/// app, and swap the notification for a confirmation.
#[cfg(target_os = "android")]
pub fn on_done(id: u64) {
    let mut data = Data::load();
    let today = crate::clock::today();
    let Some(habit) = data.habits.iter().find(|h| h.id == id) else {
        return;
    };
    let name = habit.name.clone();
    if !habit.done_on(today) {
        data.toggle(id, today);
    }
    // Saving re-syncs alarms (and clears the reminder notification).
    data.save();
    EXTERNAL_WRITES.fetch_add(1, Ordering::Relaxed);
    let t = crate::preferences::Preferences::load().language.strings();
    platform::confirm_done(id, t, &name);
}

/// Whether the system currently lets the app post notifications.
pub fn notifications_allowed() -> bool {
    platform::allowed()
}

/// Ask for the notification permission (Android 13+; a no-op once the
/// user has decided, which is when the in-sheet notice takes over).
pub fn request_permission() {
    platform::request_permission();
}

/// The app's notification page in system settings.
pub fn open_settings() {
    platform::open_settings();
}

#[cfg(target_os = "android")]
use crate::android::reminders as platform;

#[cfg(not(target_os = "android"))]
mod platform {
    use chrono::NaiveDateTime;

    pub fn schedule(_id: u64, _at: NaiveDateTime) {}
    pub fn cancel(_id: u64) {}
    pub fn dismiss(_id: u64) {}
    pub fn allowed() -> bool {
        false
    }
    pub fn request_permission() {}
    pub fn open_settings() {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::DEFAULT_STICKING_TARGET;
    use std::collections::BTreeSet;

    fn nd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn at(day: NaiveDate, h: u32, m: u32) -> NaiveDateTime {
        day.and_hms_opt(h, m, 0).unwrap()
    }

    fn habit(schedule: Schedule, days: &[NaiveDate]) -> Habit {
        Habit {
            id: 1,
            name: "Meditate".into(),
            schedule,
            sticking_target: DEFAULT_STICKING_TARGET,
            reminder: None,
            days: days.iter().copied().collect::<BTreeSet<_>>(),
        }
    }

    /// Mon – Fri at 19:00, as in design 15b.
    fn weekdays_at_seven() -> Reminder {
        Reminder {
            days: [true, true, true, true, true, false, false],
            ..Reminder::default()
        }
    }

    // Friday 25 Sep 2026 is the design's reference day.
    fn fri() -> NaiveDate {
        nd(2026, 9, 25)
    }

    #[test]
    fn next_fire_is_later_today_when_still_due() {
        let h = habit(Schedule::Daily, &[]);
        let r = weekdays_at_seven();
        assert_eq!(
            r.next_fire(&h, at(fri(), 8, 0), WeekStart::Monday),
            Some(at(fri(), 19, 0))
        );
    }

    #[test]
    fn next_fire_skips_unpicked_weekdays() {
        let h = habit(Schedule::Daily, &[]);
        let r = weekdays_at_seven();
        // After Friday's reminder: the weekend is off, so Monday.
        assert_eq!(
            r.next_fire(&h, at(fri(), 19, 0), WeekStart::Monday),
            Some(at(nd(2026, 9, 28), 19, 0))
        );
    }

    #[test]
    fn a_done_day_stays_quiet() {
        let h = habit(Schedule::Daily, &[fri()]);
        let r = Reminder::default();
        assert!(!r.should_notify(&h, fri(), WeekStart::Monday, false));
        assert_eq!(
            r.next_fire(&h, at(fri(), 8, 0), WeekStart::Monday),
            Some(at(nd(2026, 9, 26), 19, 0))
        );
    }

    #[test]
    fn a_met_weekly_target_is_quiet_until_next_week() {
        // 2×/week, both done by Tuesday: quiet until Monday 28th.
        let h = habit(
            Schedule::TimesPerWeek { times: 2 },
            &[nd(2026, 9, 21), nd(2026, 9, 22)],
        );
        let r = Reminder::default();
        assert!(!r.should_notify(&h, fri(), WeekStart::Monday, false));
        assert_eq!(
            r.next_fire(&h, at(nd(2026, 9, 22), 20, 0), WeekStart::Monday),
            Some(at(nd(2026, 9, 28), 19, 0))
        );
        // With Sunday weeks, the new week starts on the 27th.
        assert_eq!(
            r.next_fire(&h, at(nd(2026, 9, 22), 20, 0), WeekStart::Sunday),
            Some(at(nd(2026, 9, 27), 19, 0))
        );
    }

    #[test]
    fn every_n_days_waits_out_the_window() {
        let h = habit(Schedule::EveryNDays { n: 3 }, &[fri()]);
        let r = Reminder::default();
        // Done Friday: Sat and Sun are still inside the 3-day window.
        assert_eq!(
            r.next_fire(&h, at(fri(), 20, 0), WeekStart::Monday),
            Some(at(nd(2026, 9, 28), 19, 0))
        );
    }

    #[test]
    fn off_or_dayless_reminders_never_fire() {
        let h = habit(Schedule::Daily, &[]);
        let off = Reminder {
            enabled: false,
            ..Reminder::default()
        };
        let dayless = Reminder {
            days: [false; 7],
            ..Reminder::default()
        };
        assert_eq!(off.next_fire(&h, at(fri(), 8, 0), WeekStart::Monday), None);
        assert_eq!(
            dayless.next_fire(&h, at(fri(), 8, 0), WeekStart::Monday),
            None
        );
        assert!(!off.should_notify(&h, fri(), WeekStart::Monday, true));
    }

    #[test]
    fn a_snooze_ignores_the_weekday_but_not_the_due_state() {
        let r = weekdays_at_seven();
        let sat = nd(2026, 9, 26);
        let open = habit(Schedule::Daily, &[]);
        assert!(!r.should_notify(&open, sat, WeekStart::Monday, false));
        assert!(r.should_notify(&open, sat, WeekStart::Monday, true));
        let done = habit(Schedule::Daily, &[sat]);
        assert!(!r.should_notify(&done, sat, WeekStart::Monday, true));
    }

    #[test]
    fn day_labels_collapse_runs() {
        let en = Language::English;
        assert_eq!(
            weekdays_at_seven().days_label(WeekStart::Monday, en),
            "MON – FRI"
        );
        assert_eq!(
            Reminder::default().days_label(WeekStart::Monday, en),
            "EVERY DAY"
        );
        let mut scattered = Reminder {
            days: [false; 7],
            ..Reminder::default()
        };
        for d in [Weekday::Mon, Weekday::Wed, Weekday::Fri] {
            scattered.toggle_day(d);
        }
        assert_eq!(scattered.days_label(WeekStart::Monday, en), "MON, WED, FRI");
        // Sun, Mon, Tue is a run only in a Sunday-first week.
        let weekend = Reminder {
            days: [true, true, false, false, false, false, true],
            ..Reminder::default()
        };
        assert_eq!(weekend.days_label(WeekStart::Sunday, en), "SUN – TUE");
        assert_eq!(weekend.days_label(WeekStart::Monday, en), "MON, TUE, SUN");
        assert_eq!(
            weekdays_at_seven().summary(WeekStart::Monday, en),
            "19:00 · MON – FRI"
        );
    }

    #[test]
    fn stored_habits_without_a_reminder_read_back_as_none() {
        let json = r#"{"id":1,"name":"x","days":[]}"#;
        let h: Habit = serde_json::from_str(json).unwrap();
        assert_eq!(h.reminder, None);
        // And habits without one don't grow the field when saved.
        let out = serde_json::to_value(&h).unwrap();
        assert!(out.get("reminder").is_none());

        let with = Habit {
            reminder: Some(weekdays_at_seven()),
            ..h
        };
        let back: Habit = serde_json::from_str(&serde_json::to_string(&with).unwrap()).unwrap();
        assert_eq!(back.reminder, Some(weekdays_at_seven()));
    }
}
