# TALLY Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the `habits` PWA as TALLY — the "Ledger, responsive" design (mobile 1a / desktop 2a) in the Modernist system — with a binary done/not-done data model replacing timestamped ticks and the 66-day strength model.

**Architecture:** Storage moves to `habits/v2` (a `BTreeSet<NaiveDate>` of done-days per habit, plus an optional note) with a one-time silent migration from v1. The UI is one DOM for both form factors: a 900px CSS breakpoint switches between a bottom tab bar (mobile) and a left rail + right summary sidebar (desktop). `ui.rs` becomes a `ui/` module: `mod` (app shell), `nav`, `ledger`, `sidebar`, `sheet`.

**Tech Stack:** Rust, Dioxus 0.7 (web/wasm), chrono, gloo-storage, serde. `dx serve` for dev, `cargo test -p habits` runs natively.

**Spec:** `docs/superpowers/specs/2026-07-30-habits-tally-redesign-design.md`

## Global Constraints

- Design tokens (Modernist): bg `#f3f2f2`, surface `#eae9e9`, text `#201e1d`, accent `#ec3013`, divider `color-mix(in srgb, #201e1d 40%, transparent)`, muted text = 55% mix, dim text = 35% mix. Radius `0` everywhere. 2px rules for major structure, 1px for rows. Font Archivo, weights 400/600/800/900, vendored locally — **no runtime Google Fonts request** (offline PWA).
- Uppercase labels are done with CSS `text-transform: uppercase`, not by uppercasing strings in Rust (nav tab literals like `"TODAY"` may be written uppercase directly).
- No new runtime dependencies. `serde_json` is added as a **dev**-dependency only (Task 2).
- Run everything from the repo root unless a step says otherwise. Tests: `cargo test -p habits`. Lint: `cargo clippy -p habits --all-targets -- -D warnings`. Format: `cargo fmt`.
- Pre-commit hooks enforce rustfmt, clippy, and Conventional Commits. Never `--no-verify`.
- **Git is invisible inside this sandbox** (`fatal: not a git repository`). At each Commit step: run `cargo fmt && cargo clippy -p habits --all-targets -- -D warnings && cargo test -p habits` yourself, then print the exact `git add`/`git commit` command and ask the human to run it (in Claude Code they can prefix it with `!`). Do not skip the verification just because you cannot commit.
- Between Task 1 and Task 8 the app UI is intentionally reduced/partial; each task still compiles, passes tests, and lints cleanly.

---

### Task 1: Store v2 — binary days model

Replace the tick/strength model in `store.rs` with the v2 schema and per-habit derived values. Gut `ui.rs` to a minimal compiling shell (the real UI is rebuilt in Tasks 5–8).

**Files:**
- Modify: `crates/habits/src/store.rs` (full rewrite)
- Modify: `crates/habits/src/ui.rs` (reduce to shell)

**Interfaces:**
- Produces (later tasks rely on these exact signatures):
  - `pub struct Habit { pub id: u64, pub name: String, pub note: String, pub days: BTreeSet<NaiveDate> }`
  - `impl Habit`: `done_on(&self, day: NaiveDate) -> bool`, `done_today(&self) -> bool`, `streak(&self) -> usize`, `best_streak(&self) -> usize`, `history(&self, n: u64) -> Vec<(NaiveDate, bool)>`
  - `pub struct Data { next_id: u64, pub habits: Vec<Habit> }` with `load() -> Self`, `save(&self)`, `add(&mut self, name: &str, note: &str)`, `toggle(&mut self, id: u64, day: NaiveDate)`, `rename(&mut self, id: u64, name: &str)`, `set_note(&mut self, id: u64, note: &str)`, `delete(&mut self, id: u64)`
  - `pub fn editable(day: NaiveDate) -> bool`, `pub const EDIT_WINDOW_DAYS: u64 = 7`

- [ ] **Step 1: Rewrite `store.rs` with the new model and failing-first tests**

Replace the entire file with:

```rust
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
        LocalStorage::get(KEY).unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn day(back: u64) -> NaiveDate {
        Local::now().date_naive() - Days::new(back)
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
}
```

- [ ] **Step 2: Gut `ui.rs` to a compiling shell**

Replace the entire file with:

```rust
//! Placeholder shell while the TALLY UI is rebuilt module by module
//! (see docs/superpowers/plans/2026-07-30-habits-tally-redesign.md).

use dioxus::prelude::*;

use crate::store::Data;

static CSS: Asset = asset!("/assets/style.css");

pub fn app() -> Element {
    let data = use_signal(Data::load);
    rsx! {
        document::Stylesheet { href: CSS }
        main {
            h1 { "TALLY" }
            p { "{data().habits.len()} habits" }
        }
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p habits`
Expected: all tests in `store::tests` PASS; no other test modules remain.

- [ ] **Step 4: Lint and format**

Run: `cargo clippy -p habits --all-targets -- -D warnings && cargo fmt`
Expected: clean.

- [ ] **Step 5: Commit (via the human — see Global Constraints)**

```bash
git add crates/habits/src/store.rs crates/habits/src/ui.rs
git commit -m "feat(habits)!: replace tick counts and strength with binary done-days"
```

---

### Task 2: v1 → v2 migration

**Files:**
- Modify: `crates/habits/src/store.rs`
- Modify: `crates/habits/Cargo.toml` (dev-dependency `serde_json`)

**Interfaces:**
- Consumes: `Data`, `Habit` from Task 1.
- Produces: `Data::load()` transparently migrates v1; `pub(crate) mod v1` with `pub struct Data { pub next_id: u64, pub habits: Vec<Habit> }` and `pub struct Habit { pub id: u64, pub name: String, pub ticks: Vec<DateTime<Utc>> }`; `impl Data { pub(crate) fn from_v1(old: v1::Data) -> Self }`.

- [ ] **Step 1: Add the dev-dependency**

In `crates/habits/Cargo.toml` append:

```toml
[dev-dependencies]
serde_json = "1"
```

- [ ] **Step 2: Write the failing tests**

Add to `store.rs`'s `mod tests` (the `tick` helper builds timestamps at local noon so date boundaries and DST can't skew which local day they land on):

```rust
use chrono::{DateTime, Utc};

fn tick(days_back: u64) -> DateTime<Utc> {
    let day = Local::now().date_naive() - Days::new(days_back);
    day.and_hms_opt(12, 0, 0)
        .unwrap()
        .and_local_timezone(Local)
        .unwrap()
        .to_utc()
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
            v1::Habit { id: 5, name: "Read".into(), ticks: vec![] },
        ],
    };
    let data = Data::from_v1(old);
    assert_eq!(data.habits.len(), 2);
    let run = &data.habits[0];
    assert_eq!((run.id, run.name.as_str(), run.note.as_str()), (3, "Run", ""));
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
    let json = r#"{"next_id":2,"habits":[{"id":0,"name":"Run","ticks":["2026-07-28T05:30:00Z"]}]}"#;
    let old: v1::Data = serde_json::from_str(json).unwrap();
    assert_eq!(old.next_id, 2);
    assert_eq!(old.habits[0].ticks.len(), 1);
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p habits`
Expected: FAIL to compile — `v1` and `from_v1` not defined.

- [ ] **Step 4: Implement the migration**

In `store.rs`, add near the top:

```rust
const V1_KEY: &str = "habits/v1";
```

Add the v1 schema module (after `editable`):

```rust
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
```

Replace `Data::load` and add `from_v1`:

```rust
    pub fn load() -> Self {
        if let Ok(data) = LocalStorage::get(KEY) {
            return data;
        }
        // First run on v2: migrate v1 if present, else start empty. The
        // save "commits" the migration; if it fails, we just re-migrate
        // on the next load.
        let migrated: Self = LocalStorage::get(V1_KEY).map(Self::from_v1).unwrap_or_default();
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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p habits`
Expected: PASS.

- [ ] **Step 6: Lint, format, commit (via the human)**

Run: `cargo clippy -p habits --all-targets -- -D warnings && cargo fmt`

```bash
git add crates/habits/src/store.rs crates/habits/Cargo.toml Cargo.lock
git commit -m "feat(habits): migrate v1 tick storage to v2 done-days"
```

---

### Task 3: Collection summary aggregates

**Files:**
- Modify: `crates/habits/src/store.rs`

**Interfaces:**
- Consumes: `Data`, `Habit` from Task 1.
- Produces:

```rust
pub struct Summary {
    pub done: usize,                  // habits done today
    pub total: usize,                 // habit count
    pub week: Vec<(NaiveDate, f64)>,  // last 7 days (oldest first), fraction of habits done
    pub best: Option<(usize, String)>,// best streak ever across habits + that habit's name
    pub day_number: Option<u64>,      // days since the earliest recorded day, 1-based
}
impl Data { pub fn summary(&self) -> Summary }
```

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
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
    assert!(Data::default().summary().week.iter().all(|&(_, f)| f == 0.0));
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
    assert_eq!(data_with(vec![habit(&[9]), habit(&[2])]).summary().day_number, Some(10));
    assert_eq!(data_with(vec![habit(&[0])]).summary().day_number, Some(1));
    assert_eq!(data_with(vec![habit(&[])]).summary().day_number, None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p habits`
Expected: FAIL to compile — `summary` not defined.

- [ ] **Step 3: Implement `Summary`**

Add to `store.rs` (after the `Data` impl):

```rust
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
                (day, if total == 0 { 0.0 } else { done as f64 / total as f64 })
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p habits`
Expected: PASS.

- [ ] **Step 5: Lint, format, commit (via the human)**

Run: `cargo clippy -p habits --all-targets -- -D warnings && cargo fmt`

```bash
git add crates/habits/src/store.rs
git commit -m "feat(habits): add collection summary for header and sidebar"
```

---

### Task 4: Fonts + Modernist stylesheet

Vendor Archivo and write the complete TALLY stylesheet. The UI classes it defines are consumed by Tasks 5–8.

**Files:**
- Create: `crates/habits/assets/fonts/archivo-{400,600,800,900}.woff2`
- Modify: `crates/habits/assets/style.css` (full rewrite)
- Modify: `crates/habits/index.html` (title, theme color)

**Interfaces:**
- Produces: CSS classes used by Tasks 5–8 (`shell`, `main`, `rail`, `rail-tab`, `rail-new`, `brand`, `brand-dot`, `bar`, `bar-tab`, `bar-new`, `head`, `head-row`, `head-date`, `head-day`, `title`, `head-score`, `score`, `of`, `meter`, `col-head`, `ch-*`, `ledger`, `row`, `box`, `row-name`, `name`, `note`, `dots`, `dot`, `streak-n`, `empty`, `side`, `side-block`, `side-label`, `side-pct`, `side-note`, `bars`, `bars-days`, `best`, `best-n`, `best-name`, `overlay`, `sheet`, `sheet-label`, `sheet-name`, `sheet-note`, `sheet-stats`, `form`, `input`, `btn`, `btn-quiet`, `danger`, `cal-nav`, `cal-title`, `cal-grid`, `cal-wd`, `cal-blank`, `cal-day` with modifiers `done`/`today`/`off`/`locked`, `sheet-del`). Font files at `assets/fonts/archivo-<weight>.woff2` (Task 5 declares them with `asset!`).

- [ ] **Step 1: Download the Archivo woff2 files**

```bash
cd crates/habits/assets && mkdir -p fonts
UA='Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36'
for w in 400 600 800 900; do
  url=$(curl -s -A "$UA" "https://fonts.googleapis.com/css2?family=Archivo:wght@$w" \
    | awk '/\/\* latin \*\//{f=1} f && match($0,/https:[^)]*\.woff2/){print substr($0,RSTART,RLENGTH); exit}')
  curl -s -o "fonts/archivo-$w.woff2" "$url"
done
ls -la fonts/   # four files, each roughly 15–30 KB
```

Each file must be a real woff2 (starts with `wOF2`): `head -c4 fonts/archivo-400.woff2` → `wOF2`. If the network is unavailable, get Archivo TTFs from `nix build nixpkgs#google-fonts` (`share/fonts/truetype/Archivo[wght].ttf` is a variable font — instantiate weights with `fonttools varLib.instancer` and convert with `nix run nixpkgs#woff2 -- woff2_compress`), but try the direct download first.

- [ ] **Step 2: Rewrite `assets/style.css`**

Replace the entire file with:

```css
/* TALLY — Modernist: Archivo, red on light ground, zero radius, 2px rules,
   flush-left everything. @font-face rules are injected from Rust (the woff2
   files go through the asset system, so only Rust knows their hashed URLs). */

:root {
  --bg: #f3f2f2;
  --surface: #eae9e9;
  --text: #201e1d;
  --accent: #ec3013;
  --accent-down: #ae1800;
  --divider: color-mix(in srgb, var(--text) 40%, transparent);
  --muted: color-mix(in srgb, var(--text) 55%, transparent);
  --dim: color-mix(in srgb, var(--text) 35%, transparent);
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html {
  color-scheme: light;
}

body {
  font-family: "Archivo", system-ui, sans-serif;
  background: var(--bg);
  color: var(--text);
  min-height: 100dvh;
  -webkit-tap-highlight-color: transparent;
}

button {
  font: inherit;
  color: inherit;
  background: none;
  border: none;
  border-radius: 0;
  cursor: pointer;
  text-align: left;
}

button:disabled {
  cursor: default;
}

:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}

::selection {
  background: color-mix(in srgb, var(--accent) 30%, transparent);
}

/* --- layout shell -------------------------------------------------- */

.shell {
  display: flex;
  min-height: 100dvh;
}

.main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

/* --- desktop rail (hidden on mobile) ------------------------------- */

.rail {
  width: 200px;
  flex: none;
  display: none;
  flex-direction: column;
  border-right: 2px solid var(--divider);
}

.brand {
  padding: 24px 20px;
  border-bottom: 2px solid var(--divider);
  font-size: 20px;
  font-weight: 900;
  letter-spacing: -0.01em;
}

.brand-dot {
  color: var(--accent);
}

.rail-tab {
  min-height: 48px;
  display: flex;
  align-items: center;
  padding: 0 20px;
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.12em;
  color: var(--muted);
  border-bottom: 1px solid var(--divider);
}

.rail-tab.on {
  color: var(--accent);
  box-shadow: inset 3px 0 0 var(--accent);
}

.rail-new {
  margin-top: auto;
  border-top: 2px solid var(--divider);
}

.rail-new button {
  width: 100%;
  min-height: 56px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 20px;
  background: var(--text);
  color: var(--bg);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.12em;
}

.plus-sign {
  font-size: 20px;
  font-weight: 400;
  line-height: 1;
}

/* --- mobile bottom bar (hidden on desktop) -------------------------- */

.bar {
  display: flex;
  border-top: 2px solid var(--divider);
  position: sticky;
  bottom: 0;
  background: var(--bg);
  padding-bottom: env(safe-area-inset-bottom);
}

.bar-tab {
  flex: 1;
  min-height: 52px;
  display: flex;
  align-items: center;
  padding: 0 20px;
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.12em;
  color: var(--muted);
  border-right: 1px solid var(--divider);
}

.bar-tab.on {
  color: var(--accent);
}

.bar-new {
  width: 52px;
  min-height: 52px;
  flex: none;
  background: var(--text);
  color: var(--bg);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  font-weight: 400;
}

/* --- Today header ---------------------------------------------------- */

.head {
  padding: calc(20px + env(safe-area-inset-top)) 20px 14px;
  border-bottom: 2px solid var(--divider);
}

.head-row {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
}

.head-date {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--accent);
}

.head-day {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  color: var(--muted);
}

.title {
  font-size: 44px;
  font-weight: 900;
  letter-spacing: -0.02em;
  margin-top: 6px;
  line-height: 1;
}

.head-score {
  display: flex;
  align-items: baseline;
  gap: 8px;
  margin-top: 12px;
}

.score {
  font-size: 26px;
  font-weight: 800;
  line-height: 1;
}

.score .of {
  color: var(--dim);
}

.meter {
  flex: 1;
  height: 8px;
  background: var(--surface);
  border: 1px solid var(--divider);
  align-self: center;
}

.meter div {
  height: 100%;
  background: var(--accent);
}

/* --- ledger ---------------------------------------------------------- */

.col-head {
  display: none;
  padding: 0 32px;
  min-height: 36px;
  align-items: center;
  gap: 14px;
  border-bottom: 1px solid var(--divider);
}

.col-head span {
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.14em;
  color: var(--muted);
}

.ch-box {
  width: 28px;
  flex: none;
}

.ch-name {
  flex: 1;
}

.ch-days {
  width: 238px;
  flex: none;
}

.ch-streak {
  width: 52px;
  flex: none;
  text-align: right;
}

.ledger {
  flex: 1;
  overflow: auto;
}

.row {
  display: flex;
  align-items: center;
  gap: 14px;
  min-height: 64px;
  padding: 0 20px;
  border-bottom: 1px solid var(--divider);
  cursor: pointer;
  user-select: none;
}

.box {
  width: 28px;
  height: 28px;
  flex: none;
  border: 2px solid var(--text);
  background: transparent;
  display: flex;
  align-items: center;
  justify-content: center;
}

.box.done {
  background: var(--accent);
  border-color: var(--accent);
}

.box:active {
  border-color: var(--accent-down);
}

.row-name {
  flex: 1;
  min-width: 0;
}

.row-name .name {
  font-size: 16px;
  font-weight: 600;
  overflow-wrap: break-word;
}

.row-name .note {
  font-size: 11px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--muted);
}

.dots {
  width: 238px;
  flex: none;
  display: none;
  gap: 4px;
}

.dot {
  width: 13px;
  height: 13px;
  background: var(--surface);
  border: 1px solid var(--divider);
}

.dot.done {
  background: var(--accent);
  border-color: var(--accent);
}

.dot.today {
  outline: 2px solid var(--text);
  outline-offset: 1px;
}

.streak-n {
  width: 52px;
  flex: none;
  text-align: right;
  font-size: 20px;
  font-weight: 800;
}

.streak-n.dim {
  color: var(--dim);
}

.empty {
  padding: 40px 20px;
  color: var(--muted);
  line-height: 1.5;
  max-width: 30rem;
}

/* --- desktop sidebar -------------------------------------------------- */

.side {
  width: 280px;
  flex: none;
  display: none;
  flex-direction: column;
  border-left: 2px solid var(--divider);
}

.side-block {
  padding: 20px 24px;
  border-bottom: 1px solid var(--divider);
}

.side-label {
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--muted);
}

.side-pct {
  font-size: 64px;
  font-weight: 900;
  letter-spacing: -0.03em;
  line-height: 1;
  margin-top: 10px;
  color: var(--accent);
}

.side-pct span {
  font-size: 30px;
  font-weight: 800;
  letter-spacing: 0;
}

.side-note {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  margin-top: 8px;
  color: var(--muted);
}

.bars {
  display: flex;
  gap: 5px;
  margin-top: 12px;
  align-items: flex-end;
  height: 64px;
}

.bars > div {
  flex: 1;
  background: var(--accent);
}

.bars > div.today {
  background: var(--surface);
  border: 1px solid var(--divider);
}

.bars-days {
  display: flex;
  gap: 5px;
  margin-top: 6px;
}

.bars-days span {
  flex: 1;
  font-size: 9px;
  font-weight: 600;
  letter-spacing: 0.08em;
  color: var(--muted);
}

.bars-days span.today {
  color: var(--accent);
}

.best {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-top: 10px;
}

.best-n {
  font-size: 28px;
  font-weight: 800;
  line-height: 1;
}

.best-name {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--muted);
}

/* --- sheets ----------------------------------------------------------- */

.overlay {
  position: fixed;
  inset: 0;
  z-index: 10;
  background: color-mix(in srgb, var(--text) 50%, transparent);
  display: flex;
  align-items: flex-end;
  justify-content: center;
  animation: fade 0.15s ease-out;
}

.sheet {
  width: 100%;
  max-width: 28rem;
  background: var(--bg);
  border-top: 2px solid var(--text);
  padding: 20px 20px calc(20px + env(safe-area-inset-bottom));
  animation: rise 0.2s ease-out;
}

@keyframes fade {
  from {
    opacity: 0;
  }
}

@keyframes rise {
  from {
    transform: translateY(1.5rem);
    opacity: 0;
  }
}

.sheet-label {
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--muted);
  margin-bottom: 10px;
}

.sheet-name {
  display: block;
  font-size: 20px;
  font-weight: 900;
  letter-spacing: -0.01em;
  padding: 0;
}

.sheet-note {
  font-size: 11px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--muted);
  margin-top: 2px;
}

.sheet-stats {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--muted);
  margin-top: 8px;
}

.form {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 4px;
}

.input {
  width: 100%;
  min-height: 44px;
  padding: 6px 12px;
  font: inherit;
  font-size: 16px; /* ≥16px so iOS doesn't zoom on focus */
  color: var(--text);
  caret-color: var(--accent);
  background: var(--surface);
  border: 1px solid var(--divider);
  border-radius: 0;
}

.input:focus-visible {
  border-color: var(--accent);
  outline-offset: 0;
}

.btn {
  min-height: 44px;
  padding: 0 20px;
  background: var(--accent);
  color: var(--bg);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-align: center;
}

.btn:active {
  background: var(--accent-down);
}

.btn:disabled {
  opacity: 0.45;
}

.btn-quiet {
  min-height: 36px;
  min-width: 44px;
  padding: 0 12px;
  border: 1px solid var(--divider);
  color: var(--muted);
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-align: center;
}

.btn-quiet:disabled {
  opacity: 0.35;
}

.btn-quiet.danger {
  color: var(--accent);
  border-color: var(--accent);
}

/* --- sheet calendar ---------------------------------------------------- */

.cal-nav {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 16px 0 10px;
}

.cal-title {
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.cal-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
}

.cal-wd {
  text-align: center;
  font-size: 10px;
  font-weight: 800;
  letter-spacing: 0.08em;
  color: var(--muted);
  padding-bottom: 2px;
}

.cal-day {
  aspect-ratio: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  font-weight: 600;
  background: var(--surface);
  border: 1px solid var(--divider);
  text-align: center;
}

.cal-day.done {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--bg);
}

.cal-day.today {
  outline: 2px solid var(--text);
  outline-offset: -2px;
}

.cal-day.locked {
  cursor: default;
}

.cal-day.locked:not(.done) {
  color: var(--muted);
}

.cal-day.off {
  opacity: 0.3;
  cursor: default;
}

.sheet-del {
  display: flex;
  justify-content: flex-start;
  margin-top: 16px;
}

/* --- desktop (the 2a layout) ------------------------------------------- */

@media (min-width: 900px) {
  .rail {
    display: flex;
  }

  .side {
    display: flex;
  }

  .bar {
    display: none;
  }

  .col-head {
    display: flex;
  }

  .dots {
    display: flex;
  }

  .head {
    padding: 24px 32px 18px;
  }

  .title {
    font-size: 52px;
  }

  .head-score {
    gap: 24px;
    margin-top: 6px;
  }

  .score {
    font-size: 28px;
  }

  .row {
    padding: 0 32px;
  }

  .overlay {
    align-items: center;
    padding: 16px;
  }

  .sheet {
    border: 2px solid var(--text);
  }
}

@media (prefers-reduced-motion: reduce) {
  .overlay,
  .sheet,
  .meter div {
    animation: none;
    transition: none;
  }
}
```

- [ ] **Step 3: Update `index.html`**

Change only these lines (keep the rest, including the comment about root-relative PWA files):

- `<meta name="theme-color" content="#16241f" />` → `<meta name="theme-color" content="#f3f2f2" />`
- `<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent" />` → `content="default"`
- `<title>Habits</title>` → `<title>TALLY</title>`

- [ ] **Step 4: Verify the app still builds and serves**

Run: `cargo test -p habits` (still green) and, from `crates/habits`, `dx serve` briefly — the placeholder shell should render with the light background once Task 5 wires the fonts (at this point Archivo isn't loaded yet; that's expected).

- [ ] **Step 5: Commit (via the human)**

```bash
git add crates/habits/assets crates/habits/index.html
git commit -m "feat(habits): vendor Archivo and add TALLY Modernist stylesheet"
```

---

### Task 5: UI shell — `ui/` module, fonts wiring, nav

**Files:**
- Delete: `crates/habits/src/ui.rs`
- Create: `crates/habits/src/ui/mod.rs`
- Create: `crates/habits/src/ui/nav.rs`
- Create: `crates/habits/src/ui/ledger.rs` (stub, replaced in Task 6)
- Create: `crates/habits/src/ui/sidebar.rs` (stub, replaced in Task 7)
- Create: `crates/habits/src/ui/sheet.rs` (stub, replaced in Task 8)

**Interfaces:**
- Consumes: `Data::load` (Task 1); CSS classes and font files (Task 4).
- Produces: `pub struct Overlays` (Copy) with signals `detail: Signal<Option<u64>>`, `adding: Signal<bool>`, `month: Signal<NaiveDate>`, `editing: Signal<bool>`, `name_draft: Signal<String>`, `note_draft: Signal<String>`, `confirm: Signal<bool>` and methods `open_detail(&mut self, id: u64)`, `open_add(&mut self)`; module functions `nav::rail(Overlays) -> Element`, `nav::bottom_bar(Overlays) -> Element`, and stubs `ledger::ledger(Signal<Data>, Overlays) -> Element`, `sidebar::sidebar(Signal<Data>) -> Element`, `sheet::detail_sheet(Signal<Data>, Overlays) -> Element`, `sheet::add_sheet(Signal<Data>, Overlays) -> Element`.

- [ ] **Step 1: Create `ui/mod.rs`** (and delete `ui.rs`; `main.rs` needs no change — `mod ui;` resolves to the directory)

```rust
//! The TALLY UI. State is a single [`Data`] signal, persisted to
//! localStorage after every mutation. One DOM serves both form factors:
//! a 900px CSS breakpoint switches between the mobile bottom bar (design
//! option 1a) and the desktop rail + sidebar (2a).

mod ledger;
mod nav;
mod sheet;
mod sidebar;

use chrono::{Local, NaiveDate};
use dioxus::prelude::*;

use crate::store::Data;

/// Tracked through the dioxus asset system (not inlined in index.html) so
/// `dx serve` hot-reloads style edits without a rebuild.
static CSS: Asset = asset!("/assets/style.css");
static ARCHIVO_400: Asset = asset!("/assets/fonts/archivo-400.woff2");
static ARCHIVO_600: Asset = asset!("/assets/fonts/archivo-600.woff2");
static ARCHIVO_800: Asset = asset!("/assets/fonts/archivo-800.woff2");
static ARCHIVO_900: Asset = asset!("/assets/fonts/archivo-900.woff2");

/// @font-face rules live here rather than in the stylesheet: the woff2
/// files go through the asset system (hashed filenames), so only Rust
/// knows their URLs.
fn font_faces() -> String {
    [
        (400, ARCHIVO_400),
        (600, ARCHIVO_600),
        (800, ARCHIVO_800),
        (900, ARCHIVO_900),
    ]
    .into_iter()
    .map(|(weight, font)| {
        format!(
            "@font-face{{font-family:'Archivo';font-style:normal;\
             font-weight:{weight};font-display:swap;\
             src:url('{font}') format('woff2')}}"
        )
    })
    .collect()
}

/// Signals for the overlays (detail sheet and add form), created once in
/// [`app`] and passed down by copy.
#[derive(Clone, Copy)]
pub struct Overlays {
    /// Habit whose detail sheet is open.
    pub detail: Signal<Option<u64>>,
    pub adding: Signal<bool>,
    /// Calendar month shown in the detail sheet.
    pub month: Signal<NaiveDate>,
    /// Name/note edit mode inside the detail sheet.
    pub editing: Signal<bool>,
    pub name_draft: Signal<String>,
    pub note_draft: Signal<String>,
    /// Delete confirm armed.
    pub confirm: Signal<bool>,
}

impl Overlays {
    pub fn open_detail(&mut self, id: u64) {
        self.month.set(Local::now().date_naive());
        self.editing.set(false);
        self.confirm.set(false);
        self.detail.set(Some(id));
    }

    pub fn open_add(&mut self) {
        self.name_draft.set(String::new());
        self.note_draft.set(String::new());
        self.adding.set(true);
    }
}

pub fn app() -> Element {
    let data = use_signal(Data::load);
    let overlays = Overlays {
        detail: use_signal(|| None),
        adding: use_signal(|| false),
        month: use_signal(|| Local::now().date_naive()),
        editing: use_signal(|| false),
        name_draft: use_signal(String::new),
        note_draft: use_signal(String::new),
        confirm: use_signal(|| false),
    };

    rsx! {
        document::Stylesheet { href: CSS }
        document::Style { {font_faces()} }
        div { class: "shell",
            {nav::rail(overlays)}
            main { class: "main",
                {ledger::ledger(data, overlays)}
                {nav::bottom_bar(overlays)}
            }
            {sidebar::sidebar(data)}
        }
        {sheet::detail_sheet(data, overlays)}
        {sheet::add_sheet(data, overlays)}
    }
}
```

- [ ] **Step 2: Create `ui/nav.rs`**

```rust
//! Navigation chrome: the desktop rail and the mobile bottom bar. Only
//! TODAY is live — HABITS / STATS / SETTINGS exist in the design but have
//! no screens yet, so they render muted and inert.

use dioxus::prelude::*;

use super::Overlays;

const TABS: [&str; 3] = ["TODAY", "HABITS", "STATS"];

pub fn rail(mut overlays: Overlays) -> Element {
    rsx! {
        div { class: "rail",
            div { class: "brand",
                "TALLY"
                span { class: "brand-dot", "." }
            }
            for tab in TABS {
                span {
                    class: if tab == "TODAY" { "rail-tab on" } else { "rail-tab" },
                    aria_disabled: tab != "TODAY",
                    "{tab}"
                }
            }
            span { class: "rail-tab", aria_disabled: true, "SETTINGS" }
            div { class: "rail-new",
                button { onclick: move |_| overlays.open_add(),
                    span { class: "plus-sign", "+" }
                    "NEW HABIT"
                }
            }
        }
    }
}

pub fn bottom_bar(mut overlays: Overlays) -> Element {
    rsx! {
        div { class: "bar",
            for tab in TABS {
                span {
                    class: if tab == "TODAY" { "bar-tab on" } else { "bar-tab" },
                    aria_disabled: tab != "TODAY",
                    "{tab}"
                }
            }
            button {
                class: "bar-new",
                title: "New habit",
                onclick: move |_| overlays.open_add(),
                "+"
            }
        }
    }
}
```

- [ ] **Step 3: Create the three stubs**

`ui/ledger.rs`:

```rust
//! The Today ledger (real implementation in the ledger task).

use dioxus::prelude::*;

use super::Overlays;
use crate::store::Data;

pub fn ledger(data: Signal<Data>, _overlays: Overlays) -> Element {
    rsx! {
        div { class: "ledger",
            p { class: "empty", "{data().habits.len()} habits" }
        }
    }
}
```

`ui/sidebar.rs`:

```rust
//! Desktop summary sidebar (real implementation in the sidebar task).

use dioxus::prelude::*;

use crate::store::Data;

pub fn sidebar(_data: Signal<Data>) -> Element {
    rsx! {
        aside { class: "side" }
    }
}
```

`ui/sheet.rs`:

```rust
//! Overlay sheets (real implementation in the sheets task).

use dioxus::prelude::*;

use super::Overlays;
use crate::store::Data;

pub fn detail_sheet(_data: Signal<Data>, _overlays: Overlays) -> Element {
    rsx! {}
}

pub fn add_sheet(_data: Signal<Data>, _overlays: Overlays) -> Element {
    rsx! {}
}
```

- [ ] **Step 4: Verify**

Run: `cargo test -p habits && cargo clippy -p habits --all-targets -- -D warnings && cargo fmt`
Then from `crates/habits`: `dx serve`, open at a narrow and a wide viewport: mobile shows the bottom bar (TODAY accent-red, HABITS/STATS muted, black + square); desktop shows the rail with the TALLY. wordmark, inset red bar on TODAY, and the black + NEW HABIT block pinned to the bottom. Archivo renders (inspect any label's computed font).

- [ ] **Step 5: Commit (via the human)**

```bash
git add crates/habits/src
git commit -m "feat(habits): TALLY ui shell with rail and bottom-bar navigation"
```

---

### Task 6: The ledger

**Files:**
- Modify: `crates/habits/src/ui/ledger.rs` (full rewrite of the stub)

**Interfaces:**
- Consumes: `Habit::{done_today, history, streak}`, `Data::{toggle, save, summary}`, `Summary` (Tasks 1, 3); `Overlays::open_detail` (Task 5).
- Produces: `ledger(data: Signal<Data>, overlays: Overlays) -> Element` — same signature as the stub.

- [ ] **Step 1: Rewrite `ui/ledger.rs`**

```rust
//! The Today ledger: date header with the done/total meter, then one
//! strong list of habit rows — checkbox, name + note, a 14-day dot strip
//! (desktop only), and the streak numeral.

use chrono::Local;
use dioxus::prelude::*;

use super::Overlays;
use crate::store::Data;

pub fn ledger(mut data: Signal<Data>, mut overlays: Overlays) -> Element {
    let summary = data().summary();
    let date = Local::now().format("%a %-d %b %Y").to_string();
    let pct = if summary.total == 0 {
        0.0
    } else {
        summary.done as f64 * 100.0 / summary.total as f64
    };
    let today = Local::now().date_naive();

    rsx! {
        div { class: "head",
            div { class: "head-row",
                span { class: "head-date", "{date}" }
                if let Some(n) = summary.day_number {
                    span { class: "head-day", "DAY {n}" }
                }
            }
            h1 { class: "title", "Today" }
            div { class: "head-score",
                span { class: "score",
                    "{summary.done}"
                    span { class: "of", "/{summary.total}" }
                }
                div { class: "meter", div { style: "width:{pct:.0}%" } }
            }
        }
        div { class: "col-head",
            span { class: "ch-box" }
            span { class: "ch-name", "HABIT" }
            span { class: "ch-days", "LAST 14 DAYS" }
            span { class: "ch-streak", "STREAK" }
        }
        div { class: "ledger",
            if data().habits.is_empty() {
                p { class: "empty",
                    "Nothing here yet. Tap + to add a habit, then tick it off each day you do it."
                }
            }
            for habit in data().habits {
                div {
                    key: "{habit.id}",
                    class: "row",
                    role: "button",
                    onclick: move |_| overlays.open_detail(habit.id),
                    button {
                        class: if habit.done_today() { "box done" } else { "box" },
                        aria_pressed: habit.done_today(),
                        title: "Done today — tap to toggle",
                        onclick: move |e| {
                            e.stop_propagation();
                            data.with_mut(|d| {
                                d.toggle(habit.id, today);
                                d.save();
                            });
                        },
                        if habit.done_today() {
                            svg {
                                width: "16",
                                height: "16",
                                view_box: "0 0 24 24",
                                fill: "none",
                                stroke: "#f3f2f2",
                                stroke_width: "3.5",
                                path { d: "M4 12.5l5 5L20 6.5" }
                            }
                        }
                    }
                    div { class: "row-name",
                        div { class: "name", "{habit.name}" }
                        if !habit.note.is_empty() {
                            div { class: "note", "{habit.note}" }
                        }
                    }
                    div { class: "dots",
                        for (i , (day , done)) in habit.history(14).into_iter().enumerate() {
                            span {
                                class: match (done, i == 13) {
                                    (true, true) => "dot done today",
                                    (true, false) => "dot done",
                                    (false, true) => "dot today",
                                    (false, false) => "dot",
                                },
                                title: "{day}",
                            }
                        }
                    }
                    span {
                        class: if habit.done_today() { "streak-n" } else { "streak-n dim" },
                        "{habit.streak()}"
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Verify**

Run: `cargo test -p habits && cargo clippy -p habits --all-targets -- -D warnings && cargo fmt`
Then `dx serve` (from `crates/habits`): add nothing yet — the empty state shows. In the browser console, seed data if useful: `localStorage.setItem('habits/v2', JSON.stringify({next_id:2, habits:[{id:0,name:'Morning run',note:'06:30 · 5 KM',days:[]},{id:1,name:'Read 20 pages',note:'ANY TIME',days:[]}]}))` then reload. Tick a checkbox: it fills red with the check, the header meter moves, the streak numeral goes full-strength. Untick: reverses. Desktop width shows the dot strip with today outlined; mobile hides it.

- [ ] **Step 3: Commit (via the human)**

```bash
git add crates/habits/src/ui/ledger.rs
git commit -m "feat(habits): ledger rows and Today header"
```

---

### Task 7: The sidebar

**Files:**
- Modify: `crates/habits/src/ui/sidebar.rs` (full rewrite of the stub)

**Interfaces:**
- Consumes: `Data::summary`, `Summary` (Task 3).
- Produces: `sidebar(data: Signal<Data>) -> Element` — same signature as the stub.

- [ ] **Step 1: Rewrite `ui/sidebar.rs`**

```rust
//! Desktop-only summary column: today's completion, the week's bars, and
//! the best streak across all habits. Hidden below 900px by CSS.

use dioxus::prelude::*;

use crate::store::Data;

pub fn sidebar(data: Signal<Data>) -> Element {
    let s = data().summary();
    let pct = if s.total == 0 {
        0
    } else {
        (s.done * 100 + s.total / 2) / s.total
    };
    let left = s.total - s.done;
    let note = if s.total == 0 {
        "NO HABITS YET".to_string()
    } else if left == 0 {
        "ALL DONE".to_string()
    } else {
        format!("{left} LEFT BEFORE MIDNIGHT")
    };

    rsx! {
        aside { class: "side",
            div { class: "side-block",
                div { class: "side-label", "Completion" }
                div { class: "side-pct",
                    "{pct}"
                    span { "%" }
                }
                div { class: "side-note", "{note}" }
            }
            div { class: "side-block",
                div { class: "side-label", "This week" }
                div { class: "bars",
                    for (i , (day , frac)) in s.week.iter().enumerate() {
                        div {
                            key: "{day}",
                            class: if i == 6 { "today" } else { "" },
                            style: "height:{frac * 100.0:.0}%",
                        }
                    }
                }
                div { class: "bars-days",
                    for (i , (day , _)) in s.week.iter().enumerate() {
                        span {
                            key: "{day}",
                            class: if i == 6 { "today" } else { "" },
                            {day.format("%a").to_string()[..2].to_uppercase()}
                        }
                    }
                }
            }
            div { class: "side-block",
                div { class: "side-label", "Best streak" }
                if let Some((n, name)) = s.best {
                    div { class: "best",
                        span { class: "best-n", "{n}" }
                        span { class: "best-name", "{name}" }
                    }
                } else {
                    div { class: "best",
                        span { class: "best-name", "—" }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Verify**

Run: `cargo test -p habits && cargo clippy -p habits --all-targets -- -D warnings && cargo fmt`
Then `dx serve` at ≥900px: COMPLETION % matches done/total rounded; THIS WEEK shows 7 bars, today's outlined in surface grey with an accent weekday label; BEST STREAK shows the longest-ever run with its habit name (— when nothing is recorded). Ticking in the ledger updates all three blocks live.

- [ ] **Step 3: Commit (via the human)**

```bash
git add crates/habits/src/ui/sidebar.rs
git commit -m "feat(habits): desktop summary sidebar"
```

---

### Task 8: The sheets — detail and add

**Files:**
- Modify: `crates/habits/src/ui/sheet.rs` (full rewrite of the stub)

**Interfaces:**
- Consumes: `Habit::{done_on, streak, best_streak}`, `Data::{toggle, rename, set_note, delete, save}`, `editable` (Tasks 1–2); `Overlays` signals (Task 5).
- Produces: `detail_sheet(Signal<Data>, Overlays) -> Element`, `add_sheet(Signal<Data>, Overlays) -> Element` — same signatures as the stubs; private `month_cells(NaiveDate) -> Vec<Option<NaiveDate>>` with its unit test.

- [ ] **Step 1: Rewrite `ui/sheet.rs`**

```rust
//! Overlay sheets: the per-habit detail (month calendar with binary
//! toggling inside the edit window, name/note editing, delete behind a
//! two-tap confirm) and the add-habit form.

use chrono::{Datelike, Local, Months, NaiveDate};
use dioxus::prelude::*;

use super::Overlays;
use crate::store::{Data, editable};

/// The days of the month containing `month`, Monday-first, with leading
/// `None`s so indices line up with a 7-column grid.
fn month_cells(month: NaiveDate) -> Vec<Option<NaiveDate>> {
    let first = month.with_day(1).expect("every month has a day 1");
    let mut cells = vec![None; first.weekday().num_days_from_monday() as usize];
    let mut day = first;
    while day.month() == first.month() {
        cells.push(Some(day));
        match day.succ_opt() {
            Some(next) => day = next,
            None => break,
        }
    }
    cells
}

pub fn detail_sheet(mut data: Signal<Data>, mut overlays: Overlays) -> Element {
    let Some(id) = (overlays.detail)() else {
        return rsx! {};
    };
    let Some(habit) = data().habits.into_iter().find(|h| h.id == id) else {
        return rsx! {};
    };

    let today = Local::now().date_naive();
    let shown = (overlays.month)().with_day(1).expect("every month has a day 1");
    let this_month = today.with_day(1).expect("every month has a day 1");
    let name = habit.name.clone();
    let note = habit.note.clone();

    let mut save = move || {
        data.with_mut(|d| {
            d.rename(id, &(overlays.name_draft)());
            d.set_note(id, &(overlays.note_draft)());
            d.save();
        });
        overlays.editing.set(false);
    };

    let day_cells: Vec<Element> = month_cells(shown)
        .into_iter()
        .map(|cell| {
            let Some(day) = cell else {
                return rsx! {
                    span { class: "cal-blank" }
                };
            };
            let done = habit.done_on(day);
            let mut cls = String::from("cal-day");
            if done {
                cls.push_str(" done");
            }
            if day == today {
                cls.push_str(" today");
            }
            if day > today {
                cls.push_str(" off");
            } else if !editable(day) {
                cls.push_str(" locked");
            }
            rsx! {
                button {
                    class: "{cls}",
                    disabled: day > today,
                    title: if editable(day) { "{day} — tap to toggle" } else { "{day}" },
                    onclick: move |_| {
                        if editable(day) {
                            data.with_mut(|d| {
                                d.toggle(id, day);
                                d.save();
                            });
                        }
                    },
                    "{day.day()}"
                }
            }
        })
        .collect();

    rsx! {
        div { class: "overlay", onclick: move |_| overlays.detail.set(None),
            div { class: "sheet", onclick: move |e| e.stop_propagation(),
                div { class: "sheet-label", "Habit" }
                if (overlays.editing)() {
                    div { class: "form",
                        input {
                            class: "input",
                            value: "{overlays.name_draft}",
                            enterkeyhint: "done",
                            onmounted: move |e| async move {
                                let _ = e.data().set_focus(true).await;
                            },
                            oninput: move |e| overlays.name_draft.set(e.value()),
                            onkeydown: move |e| {
                                if e.key() == Key::Enter {
                                    save();
                                } else if e.key() == Key::Escape {
                                    overlays.editing.set(false);
                                }
                            },
                        }
                        input {
                            class: "input",
                            value: "{overlays.note_draft}",
                            placeholder: "Note — e.g. 06:30 · 5 KM",
                            enterkeyhint: "done",
                            oninput: move |e| overlays.note_draft.set(e.value()),
                            onkeydown: move |e| {
                                if e.key() == Key::Enter {
                                    save();
                                } else if e.key() == Key::Escape {
                                    overlays.editing.set(false);
                                }
                            },
                        }
                        button {
                            class: "btn",
                            disabled: (overlays.name_draft)().trim().is_empty(),
                            onclick: move |_| save(),
                            "SAVE"
                        }
                    }
                } else {
                    button {
                        class: "sheet-name",
                        title: "Edit name and note",
                        onclick: move |_| {
                            overlays.name_draft.set(name.clone());
                            overlays.note_draft.set(note.clone());
                            overlays.editing.set(true);
                        },
                        "{habit.name} ✎"
                    }
                    if !habit.note.is_empty() {
                        div { class: "sheet-note", "{habit.note}" }
                    }
                    div { class: "sheet-stats",
                        "Streak {habit.streak()} · best {habit.best_streak()}"
                    }
                }
                div { class: "cal-nav",
                    button {
                        class: "btn-quiet",
                        title: "Earlier month",
                        onclick: move |_| overlays.month.set(shown - Months::new(1)),
                        "‹"
                    }
                    span { class: "cal-title", {shown.format("%B %Y").to_string()} }
                    button {
                        class: "btn-quiet",
                        title: "Later month",
                        disabled: shown >= this_month,
                        onclick: move |_| overlays.month.set(shown + Months::new(1)),
                        "›"
                    }
                }
                div { class: "cal-grid",
                    for wd in ["M", "T", "W", "T", "F", "S", "S"] {
                        span { class: "cal-wd", "{wd}" }
                    }
                    {day_cells.into_iter()}
                }
                div { class: "sheet-del",
                    if (overlays.confirm)() {
                        button {
                            class: "btn-quiet danger",
                            title: "Really delete",
                            onclick: move |_| {
                                overlays.detail.set(None);
                                data.with_mut(|d| {
                                    d.delete(id);
                                    d.save();
                                });
                            },
                            "SURE?"
                        }
                    } else {
                        button {
                            class: "btn-quiet",
                            title: "Delete habit",
                            onclick: move |_| overlays.confirm.set(true),
                            "DELETE HABIT"
                        }
                    }
                }
            }
        }
    }
}

pub fn add_sheet(mut data: Signal<Data>, mut overlays: Overlays) -> Element {
    if !(overlays.adding)() {
        return rsx! {};
    }

    let mut add = move || {
        let name = (overlays.name_draft)();
        if name.trim().is_empty() {
            return;
        }
        data.with_mut(|d| {
            d.add(&name, &(overlays.note_draft)());
            d.save();
        });
        overlays.adding.set(false);
    };

    rsx! {
        div { class: "overlay", onclick: move |_| overlays.adding.set(false),
            div { class: "sheet", onclick: move |e| e.stop_propagation(),
                div { class: "sheet-label", "New habit" }
                div { class: "form",
                    input {
                        class: "input",
                        value: "{overlays.name_draft}",
                        placeholder: "Name it…",
                        enterkeyhint: "done",
                        onmounted: move |e| async move {
                            let _ = e.data().set_focus(true).await;
                        },
                        oninput: move |e| overlays.name_draft.set(e.value()),
                        onkeydown: move |e| {
                            if e.key() == Key::Enter {
                                add();
                            } else if e.key() == Key::Escape {
                                overlays.adding.set(false);
                            }
                        },
                    }
                    input {
                        class: "input",
                        value: "{overlays.note_draft}",
                        placeholder: "Note (optional) — e.g. 06:30 · 5 KM",
                        enterkeyhint: "done",
                        oninput: move |e| overlays.note_draft.set(e.value()),
                        onkeydown: move |e| {
                            if e.key() == Key::Enter {
                                add();
                            } else if e.key() == Key::Escape {
                                overlays.adding.set(false);
                            }
                        },
                    }
                    button {
                        class: "btn",
                        disabled: (overlays.name_draft)().trim().is_empty(),
                        onclick: move |_| add(),
                        "ADD"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_cells_pad_to_weekday_columns() {
        // July 2026 starts on a Wednesday: two leading blanks, then 31 days.
        let cells = month_cells(NaiveDate::from_ymd_opt(2026, 7, 15).unwrap());
        assert_eq!(cells.len(), 2 + 31);
        assert!(cells[..2].iter().all(Option::is_none));
        assert_eq!(cells[2], NaiveDate::from_ymd_opt(2026, 7, 1));
        assert_eq!(cells[32], NaiveDate::from_ymd_opt(2026, 7, 31));
    }
}
```

Note: the uppercase in "HABIT", "SAVE", "SURE?" etc. above is fine — `sheet-label`, `btn`, `btn-quiet` all apply `text-transform: uppercase` anyway; keep the literals as written in the code block.

- [ ] **Step 2: Verify**

Run: `cargo test -p habits && cargo clippy -p habits --all-targets -- -D warnings && cargo fmt`
Then `dx serve`: + opens the add sheet (name + note, Enter adds, Esc closes, empty name disabled). Tapping a row opens the detail sheet: calendar shows done days as red squares, today outlined; tapping a day within the last 7 toggles it and the ledger/sidebar update; older days don't respond; future days are dimmed and disabled; ‹ › month nav stops at the current month; tapping the name opens the edit form (both fields prefilled; Save applies both); DELETE HABIT arms to SURE? and then deletes; clicking the backdrop closes.

- [ ] **Step 3: Commit (via the human)**

```bash
git add crates/habits/src/ui/sheet.rs
git commit -m "feat(habits): detail and add sheets with binary calendar"
```

---

### Task 9: PWA branding + README

**Files:**
- Modify: `crates/habits/web/manifest.json`
- Modify: `crates/habits/web/icon-192.png`, `crates/habits/web/icon-512.png` (regenerated)
- Create (scratch only, not committed): `icon.svg` in the scratchpad
- Modify: `crates/habits/README.md` (full rewrite)

**Interfaces:**
- Consumes: nothing from other tasks (parallel-safe after Task 4).
- Produces: user-facing branding only; no code interfaces.

- [ ] **Step 1: Replace `web/manifest.json`**

```json
{
  "name": "TALLY",
  "short_name": "TALLY",
  "description": "Tick your habits off, day by day.",
  "start_url": "./",
  "scope": "./",
  "display": "standalone",
  "background_color": "#f3f2f2",
  "theme_color": "#f3f2f2",
  "icons": [
    { "src": "icon-192.png", "sizes": "192x192", "type": "image/png" },
    { "src": "icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any maskable" }
  ]
}
```

- [ ] **Step 2: Regenerate the icons**

Write this SVG to the scratchpad as `icon.svg` (a geometric Archivo-like T, white on the accent red, glyph kept inside the maskable safe zone):

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512">
  <rect width="512" height="512" fill="#ec3013"/>
  <rect x="146" y="166" width="220" height="52" fill="#f3f2f2"/>
  <rect x="230" y="166" width="52" height="180" fill="#f3f2f2"/>
</svg>
```

Render both sizes (resvg via nix):

```bash
nix run nixpkgs#resvg -- --width 512 --height 512 icon.svg crates/habits/web/icon-512.png
nix run nixpkgs#resvg -- --width 192 --height 192 icon.svg crates/habits/web/icon-192.png
```

(Run from the repo root; adjust the icon.svg path to wherever you wrote it.) Verify: `file crates/habits/web/icon-*.png` reports the right dimensions, and opening them shows a red square with a white T.

- [ ] **Step 3: Rewrite `README.md`**

```markdown
# habits (TALLY)

Habit-ledger PWA built with Dioxus (web/wasm). Fully client-side: habits
and their done-days live in the browser's localStorage, so the app works
offline — no server, no account, no sync.

The Today screen is a ledger: one strong list, a checkbox per habit, the
current streak as a numeral. On a phone it's a single column with a bottom
tab bar; from 900px up it grows a rail nav, a 14-day dot strip per habit,
and a summary sidebar (completion, the week's bars, best streak). Design:
"TALLY", Modernist system — Archivo, red on light ground, zero radius —
from a Claude Design exploration (see
docs/superpowers/specs/2026-07-30-habits-tally-redesign-design.md).

Days are binary: done or not. The checkbox toggles today; tapping the rest
of the row opens a detail sheet with a month calendar (the last 7 days can
be corrected there — forgot to log, logged by mistake…; older days are
view-only), name/note editing, and delete behind a two-tap confirm.

Storage is schema v2 (`habits/v2`). Data recorded by the v1 app
(timestamped ticks) is migrated automatically on first load; the old key
is left in place as a backup.

## Develop

```sh
dx serve            # from crates/habits; hot-reloading dev server
```

`dx` is in the devenv shell. Unit tests for the date math run natively:
`cargo test -p habits`.

## Ship

Offline launch needs a service worker, and browsers only register those on
HTTPS origins (localhost is exempt, LAN IPs are not). So: build, drop the
`web/` files into the output root, host the result on any static HTTPS host
(GitHub Pages, Netlify, a homelab behind a real cert, ...):

```sh
dx build --release
cp web/* target/dx/habits/release/web/public/
```

Then open the URL on the phone once, and "Add to Home Screen". From then on
it launches and works with no connectivity. Archivo is vendored under
`assets/fonts/`, so no network is needed even for type.

Caveats of being fully client-side: data is per-device (no sync), and
clearing the browser's site data deletes it.
```

- [ ] **Step 4: Verify + Commit (via the human)**

Run: `cargo test -p habits` (unchanged, still green).

```bash
git add crates/habits/web crates/habits/README.md
git commit -m "feat(habits): TALLY branding for manifest, icons, and README"
```

---

### Task 10: Final verification

**Files:** none modified (fixes only if checks fail).

- [ ] **Step 1: Full native check**

Run from the repo root:

```bash
cargo fmt --check && cargo clippy -p habits --all-targets -- -D warnings && cargo test -p habits
```

Expected: all clean/green.

- [ ] **Step 2: Wasm build**

From `crates/habits`: `dx build` — must succeed (catches wasm-only breakage the native tests can't).

- [ ] **Step 3: Visual verification against the design**

From `crates/habits`, `dx serve` (background), then with the playwright-cli skill (or by hand) screenshot at 390×844 and 1280×800 and compare against options 1a and 2a in the design doc:

- Mobile: accent date row, Today display heading, done/total + meter, rows (checkbox / name+note / streak numeral), bottom bar with TODAY accent + black + square. No dot strip, no sidebar.
- Desktop: rail (wordmark, inset-bar TODAY, + NEW HABIT bottom block), column headers, 14-day dot strips with today outlined, sidebar (COMPLETION / THIS WEEK / BEST STREAK).
- Exercise: add a habit with a note, tick it, open its sheet, backfill a day, rename, delete. Reload — state persists. Seed a v1 payload (`localStorage.setItem('habits/v1', ...)` with the old shape, clear `habits/v2`, reload) and confirm the migration shows the habits with their days.

- [ ] **Step 4: Present screenshots to the human for final visual sign-off.**

No commit — this task only verifies.
