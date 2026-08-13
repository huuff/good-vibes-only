# Codex project trust state

The `codex-trust-state` package is a small patch over upstream Codex CLI. It
stores project trust decisions in `$CODEX_HOME/project-trust.toml` (normally
`~/.codex/project-trust.toml`) instead of modifying `config.toml`.

Project trust is mutable application state: Codex records it when a project is
trusted or explicitly marked untrusted. Keeping it separate prevents routine
trust prompts from rewriting a declarative, potentially version-controlled
`config.toml`.

## Install or run

Build the package from this repository:

```console
nix build .#codex-trust-state
```

Or run it without installing:

```console
nix run .#codex-trust-state
```

The package installs the normal `codex` executable. It can also be consumed as
`packages.codex-trust-state` through this flake's default overlay.

## File format and compatibility

Codex creates the state file as needed, using the same project table format:

```toml
[projects."/home/user/src/example"]
trust_level = "trusted"
```

Both `trusted` and `untrusted` decisions are stored there. Existing
`[projects]` entries in `config.toml` remain supported, so no manual migration
is required. If a project appears in both files, `project-trust.toml` wins. New
trust decisions are written only to `project-trust.toml`.

The file is filtered through a trust-only schema when loaded; unrelated Codex
settings placed there do not become configuration.

## Maintenance

Upstream Codex is pinned by `flake.lock`. The behavior change lives entirely in
`nix/patches/codex-project-trust-file.patch`. When updating the `codex` input,
rebuild this package and refresh the patch only if upstream changed one of the
touched trust-loading or persistence paths.
