# habits — deferred issues (TALLY redesign, 2026-07-30)

Known-but-accepted items from the TALLY redesign. Each was reviewed and
triaged as OK-to-defer; none block the release. Salvaged from the
subagent run ledger before it was untracked.

Plan: `docs/superpowers/plans/2026-07-30-habits-tally-redesign.md`
Spec: `docs/superpowers/specs/2026-07-30-habits-tally-redesign-design.md`

## Time and clock

- **Store tests depend on the wall clock.** Nearly every test in
  `crates/habits/src/store.rs` builds dates from `Local::now()` (and the
  API itself does — `done_today`, `streak`, `history`). Nothing injects a
  clock, so tests can behave differently across a midnight boundary or in
  another timezone. Fixing it means threading a clock/`today` parameter
  through the store API.
- **Header date goes stale at midnight.** `ledger.rs:13` formats
  `Local::now()` at render time, so an app left open across midnight keeps
  showing yesterday's date until something triggers a re-render. Display
  only — the underlying data bug is fixed: the checkbox at `ledger.rs:63`
  computes the date at click time.
- **v1→v2 migration timezone edge is untested.** The migration
  reinterprets UTC tick timestamps as local days; without clock injection
  there is no way to test the boundary case.

## Storage

- **A corrupt (not merely absent) v2 key re-runs the v1 migration.**
  `Data::load()` (`store.rs:129`) treats any `LocalStorage::get(KEY)`
  error as "no v2 data yet" and falls through to the v1 path. Graceful
  either way — worst case the user gets their v1 state back — but it
  silently discards unreadable v2 data.
- **`load()`'s migration path has no end-to-end test.** `gloo`'s
  `LocalStorage` needs a wasm runtime; coverage is via `from_v1` plus
  JSON-shape tests instead. The full path was verified manually in-browser
  (collapse, `next_id`, v1 key retained).
- **Multi-tab writes are last-write-wins.** Pre-existing; no cross-tab
  storage-event sync.

## UI and accessibility

- **Locked calendar days stay focusable and keep a live no-op `onclick`.**
  In `sheet.rs`, days outside the 7-day edit window get the `locked` class
  and an `onclick` guarded by `editable(day)`, but are not `disabled` —
  only future days are. Keyboard users can tab to a day that does nothing.
- **No focus trap in the sheets.** Both sheets have `role="dialog"`,
  `aria-modal`, self-focus on mount, and Escape-to-close. But after
  Escape exits edit mode, focus falls to `body`, so a second Escape needs
  a refocus before it closes the sheet. Cosmetic focus-management gap; a
  full focus trap was classed as nice-to-have.
- **Weekday abbreviations assume an ASCII locale.**
  `sidebar.rs:51` does `day.format("%a").to_string()[..2]`, a byte slice.
  Safe under chrono's default English locale; would panic on a
  multi-byte weekday name if the locale ever changes.
- **`Data` is cloned on every render.** Idiomatic-Dioxus concern, not a
  measured problem at this data size.

## Assets and build

- **PWA icons were hand-encoded** with stdlib Python rather than rendered
  with `resvg` (nix was unavailable in the sandbox). Pixel-verified
  correct; optionally re-render outside the sandbox for a clean
  provenance trail.
- **First-visit offline is unsupported.** The service worker caches on
  first load, so a cold first visit while offline fails. Pre-existing and
  out of the redesign's scope.
