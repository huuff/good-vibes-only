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

## Android APK

The devenv shell carries the whole toolchain (SDK 34, NDK, Java, an
Android-target rustc). From `crates/habits`:

```sh
dx build --platform android --release --target aarch64-linux-android
find ../../target/dx/habits -name '*.apk'
```

The explicit `--target` matters: without it dx assumes an emulator
(x86_64) and tries to rustup-install that target, which the nix
toolchain can't do. The APK lands under
`target/dx/habits/release/android/.../outputs/apk/debug/app-debug.apk` —
Gradle's *debug variant*, but the Rust inside is the release build, and
it's debug-signed, which is exactly what sideloading wants.

Install: `adb install <apk>` (USB debugging), or copy the file to the
phone and open it (allow "install unknown apps"). Storage on Android is
JSON files in the app's private data dir (see `src/persist.rs`) — data
is per-device, deleted with the app, and separate from any web/PWA
instance.
