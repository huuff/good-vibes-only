# Habits Detail-Sheet Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tapping a habit card opens a detail sheet (calendar + rename + delete); a single round button on the card is the only way to record a tick.

**Architecture:** All UI lives in `crates/habits/src/ui.rs` (Dioxus rsx, signals created in `app` and passed to a plain helper function that renders the sheet — hooks are never created inside the helper, so hook order stays stable). Persistence lives in `crates/habits/src/store.rs`; every mutation goes through `Data` methods followed by `d.save()`. Styling is `crates/habits/assets/style.css`.

**Tech Stack:** Rust, Dioxus 0.7 (web/wasm), chrono, gloo-storage, plain CSS. Spec: `docs/superpowers/specs/2026-07-30-habits-detail-sheet-design.md`.

## Global Constraints

- **No git access from this sandbox.** `git` cannot see the repository here (worktree gitdir is permission-blocked). Skip every "Commit" step; leave the working tree dirty for the user to commit.
- Store tests run natively: `cargo test -p habits` from the repo root.
- Lint gate: `cargo clippy -p habits` must be warning-free; `cargo fmt` must leave no diff (pre-commit enforces both).
- The wasm UI has no unit-test harness in this crate (existing pattern); UI tasks are verified by `cargo test`/`clippy` compiling the code plus a final interactive smoke check.
- localStorage key stays `habits/v1`; `Habit` keeps the shape `{id, name, ticks}` — no migration.
- UI copy style is lowercase-terse (e.g. "sure?", "view only"); follow it for new strings.

---

### Task 1: `Data::rename` in the store

**Files:**
- Modify: `crates/habits/src/store.rs` (method next to `delete` around line 197; test at the end of `mod tests`)

**Interfaces:**
- Consumes: existing `Data { habits: Vec<Habit>, .. }` and `Habit { id, name, .. }`.
- Produces: `pub fn rename(&mut self, id: u64, name: &str)` on `Data` — trims `name`; empty result or unknown `id` is a silent no-op. Task 4 calls it as `d.rename(id, &draft())`.

- [ ] **Step 1: Write the failing test**

Add at the end of `mod tests` in `crates/habits/src/store.rs`:

```rust
    #[test]
    fn rename_trims_and_ignores_empty_or_unknown() {
        let mut data = Data::default();
        data.add("Stretch");
        let id = data.habits[0].id;

        data.rename(id, "  Morning stretch  ");
        assert_eq!(data.habits[0].name, "Morning stretch");

        // Whitespace-only: rejected, name untouched.
        data.rename(id, "   ");
        assert_eq!(data.habits[0].name, "Morning stretch");

        // Unknown id: no-op, no panic.
        data.rename(id + 1, "Other");
        assert_eq!(data.habits[0].name, "Morning stretch");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run from the repo root: `cargo test -p habits rename_trims`
Expected: compile error — `no method named `rename` found for struct `Data``.

- [ ] **Step 3: Write minimal implementation**

Add to `impl Data` in `crates/habits/src/store.rs`, right before `pub fn delete`:

```rust
    pub fn rename(&mut self, id: u64, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(habit) = self.habits.iter_mut().find(|h| h.id == id) {
            habit.name = name.to_string();
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p habits`
Expected: all tests PASS, including `rename_trims_and_ignores_empty_or_unknown`.

- [ ] **Step 5: Commit**

Skipped — no git access (see Global Constraints).

---

### Task 2: Delete moves into the sheet

The sheet gains a "Delete habit" button with the existing two-tap confirm pattern. The card's delete button still exists after this task (removed in Task 3) — temporary duplication is expected.

**Files:**
- Modify: `crates/habits/src/ui.rs` (function `calendar_sheet`, ~line 48; `app`, ~line 186)
- Modify: `crates/habits/assets/style.css` (new rule after the `.cal-lock` block)

**Interfaces:**
- Consumes: `Data::delete(id)` (existing).
- Produces: `detail_sheet(data, open, month, sel, confirm)` — `calendar_sheet` renamed, with new param `confirm: Signal<bool>` (armed delete-confirm state). `app` owns the signal as `cal_confirm` and resets it to `false` whenever the sheet opens. Tasks 3–4 build on this signature.

- [ ] **Step 1: Rename `calendar_sheet` → `detail_sheet` and add the `confirm` param**

In `crates/habits/src/ui.rs`, change the signature (and its doc comment first line to `/// Detail sheet for one habit: month calendar plus rename/delete.`):

```rust
fn detail_sheet(
    mut data: Signal<Data>,
    mut open: Signal<Option<u64>>,
    mut month: Signal<NaiveDate>,
    mut sel: Signal<Option<NaiveDate>>,
    mut confirm: Signal<bool>,
) -> Element {
```

- [ ] **Step 2: Add the delete row at the bottom of the sheet**

In `detail_sheet`'s final `rsx!`, insert after `{editor}`:

```rust
                div { class: "sheet-del",
                    if confirm() {
                        button {
                            class: "mini danger",
                            title: "Really delete",
                            onclick: move |_| {
                                open.set(None);
                                data.with_mut(|d| {
                                    d.delete(id);
                                    d.save();
                                });
                            },
                            "sure?"
                        }
                    } else {
                        button {
                            class: "mini",
                            title: "Delete habit",
                            onclick: move |_| confirm.set(true),
                            "Delete habit"
                        }
                    }
                }
```

- [ ] **Step 3: Create and wire the signal in `app`**

In `app`, after `let mut cal_day = ...`:

```rust
    let mut cal_confirm = use_signal(|| false);
```

In the ▦ button's `onclick` (the one that sets `calendar.set(Some(habit.id))`), add `cal_confirm.set(false);` before `calendar.set(...)`. Update the call site at the bottom of `app`'s rsx:

```rust
            {detail_sheet(data, calendar, cal_month, cal_day, cal_confirm)}
```

- [ ] **Step 4: Style the delete row**

In `crates/habits/assets/style.css`, after the `.cal-lock` rule:

```css
.sheet-del {
  display: flex;
  justify-content: center;
  margin-top: 1.1rem;
}
```

- [ ] **Step 5: Verify it compiles clean and tests pass**

Run: `cargo test -p habits && cargo clippy -p habits`
Expected: tests PASS, clippy warning-free.

- [ ] **Step 6: Commit**

Skipped — no git access (see Global Constraints).

---

### Task 3: Card rework — tap opens sheet, one record button

Tapping the card opens the detail sheet; a round tick button (which absorbs the today-count display) is the only way to record. The actions row (▦ / ✕ / "sure?") and the `confirm_delete` signal disappear.

**Files:**
- Modify: `crates/habits/src/ui.rs` (delete `today_count` helper ~line 14; rework the card in `app`)
- Modify: `crates/habits/assets/style.css` (replace `.count` and `.actions` rules with `.tick`)

**Interfaces:**
- Consumes: `detail_sheet(..., confirm)` from Task 2; `Data::record(id)` (existing).
- Produces: card markup with a `button.tick`; `app` no longer has a `confirm_delete` signal. Task 4 adds one more reset line to the card `onclick`.

- [ ] **Step 1: Remove the `today_count` helper**

Delete the `fn today_count(count: usize) -> Element { ... }` block (and its doc comment) near the top of `crates/habits/src/ui.rs`. Nothing else uses it after this task.

- [ ] **Step 2: Rework the card, drop the actions row**

In `app`: delete the line `let mut confirm_delete = use_signal(|| None::<u64>);`. Replace the whole `li { ... }` body (card div + actions div) with:

```rust
                    li { key: "{habit.id}",
                        div {
                            class: "card",
                            role: "button",
                            title: format!(
                                "habit strength {:.0} of ~{FORMATION_DAYS} days — grows each practiced day, fades a little over long breaks",
                                habit.strength(),
                            ),
                            onclick: move |_| {
                                let today = Local::now().date_naive();
                                cal_month.set(today);
                                cal_day.set(Some(today));
                                cal_confirm.set(false);
                                calendar.set(Some(habit.id));
                            },
                            div { class: "card-top",
                                span { class: "name", "{habit.name}" }
                                button {
                                    class: if habit.today_count() == 0 { "tick dim" } else { "tick" },
                                    title: "Done it — record one",
                                    onclick: move |e| {
                                        e.stop_propagation();
                                        data.with_mut(|d| {
                                            d.record(habit.id);
                                            d.save();
                                        });
                                    },
                                    if habit.today_count() == 0 {
                                        "—"
                                    } else {
                                        "{habit.today_count()}"
                                    }
                                }
                            }
                            div { class: "card-bottom",
                                div { class: "week",
                                    for (day , done) in habit.week() {
                                        span {
                                            class: if done { "dot done" } else { "dot" },
                                            title: "{day}",
                                        }
                                    }
                                }
                                if habit.streak() > 1 {
                                    span { class: "streak", "🔥 {habit.streak()}d" }
                                }
                                span { class: "total", "{habit.ticks.len()} total" }
                            }
                            div {
                                class: if habit.strength() >= FORMATION_DAYS as f64 { "root rooted" } else { "root" },
                                style: format!(
                                    "width:{:.1}%",
                                    habit.strength() * 100.0 / FORMATION_DAYS as f64,
                                ),
                            }
                        }
                    }
```

(This drops the old whole-card `record` onclick, the `div.actions` block with ▦/✕/"sure?", and the `confirm_delete` resets. The ▦ button's open-sheet logic now lives in the card `onclick`.)

- [ ] **Step 3: Swap the CSS**

In `crates/habits/assets/style.css`:

Delete the `/* --- today's count --- */` section (`.count`, `.count.dim`) and, from the `/* --- per-habit actions --- */` section, the `.actions` rule only — `.mini`, `.mini:active`, and `.mini.danger` stay (the sheet uses them). Add in place of the `.count` rules:

```css
/* --- record button --------------------------------------------------
   The one tap that logs a tick; also shows today's count. */

.tick {
  flex: none;
  width: 2.9rem;
  height: 2.9rem;
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--amber);
  background: rgba(0, 0, 0, 0.25);
  border: 1px solid var(--card-edge);
  border-radius: 50%;
  cursor: pointer;
  transition: transform 0.08s ease;
}

.tick:active {
  transform: scale(0.9);
}

.tick.dim {
  color: var(--chalk-dim);
  font-weight: 400;
}
```

- [ ] **Step 4: Verify it compiles clean and tests pass**

Run: `cargo test -p habits && cargo clippy -p habits`
Expected: tests PASS, clippy warning-free (in particular: no unused `today_count` / unused-variable warnings).

- [ ] **Step 5: Commit**

Skipped — no git access (see Global Constraints).

---

### Task 4: Rename in the sheet header

The sheet's title becomes a tappable button with a ✎ hint; tapping swaps it for a prefilled input (Enter/Save saves, Escape cancels).

**Files:**
- Modify: `crates/habits/src/ui.rs` (`detail_sheet` signature + header; `app` signals and card onclick)
- Modify: `crates/habits/assets/style.css` (new `.sheet-name` rule)

**Interfaces:**
- Consumes: `Data::rename(id, &str)` from Task 1; `detail_sheet` from Task 2.
- Produces: final signature `detail_sheet(data, open, month, sel, confirm, renaming, draft)` with `renaming: Signal<bool>`, `draft: Signal<String>` (7 params — at clippy's `too_many_arguments` threshold, not over it).

- [ ] **Step 1: Extend `detail_sheet`**

New signature:

```rust
fn detail_sheet(
    mut data: Signal<Data>,
    mut open: Signal<Option<u64>>,
    mut month: Signal<NaiveDate>,
    mut sel: Signal<Option<NaiveDate>>,
    mut confirm: Signal<bool>,
    mut renaming: Signal<bool>,
    mut draft: Signal<String>,
) -> Element {
```

After the `let Some(habit) = ... else` line, add the save closure and a captured name (needed because `habit` itself can't move into the handler while the rsx also reads it):

```rust
    let mut save = move || {
        data.with_mut(|d| {
            d.rename(id, &draft());
            d.save();
        });
        renaming.set(false);
    };
    let name = habit.name.clone();
```

Replace the sheet's `h2 { "{habit.name}" }` with:

```rust
                if renaming() {
                    div { class: "add",
                        input {
                            value: "{draft}",
                            enterkeyhint: "done",
                            onmounted: move |e| async move {
                                let _ = e.data().set_focus(true).await;
                            },
                            oninput: move |e| draft.set(e.value()),
                            onkeydown: move |e| {
                                if e.key() == Key::Enter {
                                    save();
                                } else if e.key() == Key::Escape {
                                    renaming.set(false);
                                }
                            },
                        }
                        button {
                            class: "add-btn",
                            disabled: draft().trim().is_empty(),
                            onclick: move |_| save(),
                            "Save"
                        }
                    }
                } else {
                    button {
                        class: "sheet-name",
                        title: "Rename habit",
                        onclick: move |_| {
                            draft.set(name.clone());
                            renaming.set(true);
                        },
                        "{habit.name} ✎"
                    }
                }
```

- [ ] **Step 2: Wire the signals in `app`**

After `let mut cal_confirm = use_signal(|| false);`:

```rust
    let mut cal_renaming = use_signal(|| false);
    let cal_draft = use_signal(String::new);
```

In the card `onclick` (Task 3), add `cal_renaming.set(false);` next to `cal_confirm.set(false);`. Update the call site:

```rust
            {detail_sheet(data, calendar, cal_month, cal_day, cal_confirm, cal_renaming, cal_draft)}
```

- [ ] **Step 3: Style the tappable title**

In `crates/habits/assets/style.css`, right after the `.sheet h2` rule (the title mirrors it, as a button):

```css
/* The detail sheet's title doubles as the rename affordance. */
.sheet-name {
  display: block;
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 0.72rem;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--chalk-dim);
  background: transparent;
  border: none;
  padding: 0;
  margin-bottom: 0.75rem;
  cursor: pointer;
  text-align: left;
}
```

- [ ] **Step 4: Verify it compiles clean and tests pass**

Run: `cargo test -p habits && cargo clippy -p habits`
Expected: tests PASS, clippy warning-free.

- [ ] **Step 5: Commit**

Skipped — no git access (see Global Constraints).

---

### Task 5: Final verification

**Files:** none new — whole-crate gate.

- [ ] **Step 1: Format, lint, test**

Run from the repo root:

```bash
cargo fmt && cargo clippy -p habits && cargo test -p habits
```

Expected: no fmt diff, no warnings, all tests PASS.

- [ ] **Step 2: Interactive smoke check**

Serve the app (`dx serve` from `crates/habits`; `dx` is in the devenv shell) and verify in a browser:

1. Tapping a card opens the detail sheet (no tick recorded).
2. The round button on the card records a tick and shows the count; "—" when untouched today.
3. In the sheet: title tap → input prefilled → Enter renames (trimmed); Escape cancels; whitespace-only Save is disabled.
4. Calendar month nav and − / + stepper still work; days older than 7 days are view-only.
5. "Delete habit" → "sure?" → habit gone, sheet closed. Reopening another habit's sheet starts un-armed and not renaming.
6. Reload the page: renames and ticks persisted.

If a browser can't be driven from this environment, ask the user to run the smoke check.

- [ ] **Step 3: Hand off for commit**

Report results; the user commits (suggested message: `feat(habits): open detail sheet from card; rename, delete and record button rework`).
