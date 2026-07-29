# good-vibes-only

A cargo workspace hosting small, independent vibe-coded Rust projects.

- Each project is a workspace member under `crates/`. Start a new one with
  `cargo new crates/<name>` — the root `Cargo.toml` picks it up via glob.
- New crates inherit `version`, `edition`, and `license` from
  `[workspace.package]` (`key.workspace = true` in the crate manifest).
- Build/test from the repo root: `cargo build`, `cargo test`,
  `cargo run -p <name>`.
- Pre-commit hooks enforce rustfmt, clippy, and Conventional Commit
  messages (`feat:`, `fix:`, `chore:`, ...). Fix failures; never
  `--no-verify`.
- The flake's `packages.default` builds the whole workspace; keep
  `Cargo.lock` committed and up to date.
