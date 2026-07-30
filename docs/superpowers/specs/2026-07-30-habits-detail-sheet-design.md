# Habits: card-opens-detail-sheet redesign (rename + delete in sheet)

Date: 2026-07-30
Crate: `crates/habits`

## Goal

Make habits editable (renameable), and simplify each card's interaction
model while doing so. Today a card is three competing tap targets: the
whole card records a tick, ▦ opens the calendar, ✕ (then "sure?")
deletes. Stray taps log accidental ticks, and there is no way to rename.

New model:

- **Tapping the card opens a detail sheet** (the existing calendar sheet,
  extended) where the habit can be inspected, renamed, and deleted.
- **One explicit record button on the card** is the only way to log a
  tick.

## Card

- Tapping anywhere on the card opens the detail sheet for that habit.
- The right side of the card carries a single large round **record
  button** that merges today's count display with the record action:
  dimmed "—" when untouched today, the count once tapped. Tapping it
  records a tick (and must not also open the sheet — stop propagation).
- Week dots, streak flame, total count, and the strength line along the
  bottom edge are unchanged.
- The actions row (▦, ✕, "sure?") is removed entirely, along with the
  per-card `confirm_delete` state driving it.

## Detail sheet

The existing `calendar_sheet` grows two abilities:

- **Rename**: the habit name in the sheet header is tappable. Tapping
  swaps it for an input prefilled with the current name, with a subtle ✎
  hint next to the static title so the affordance is discoverable.
  Enter or a Save button saves (trimmed; empty input saves nothing),
  Escape cancels. Saving calls the new store method and re-renders the header.
- **Calendar**: unchanged — month navigation, shaded day cells, − / +
  stepper for today and the previous 7 days, older days view-only.
- **Delete**: a "Delete habit" button at the bottom of the sheet, using
  the existing two-tap confirm pattern (first tap arms it into a red
  "sure?", second tap deletes). Confirming deletes the habit, closes the
  sheet. The armed state resets whenever the sheet opens.

## Store

One new method on `Data`:

```rust
pub fn rename(&mut self, id: u64, name: &str)
```

Trims the name; a resulting empty string is ignored; unknown ids are
no-ops. `add`, `record`, `record_on`, `unrecord_on`, `delete` are
unchanged. No data migration: `Habit` keeps the same shape and the
localStorage key stays `habits/v1`.

## Testing

- Unit tests for `rename` next to the existing store tests: renames by
  id, trims whitespace, rejects empty/whitespace-only, no-op on unknown
  id.
- UI changes verified interactively with `dx serve` (wasm UI has no unit
  test harness in this crate; existing pattern).

## Out of scope

- Any new editable habit fields (colour, schedule, notes, …).
- Data model or storage changes.
- Reordering habits.
