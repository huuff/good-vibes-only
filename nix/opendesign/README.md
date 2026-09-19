# OpenDesign Nix packaging

This directory packages the unmodified official OpenDesign release pinned by
the root flake's `opendesign-src` input. It is packaging glue, not a product
fork: do not vendor or patch upstream application source here.

## Outputs

The root flake exposes:

| Output | Contents |
| --- | --- |
| `packages.<system>.opendesign` | The `od` daemon and CLI |
| `packages.<system>.opendesign-web` | The static web frontend |
| `homeManagerModules.open-design` | User-scoped daemon and optional web service |
| `nixosModules.open-design` | System-scoped daemon and optional web service |

The service modules retain the `services.open-design` option namespace. They
default to the two packages above, so existing module consumers do not need to
set package overrides.

## Updating OpenDesign

1. Change `inputs.opendesign-src.url` in the root `flake.nix` to the desired
   official release tag.
2. Run `nix flake update opendesign-src`.
3. Confirm the workspace dependency lists in `default.nix` still cover the
   daemon and web packages.
4. Refresh the fixed-output hashes in `pnpm-deps.nix` by temporarily using
   `lib.fakeHash` and building each package.
5. Run `nix flake check --no-build`, build both OpenDesign packages, and run
   the module smoke tests.

OpenDesign's frontend is a static SPA. When `webFrontend.enable = true`, the
modules serve it through Caddy and reverse-proxy `/api/*`, `/artifacts/*`, and
`/frames/*` to the daemon. BYOK secrets belong in `environmentFile`, never in
Nix configuration or the Nix store.
