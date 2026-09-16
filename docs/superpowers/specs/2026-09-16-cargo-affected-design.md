# Cargo affected

Approved in conversation on 2026-09-16. Implement a Rust Cargo subcommand at
`crates/cargo-affected`, inheriting workspace version, edition and license.

## Interface

`cargo affected --base REF [--head HEAD] [--exact] [--format plain|json] [--explain] [--offline] [--manifest-path PATH]`.
Compare committed revisions only, defaulting to the merge base of base and head;
`--exact` compares the two tips. JSON is a sorted array of current workspace
package names, directly usable in a GitHub Actions matrix. Explanations go to
stderr. Empty selections succeed; invalid revisions, invalid manifests and
unavailable metadata fail with actionable errors, never an empty success.

## Analysis

Export both revisions to temporary directories without modifying the checkout.
Use Cargo metadata (all features, all targets) for package identities, workspace
membership and resolved dependencies. Add declared local dependency edges so
normal, dev, build, renamed, optional and target dependencies participate.
Normalize snapshot paths before comparison. Inspect changed files against both
snapshots, taking the most specific local package directory as owner.
Compute reverse reachability in the union of both dependency graphs; output
only workspace members present in the head revision.

Compare parsed package manifests, ignoring comments and formatting. Compare
expanded Cargo package metadata to identify inherited workspace package and
dependency changes. Compare inherited workspace lint tables explicitly.
Compare resolved dependency identities, features and edges to catch lockfile
updates and propagate through consumers. Root resolver, profiles, patches,
Cargo config and toolchain changes select all members with explanations.
Workspace member list edits alone do not select unrelated packages. If a base
workspace did not exist, select all current members. Metadata failures are errors.

## Shared files

Configuration lives under `workspace.metadata.affected`:
`[[workspace.metadata.affected.rules]]` with `paths` (repository-relative globs)
and `packages` (workspace package names or `*`). Consult both revisions. Invalid
rules are errors. Unowned files conservatively select all members unless matched
by a rule; a rule with an empty package array explicitly ignores matching files.
This gives a safe default for arbitrary build-script inputs. Tracked symlinks and
inputs outside the repository need documented limitations; no claim of proving
semantic equivalence of arbitrary code or environment changes.

## Validation and delivery

Integration tests use real temporary Git repositories and real Cargo metadata,
with local dependencies to keep tests offline. Cover chains, cycles, dependency
kinds, root/package manifests, inherited settings, lockfiles, deletions, renames,
merge-base behavior, shared rules, errors and clean output. Include install/CLI
documentation and a GitHub Actions matrix example with full Git history and an
empty-matrix guard. Verify rustfmt, clippy, workspace tests and commit all work.
