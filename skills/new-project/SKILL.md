---
name: new-project
description: Scaffold a new project with a Nix flake export, a standalone devenv shell, pre-commit hooks (secret scanning, static analysis, Conventional Commits), and agent instructions. Use when the user asks to create, bootstrap, or scaffold a new project.
---

# New project scaffold

Creates a project that is simultaneously:

1. **A flake** — consumable from nix. It always exports *something* useful, but
   **what** depends on the project: a build (`packages.default` +
   `overlays.default`) only when the project actually produces a buildable
   artifact. For config-only projects (Terraform/OpenTofu, dotfiles, plain
   scripts, docs) a `packages.default` that just copies the files into the store
   is useless — export `nixosModules`/`homeManagerModules`/`lib`, dev tooling,
   or nothing at all instead. When unsure whether a package export makes sense,
   **ask the user**. See step 3.
2. **A devenv shell** — standalone (`devenv.yaml` + `devenv.nix` + `devenv.lock`),
   deliberately **not** via the flake's `devShells`. Rationale: the flake
   integration needs `--impure`, ties devenv to the flake's nixpkgs, and
   restricts devenv features (containers, `devenv up` processes, its own
   per-input lockfile). Keeping them side by side also keeps the flake minimal
   for consumers — dev tooling never leaks into the export.
3. **Guarded by git hooks** — secrets scanning, static analysis, and
   Conventional Commits enforcement, all managed declaratively through
   devenv's `git-hooks` module (installed automatically on shell entry).
4. **Agent-ready** — an `AGENTS.md` (symlinked as `CLAUDE.md`) with concise
   project instructions.
5. **Secrets via sops** — if the application needs secrets at runtime, they
   are provided with [sops](https://github.com/getsops/sops) (age-encrypted,
   committed to the repo), never as plaintext files or env vars in the shell
   config. See "Runtime secrets" below.

## Workflow

### 1. Gather parameters

Needed before writing anything:

- **Directory / project name** (kebab-case; used as `pname`).
- **One-line description**.
- **Language(s)** — infer from the user's request; if genuinely unknown, ask.

### 2. Copy templates

All templates live in `templates/` next to this file. Copy and rename:

| Template | Destination |
|---|---|
| `templates/flake.nix` | `flake.nix` |
| `templates/nix/package.nix` | `nix/package.nix` |
| `templates/devenv.yaml` | `devenv.yaml` |
| `templates/devenv.nix` | `devenv.nix` |
| `templates/envrc` | `.envrc` |
| `templates/gitignore` | `.gitignore` |

Then replace every `CHANGEME_PNAME` / `CHANGEME_DESCRIPTION` placeholder and
resolve the remaining `# CHANGEME` comments (language blocks, license).

### 3. Adapt to the language

In `devenv.nix`, enable `languages.<lang>` and the matching hooks:

| Language | devenv | extra git-hooks |
|---|---|---|
| Rust | `languages.rust.enable = true;` | `clippy`, `rustfmt` |
| Python | `languages.python.enable = true;` (+ `uv.enable`) | `ruff`, `ruff-format` |
| JS/TS | `languages.javascript = { enable = true; npm.enable = true; };` | `eslint`, `prettier` |
| Go | `languages.go.enable = true;` | `golangci-lint`, `gofmt` |
| Nix-only | `languages.nix.enable = true;` (already on) | already covered |

First decide whether a `packages.default` even belongs here. Export a package
**only when the project produces a real buildable artifact** (a binary, a
library, a bundled app). For projects that are just config or files consumed by
some other tool — Terraform/OpenTofu, Ansible, k8s manifests, dotfiles, docs —
a derivation that copies those files into the nix store buys nothing; skip it
and use the "Pure nix config/modules" branch below (export modules/`lib`, or
just the devenv shell + hooks). If it's genuinely unclear which case you're in,
**ask the user before writing `nix/package.nix`** rather than defaulting to a
useless copy-to-store derivation.

When a package does make sense, replace the stub in `nix/package.nix` with the
right builder:

- Rust: `rustPlatform.buildRustPackage { cargoLock.lockFile = ../Cargo.lock; ... }`
- Go: `buildGoModule { vendorHash = ...; ... }`
- Python: `python3Packages.buildPythonApplication { pyproject = true; ... }`
- Node: `buildNpmPackage { npmDepsHash = ...; ... }`
- Pure nix config/modules: drop `nix/package.nix`, export `nixosModules` /
  `homeManagerModules` / `lib` from the flake instead, and point
  `checks` at an eval test or leave only formatting checks.

The stub is intentionally buildable, so the scaffold passes `nix flake check`
before any real code exists.

### 4. Runtime secrets with sops (skip if the app needs none)

Secrets the application needs at runtime are managed with sops + age:
encrypted files are committed, the private key never enters the repo.

1. Uncomment `pkgs.sops` / `pkgs.age` in `devenv.nix` `packages`, and the
   `scripts.with-secrets` block.
2. Ensure the user has an age key (`~/.config/sops/age/keys.txt`); if not:
   `mkdir -p ~/.config/sops/age && age-keygen -o ~/.config/sops/age/keys.txt`.
   Never write this key into the project.
3. Create `.sops.yaml` at the repo root with the public recipient
   (from `age-keygen -y ~/.config/sops/age/keys.txt`):

   ```yaml
   creation_rules:
     - path_regex: secrets/.*\.yaml$
       age: age1...publickey...
   ```

4. Create the encrypted file: `sops secrets/dev.yaml` (opens $EDITOR; write
   `KEY: value` pairs). The resulting file is encrypted — committing it is
   safe and expected. Do **not** gitignore `secrets/`.
5. Run the app as `with-secrets '<command>'` — it uses `sops exec-env`, so
   decrypted values exist only in that process's environment, never on disk.

For production/NixOS deployment of the same secrets, point the user at
[sops-nix](https://github.com/Mic92/sops-nix) (`sops.secrets.*` module
options); the encrypted files and `.sops.yaml` carry over unchanged.

### 5. Agent instructions (AGENTS.md)

Agent instructions are harness-agnostic: the real file is `AGENTS.md`,
and harness-specific names are symlinks into it:

```bash
ln -s AGENTS.md CLAUDE.md
```

Write a short `AGENTS.md` (a few lines, not a manual): what the project
is, how to build/test it, and any conventions an agent must follow.
Don't duplicate what the code or scaffold already makes obvious. Commit
both the file and the symlink.

### 6. Initialize and verify

```bash
git init -b main
devenv shell -- true        # builds the shell, writes devenv.lock, installs git hooks
nix flake check --no-build  # flake evaluates
devenv test                 # runs enterTest (flake evals)
git add -A
git commit -m "chore: scaffold project"   # must pass all hooks, incl. commitizen
```

Commit `devenv.lock` and `flake.lock`; `.pre-commit-config.yaml` and
`.devenv/` are generated and gitignored.

If any hook fails on the initial commit, fix the offending file — do not
bypass with `--no-verify`.

### 7. Hand over

Tell the user: enter with `devenv shell` (or `direnv allow` for automatic
activation) and consume the project via its `github:<owner>/<repo>` flake ref.

## Scripts: bash vs nushell

devenv `scripts.*` run with bash by default. Keep bash for trivial one-line
exec wrappers. When a script has real logic — filtering lists, parsing
JSON/structured output, or more than a couple of conditionals — write it in
nushell instead; it will usually be clearer.

```nix
scripts.my-script = {
  package = pkgs.nushell; # binary defaults to meta.mainProgram = "nu"
  exec = ''
    http get https://api.example.com/items | where size > 10mb | to md
  '';
};
```

Nushell-specific notes:

- A script that receives CLI arguments needs `def --wrapped main [...args]`;
  `--wrapped` stops nu from parsing flags meant for the wrapped command.
- Nu interpolation is `$"(...)"`, which doesn't collide with nix `''...''`
  strings — no `''${}` escaping needed, unlike bash.
- Nu `mkdir` already has `mkdir -p` semantics: creates parents, no error if
  the directory exists (there is no `-p` flag).
