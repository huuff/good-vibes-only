# Cargo Affected Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for the tightly coupled implementation and superpowers:requesting-code-review for independent review. Track steps with checkboxes.

**Goal:** Select affected Cargo workspace members for CI, including transitive dependents and manifest/lockfile effects.

**Architecture:** Git exports two committed snapshots. Cargo metadata and parsed manifests yield normalized package graphs; an analyzer seeds changes and computes reverse reachability. The CLI formats results and diagnostics.

**Tech Stack:** Rust, anyhow, clap, serde/serde_json, toml, tempfile, tar, globset; system Git and Cargo.

**Spec:** `docs/superpowers/specs/2026-09-16-cargo-affected-design.md`

## Global Constraints

- Inherit workspace version, edition and license.
- Compare committed revisions, default merge base; exact-tip option.
- Include both graphs and every declared local dependency kind.
- Sorted JSON array on stdout; explanations on stderr.
- Errors must never become an empty successful selection.
- Test using real Git and Cargo with offline local fixtures.

## Task 1: Snapshot analysis and CLI

Files: `crates/cargo-affected/Cargo.toml`, `src/main.rs`, `src/git.rs`,
`src/snapshot.rs`, `src/analysis.rs`, `tests/cli.rs`, root `Cargo.lock`.

Interfaces: `git::Repository::open(&Path)`, `Repository::resolve(&str)`,
`Repository::merge_base(&str, &str)`, `Repository::export(&str)`,
`Repository::changed(&str, &str)`; `Snapshot::load(&Path, &Path, bool)`;
`analysis::analyze(&Snapshot, &Snapshot, &[String]) -> Selection`.
`Selection` maps current member names to sorted reason sets.

- [x] Scaffold with `cargo new crates/cargo-affected`; set inherited fields and dependencies.
- [x] Write fixture helpers that initialize Git, create local library crates, commit and invoke the real binary.
- [x] Add tests asserting a changed leaf selects `["app", "core", "middle"]`, while an independent crate stays excluded; unchanged snapshots emit `[]`.
- [x] Run `cargo test -p cargo-affected --test cli`; observe failing output against the generated main.
- [x] Implement Git snapshots, Cargo metadata normalization and reverse closure; run the same tests.
- [x] Add failing tests for manifest inheritance, deleted/renamed crates and removed edges; implement semantic manifest and graph comparison.
- [x] Add failing tests for resolved lock changes and global settings; implement dependency-resolution fingerprints and global invalidation.
- [x] Add failing tests for shared-file rules, CLI errors, exact/merge-base comparison and explanations; implement these behaviors.
- [x] Run `cargo test -p cargo-affected` and `cargo clippy -p cargo-affected --all-targets -- -D warnings`.

## Task 2: Documentation and independent review

Files: `crates/cargo-affected/README.md`, `examples/affected-tests.yml`.

- [x] Document committed-snapshot behavior, conservative cases, external inputs and requirements.
- [x] Provide install commands and a JSON matrix workflow guarded with `needs.affected.outputs.packages != '[]'`.
- [x] Exercise the CLI against this repository and an actual example fixture.
- [x] Run `cargo fmt --all -- --check` and `cargo test --workspace --locked`.
- [x] Review complete diff for correctness and spec coverage; fix findings with regression tests.
- [x] Commit with Conventional Commit messages and confirm a clean worktree.

## Verification record

- Original workspace baseline: 107 tests passed.
- New CLI: 31 integration tests passed, exercising real Git and Cargo, including
  lockfile-only Git updates and all independently reported regressions.
- Strict package Clippy and workspace formatting passed.
- `nix flake check --no-build` passed on x86_64-linux.
- Historical repository change `46acb765f^..46acb765f` selected only `tally`,
  with explanations identifying its two changed source files.
- Independent review approved the final fixes for export attributes, shared rules
  on manifests, nested inherited lints, SHA-256 Git, and temporary path encoding.

## Decisions clarified during implementation

- Rules add consumers and suppress only the unowned-file fallback; they cannot
  suppress intrinsic package changes. This preserves safe dependency selection.
- Arbitrary workspace metadata is conservatively treated as a shared build input.
- Native Nix checks supply Git for the new integration tests. Existing CI remains
  unchanged; the selectable matrix is delivered as a complete example workflow.
