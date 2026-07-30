# habits: Android APK build — design

Date: 2026-07-30
Status: approved

## Goal

`dx build --android --release` (run from the devenv shell, from
`crates/habits`) produces an APK that can be sideloaded onto a phone.
The app starts with fresh data on the phone — no migration from any
browser/PWA instance. The web build's behavior is unchanged.

## Context

The crate is currently web-only: `dioxus = { features = ["web"] }` and
persistence through `gloo-storage` (browser localStorage). On Android,
Dioxus compiles the Rust natively and renders through a system WebView,
so localStorage is not reachable from the Rust side. UI code is already
platform-clean: `dioxus::launch`, `asset!`, and `document::Stylesheet`
all work on mobile, and fonts are vendored. Only persistence and the
manifest are web-bound.

## Design

### 1. Manifest & platform features

- `Cargo.toml`:
  - `dioxus = "0.7"` with no platform feature.
  - `[features]`: `default = ["web"]`, `web = ["dioxus/web"]`,
    `mobile = ["dioxus/mobile"]`. `dx` selects the feature matching the
    target platform; plain `cargo build` / `cargo test` and the flake's
    workspace build keep using `web` via the default.
  - `gloo-storage` becomes a
    `[target.'cfg(target_family = "wasm")'.dependencies]` entry.
- New `Dioxus.toml` with the app name (TALLY) and Android bundle
  identifier `com.huuff.habits`.

### 2. Storage split (`persist` module)

A new module owning "get JSON value by key / set JSON value by key",
with two compile-time backends:

- **wasm32**: today's gloo/localStorage code, moved verbatim.
- **native**: each key serialized as JSON to a file in the app's data
  directory. On Android the directory comes from the app context
  (`android_activity` / `ndk-context`; exact API pinned during
  implementation). Non-Android native (dev runs, tests) falls back to a
  local directory.

`Data::load()` / `Data::save()` call through this module and stay
synchronous. Domain logic and the v1→v2 migration are untouched (on
Android the v1 key simply never exists). Existing tests unchanged; add
a native round-trip test for the file backend.

### 3. Toolchain (devenv)

- `android.enable = true` in `devenv.nix` (devenv's Android
  integration: SDK, NDK, JDK, env vars). Emulator disabled — the target
  is a physical phone.
- `languages.rust` switches to a channel that supports extra targets
  and adds `aarch64-linux-android` (only; no 32-bit/x86 targets).
- `nix flake check --no-build` must still pass.

### 4. Build, sign, install

- `dx build --android --release` → APK under `target/dx/habits/…`.
- Signing: if the release APK comes out unsigned, wire a local,
  gitignored keystore into the generated Gradle config; fallback is the
  debug-signed APK, which sideloads fine.
- Install via `adb install` or by copying the APK to the phone.
- README "Ship" section documents the Android recipe.

## Error handling

- `persist` native backend: read errors ⇒ `None` (fresh start), write
  errors ignored — same policy as the existing localStorage code, where
  quota exhaustion is silently dropped.
- Missing Android data dir ⇒ fall back to current dir rather than
  panic.

## Testing

- `cargo test -p habits` (native): existing suite + file-backend
  round-trip test.
- Web smoke: `dx serve` still works, localStorage persistence intact.
- Device: install APK, add habit, force-close, reopen — data persists.

## Risks

1. **dx Android bundling on NixOS** — an old open upstream issue
   (dioxus#3762) reports bundling failures; likely stale. Exit: env
   vars (`JAVA_HOME` et al.) from the devenv android module.
2. **dx release signing behavior** unclear in 0.7 docs. Exit: keystore
   in Gradle config, or ship the debug-signed APK.
3. **Data-dir API shape** in Dioxus 0.7 mobile. Exit: one small JNI
   call via `ndk-context` to `getFilesDir()`.

## Out of scope

- Data sync/export between web and Android instances.
- iOS, Play Store distribution, release keystore management in CI.
- 32-bit / x86 Android targets.
