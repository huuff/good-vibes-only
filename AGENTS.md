# good-vibes-only

A cargo workspace hosting small, independent vibe-coded Rust projects,
plus reusable Nix modules under `nix/`.

- Each project is a workspace member under `crates/`. Start a new one with
  `cargo new crates/<name>` — the root `Cargo.toml` picks it up via glob.
- New crates inherit `version`, `edition`, and `license` from
  `[workspace.package]` (`key.workspace = true` in the crate manifest).
- Build/test from the repo root: `cargo build`, `cargo test`,
  `cargo run -p <name>`.
- Pre-commit hooks enforce rustfmt, clippy, and Conventional Commit
  messages (`feat:`, `fix:`, `chore:`, ...). Fix failures; never
  `--no-verify`.
- The flake exports one package per crate (`packages.<name>`, built
  with `cargo build -p <name>`); keep `Cargo.lock` committed and up
  to date.
- Home-manager modules live in `nix/home-manager/`; each `<name>.nix`
  is auto-exported as `homeManagerModules.<name>` (alias `homeModules`),
  no flake edits needed. Give each one an eval-only smoke test in the
  flake's `checks` (see `hm-nono`) — `nix flake check --no-build` must
  pass.
