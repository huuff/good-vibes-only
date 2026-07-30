# TALLY — habits redesign

Redesign of the `habits` PWA to the "Ledger, responsive" direction from the
Claude Design project *Habit Tracker Options* (options 1a mobile + 2a desktop),
in the Modernist design system. Decisions confirmed with the user on
2026-07-30:

- Direction: **Ledger, responsive** (1a below ~900px, 2a's rail + sidebar above).
- Nav: **tabs shown, only Today live** — HABITS / STATS / SETTINGS render muted
  and inert.
- Ticks: **fully binary** — a day is done or not; no counts, no ± stepper.
- The 66-day **strength model is dropped**; streak numerals replace it.
- Each habit gains an **optional free-text note** (the design's meta line,
  e.g. "06:30 · 5 KM").
- **Full TALLY branding**: UI wordmark, manifest name, theme colors, new icons.
  The crate stays `habits`.

## Data model (`store.rs`, storage v2)

```rust
pub struct Habit {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub note: String,              // optional meta line; "" = none
    pub days: BTreeSet<NaiveDate>, // days on which the habit was done
}
```

- Storage key `habits/v2`. On load: try v2; if absent, read v1
  (`Vec<DateTime<Utc>>` ticks) and migrate — each habit's timestamps collapse
  to the set of their *local* dates, `note` starts empty — then save as v2.
  The v1 key is left untouched as a free backup. Corrupt/missing data still
  falls back to an empty default.
- Mutations: `add(name, note)`, `toggle(id, day)` (no-op outside the edit
  window), `rename(id, name)`, `set_note(id, note)`, `delete(id)`. The
  existing 7-day edit window (`editable`, `EDIT_WINDOW_DAYS`) is kept; the
  ledger checkbox toggles today, the calendar toggles any editable day, older
  days are view-only.
- Derived, per habit:
  - `done_on(day)`, `done_today()`.
  - `streak()` — consecutive done-days ending today, or yesterday if today is
    not yet done (unchanged semantics).
  - `best_streak()` — longest run ever (sidebar).
  - `history(n)` — the last `n` days, oldest first, as done-flags (14-day dot
    strip).
- Derived, whole collection:
  - done-today / total counts → the `4/6`, header progress bar, completion %,
    and "N LEFT BEFORE MIDNIGHT".
  - per-day completion fraction over the last 7 days → sidebar week bars
    (computed over the habits that currently exist).
  - best streak across habits, with the habit's name.
  - "DAY N": days since the earliest recorded day across all habits, 1-based.
    Hidden until at least one day is recorded.
- Deleted outright: `strength()`, `FORMATION_DAYS`, tick counts, and their
  tests.

## UI (`ui/` module)

`ui.rs` splits into `ui/{mod,nav,ledger,sidebar,sheet}.rs`:

- `mod.rs` — `app()`: the single `Data` signal (saved after every mutation),
  sheet-state signals, and the layout shell. One DOM for both form factors;
  a CSS media query at 900px decides what shows.
- `nav.rs` — mobile bottom bar (TODAY · HABITS · STATS · black `+` square) and
  desktop left rail (TALLY. wordmark, stacked tabs incl. SETTINGS, `+ NEW
  HABIT` block pinned to the bottom). Only TODAY is live (accent + inset bar
  per the design); other tabs are muted, `aria-disabled`, and do nothing.
- `ledger.rs` — the header (uppercase date in accent red, DAY N right-aligned
  muted, `Today` display heading, done/total numeral + progress bar) and the
  habit rows: 28px square checkbox (accent-filled with check when done,
  2px-outlined when not), name + uppercase note line, 14-day dot strip and
  right-aligned streak numeral (muted column headers HABIT / LAST 14
  DAYS / STREAK on desktop; strip and headers hidden on mobile per 1a). The
  streak numeral is full-strength when done today, dimmed when not.
  Checkbox click toggles today; clicking anywhere else on the row opens the
  detail sheet. Empty state: one muted line inviting the first habit.
- `sidebar.rs` — desktop-only right column: COMPLETION % block, THIS WEEK
  bar chart (7 bars, today outlined instead of filled), BEST STREAK numeral +
  habit name. Hidden below 900px.
- `sheet.rs` — the detail sheet and add sheet, restyled Modernist (square
  corners, 2px rules, Archivo). Detail sheet keeps: month calendar (binary —
  done days fill accent; tap toggles within the 7-day window, older days
  view-only), tap-to-rename, note editing, delete behind two-tap confirm.
  The ± stepper is gone. Add sheet: name input + optional note input.

## Visuals and assets

- `assets/style.css` rewritten from the Modernist tokens — only the rules the
  app uses. Tokens: bg `#f3f2f2`, surface `#eae9e9`, text `#201e1d`, accent
  `#ec3013`, divider `color-mix(in srgb, #201e1d 40%, transparent)`; radius 0
  everywhere; 2px rules for major structure, 1px for rows; Archivo with
  weights 400/600/800/900; uppercase letterspaced labels. Light-only, like
  the design system.
- Archivo is vendored: woff2 files (weights 400, 600, 800, 900) under
  `assets/fonts/`, declared with `@font-face` + `font-display: swap`,
  `system-ui` fallback. No runtime Google Fonts request — offline must keep
  working. Verify during implementation that the service worker caches the
  font assets like the rest of the build output.
- PWA branding: `manifest.json` name/short-name TALLY, light
  `background_color`/`theme_color` (`#f3f2f2`), regenerated `icon-192.png` /
  `icon-512.png` — flat `#ec3013` square, white Archivo "T", zero radius.
  `index.html` title TALLY.
- README rewritten to match (binary model, ledger UI, TALLY name; strength
  paragraph removed).

## Error handling

Unchanged philosophy: `save()` ignores localStorage errors; malformed stored
data degrades to the empty default. Migration only runs when v2 is absent, so
a failed first save just means it re-migrates next load.

## Testing

- Native unit tests (`cargo test -p habits`) for: v1→v2 migration, toggle and
  edit-window enforcement, streak / best-streak / history, collection
  aggregates (done counts, week fractions, best streak, DAY N), and the
  existing `month_cells` test.
- UI verified manually via `dx serve` at both form factors.
- No new test infrastructure.

## Out of scope

- HABITS / STATS / SETTINGS pages (tabs are inert placeholders until designed).
- Dark mode, sync, reminders, habit scheduling/frequency.
- Any change to how the app is built or shipped (`dx build` + copy `web/`).
