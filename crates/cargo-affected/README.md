# cargo affected

Find changed Cargo workspace packages **and every workspace package that depends
on them**, directly or transitively. Produces a package list for selective CI
jobs, with explanations available for debugging a selection.

```sh
cargo install --path crates/cargo-affected --locked
cargo affected --base origin/main
cargo affected --base origin/main --format json --explain
```

Requires Git and Cargo on `PATH`, a toolchain capable of reading both revisions,
and a committed, up-to-date `Cargo.lock` in each existing workspace. Cargo may
fetch dependencies to read metadata; use `--offline` when both revisions' sources
are already cached. The tool does not build or test the workspace.

For `app -> middle -> core`, changing `core` selects `app`, `middle`, and `core`.
An independent package stays out of the selection.

## Revision and output behavior

- `--base REF` is required. Fetch enough Git history to resolve it.
- `--head REF` defaults to `HEAD`. Only committed contents are analyzed; staged,
  unstaged and untracked changes are excluded. Your checkout is not modified.
- By default, compare the merge base of base/head to head, as for a PR diff.
  `--exact` instead compares the two revision tips, useful for push events.
- `--manifest-path rust/Cargo.toml` selects a nested workspace. The path must
  currently exist in your checkout; it identifies the same path in both snapshots.
  If that workspace did not exist at base, all head members are selected.
- Plain output is one package name per line. `--format json` emits a sorted JSON
  array, e.g. `["app","core","middle"]`. `--explain` writes reasons to stderr,
  leaving stdout machine-readable. An empty selection succeeds with `[]` in JSON
  mode (no lines in plain mode).
- Missing history, invalid manifests/rules, stale lockfiles, unavailable
  dependencies, and unsupported local dependencies outside the repository are
  errors with a nonzero exit status. They never produce a successful empty list.

You can also invoke the installed binary directly as `cargo-affected`.

## What counts as affected?

Both revisions contribute dependency edges, so removed dependencies and deleted
packages still invalidate their former consumers. Only current workspace members
appear in the output. Normal, build, development, renamed, optional and
platform-specific dependencies are included. Local path dependencies outside the
workspace but inside the repository are tracked too.

| Change | Selection |
| --- | --- |
| File under a local package | Its owner and transitive dependents |
| Package manifest | That package and dependents, ignoring TOML comments/formatting |
| Inherited workspace dependency or package field | Packages whose effective Cargo metadata changes, then dependents |
| Inherited workspace lints | Inheriting packages and dependents |
| Dependency version, Git revision, resolved edges or features | Consumers of the changed resolution and dependents |
| Added/removed workspace member | Added package or surviving former dependents |
| Root profiles, patches, resolver or arbitrary workspace metadata | All members |
| Cargo configuration or Rust toolchain file | All members |
| Unmapped file outside every local package | All members, unless an explicit rule covers it |

Analysis uses `cargo metadata --all-features --locked`, without filtering targets.
This deliberately covers the widest declared feature/platform graph. It can
select more packages than one particular CI feature/target combination needs;
shared feature unification and dev-dependency closure are conservative. Workspaces
whose complete graph cannot resolve together fail with Cargo's diagnostic.

The snapshots preserve committed contents even if `.gitattributes` uses
`export-ignore` or `export-subst`.

## Shared inputs and explicit ignores

Configure additional consumers of files such as schemas or shared build scripts
in the root manifest. Globs are repository-relative, including for nested
workspaces. Matching rules accumulate and apply to both revisions, including
manifest and lockfile paths.

```toml
[[workspace.metadata.affected.rules]]
paths = ["schemas/**", "build/common.rs"]
packages = ["api", "client"]

[[workspace.metadata.affected.rules]]
paths = ["ci/test-environment/**"]
packages = ["*"]

[[workspace.metadata.affected.rules]]
paths = ["docs/**", "README.md"]
packages = []
```

The last rule suppresses the fallback for **unowned** files. Rules add consumers;
they cannot suppress intrinsic package changes or global build settings. In a
workspace with a root package, that package owns otherwise-unowned files, so a
root README still selects the root package. Changing selection rules selects all
members for that transition. Misspelled package names and invalid globs fail.

Rust code and build scripts can read arbitrary files. Declare shared input rules
for files read across package boundaries, including symlink targets, `include!`
inputs, and generated-code inputs. Changes to files outside the repository,
environment variables, external services, tool installations, submodule contents
and Git LFS payloads are not analyzed. Gitlink/LFS pointer changes are ordinary
file changes, but their payloads are not fetched into snapshots. An affected list
is a conservative dependency analysis, not proof of semantic equivalence.

## GitHub Actions

See [the complete workflow](examples/affected-tests.yml). It:

1. Fetches full Git history.
2. Installs this tool from the checkout.
3. Exposes the JSON array as a job output.
4. Runs one `cargo test --locked -p <package>` job per selected package, skipping
   the matrix job when the array is empty.

The example assumes the tool is checked in at `crates/cargo-affected` as in this
repository. For another repository, install from a pinned Git revision of this
project or provide a prebuilt binary. Keep broader checks for environment or
build-system changes your per-package tests do not cover. The repository's
existing Nix CI remains unchanged.

## Development

```sh
cargo test -p cargo-affected
cargo clippy -p cargo-affected --all-targets -- -D warnings
```

Integration fixtures use real temporary Git repositories and Cargo. The lockfile
update fixture uses a local `file://` Git dependency, requiring no network.
