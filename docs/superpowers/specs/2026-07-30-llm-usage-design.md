# llm-usage — design

2026-07-30. Status: implemented in the same session (autonomous run; no user
review possible mid-task — decisions below were taken with sensible defaults).

## Purpose

A small CLI (`cargo run -p llm-usage`) that detects which LLM coding CLIs are
set up on this machine and prints the current usage limits for each, as the
tools' own `/usage` / `/status` screens would.

## Scope (v1)

Providers: **Claude Code** and **Codex**. Anything else prints nothing — the
provider list is a `Vec<Box<dyn Provider>>`, so adding more later is one new
module.

## Data sources

### Claude Code

- Detection: `~/.claude/.credentials.json` exists and contains
  `claudeAiOauth.accessToken` (also carries `subscriptionType`,
  `rateLimitTier`, `expiresAt`).
- Usage: `GET https://api.anthropic.com/api/oauth/usage` with
  `Authorization: Bearer <accessToken>` and `anthropic-beta: oauth-2025-04-20`
  — the same endpoint Claude Code's `/usage` uses. Verified live on this
  machine. The response's `limits` array is the primary source: each entry has
  `kind`, `percent`, `severity`, `resets_at`, and an optional `scope` with a
  model display name (e.g. the per-model weekly limit).
- Expired token (`expiresAt` in the past, or HTTP 401): report "token
  expired — open claude to refresh", don't fail the whole run.

### Codex

- Detection: `auth.json` exists under the Codex home (`CODEX_HOME`, else
  `~/.codex`). `auth.json` also says whether this is a ChatGPT login or a
  plain `OPENAI_API_KEY`; API-key users are billed per token and have no plan
  windows, so they get a note instead of bars.
- Usage (primary, live): spawn `codex app-server`, do the JSON-RPC handshake
  (`initialize` → `initialized`), then call **`account/rateLimits/read`**.
  This is the interface Codex's own UI uses, so Codex refreshes the OAuth
  token and talks to the backend itself — we never touch the tokens. A reader
  thread plus a 20s timeout keeps a wedged server from hanging the CLI, and
  the child is killed when we're done.
  - Method name and handshake were confirmed empirically against
    codex-cli 0.145.0 (the app-server enumerates its methods on a bad
    request, and `account/rateLimits/read` answers "authentication
    required" when unauthenticated).
- Usage (fallback): the newest `rate_limits` snapshot from a session rollout
  under `<codex home>/sessions/YYYY/MM/DD/*.jsonl`, scanning files
  newest-first (mtime) and each from the last line upward. This is only as
  fresh as the last Codex run, so it renders with "as of <t> ago" plus a note
  explaining why live data was unavailable. **Never** present it as current:
  the stale value diverging from what `codex` itself reports is exactly the
  bug this design has to avoid.
- Parsers are schema-tolerant, because rollout files use snake_case while the
  app-server uses camelCase, and the shapes drift between Codex versions:
  find `rate_limits`/`rateLimits` anywhere in the payload (or accept bare
  `primary`/`secondary`), accept `resets_at` as RFC3339 *or* epoch seconds,
  and accept relative `resets_in_seconds`. If a live response can't be
  understood, the error quotes the body (truncated) so the shape can be
  pinned down instead of silently falling back.

## Architecture

```
crates/llm-usage/src/
  main.rs      clap (flag: --json), provider loop, exit code
  report.rs    shared Report/Window types (serde-serializable for --json)
  render.rs    terminal table: usage bar, %, reset time (colored handles tty)
  claude.rs    credentials parsing + oauth/usage call + response types
  codex.rs     auth detection + session-file scan + rate_limits parsing
```

Shared types in `report.rs`: `Report { provider, detail, as_of, note,
windows }` and `Window { label, used_percent, resets_at }`. Each provider
module exposes a detection fn (`credentials_path()` / `codex_home()`) and
`report() -> Result<Report>`. `note` carries caveats (stale data and why,
API-key auth) and renders dim under the heading; `as_of` is set only when
the numbers come from a local snapshot rather than a live call.

- A provider that isn't detected is listed as "not detected" (dim, one line).
- A detected provider whose usage fetch fails prints the error under its
  heading; the other providers still render. Exit code 0 as long as at least
  one provider rendered; 1 if none were detected.
- `--json` prints the collected `Report`s as JSON instead of the table.

## Dependencies

clap (derive), anyhow, serde, serde_json, ureq 2 (blocking, no tokio; used
for the Claude call only), jiff (timestamp parsing, "resets in 2h 13m"),
colored 3 (matches keyloader).

## Testing

- Unit tests (26): Claude response JSON → windows (fixture trimmed from the
  real response) and credentials parsing; Codex rollout lines → snapshot
  across schema variants including a real codex-cli 0.145.0 line, live
  payloads in both snake_case and camelCase, JSON-RPC response/error
  extraction, `CODEX_HOME` resolution, and junk-input handling; bar and
  duration rendering.
- No network or subprocess in tests: the Claude HTTP call and the app-server
  transport are thin wrappers over separately tested parsers.
- Beyond unit tests, the Codex RPC path was exercised end-to-end against a
  real `codex app-server` using a throwaway `CODEX_HOME`, confirming the
  handshake, method name, and error propagation. Only the *authenticated*
  response shape is unverified in the dev sandbox (no readable credentials),
  which the tolerant parser and body-quoting error are there to cover.
