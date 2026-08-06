//! Codex CLI: gets live usage by running `codex app-server` and calling its
//! `account/rateLimits/read` JSON-RPC method — the same interface Codex's own
//! UI uses, so Codex owns the token refresh and backend call.
//!
//! If that fails we fall back to the newest `rate_limits` snapshot Codex left
//! in a session rollout under `<codex home>/sessions/YYYY/MM/DD/*.jsonl` —
//! usable, but only as fresh as the last Codex run, so it gets a stale note.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use jiff::Timestamp;
use serde_json::Value;

use crate::report::{Report, Window};

/// How many of the newest session files to scan before giving up.
const MAX_FILES: usize = 20;
/// JSON-RPC id we use for the rate-limits request.
const RATE_LIMITS_ID: i64 = 2;
const RPC_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct Snapshot {
    pub at: Option<Timestamp>,
    pub plan: Option<String>,
    pub windows: Vec<Window>,
}

/// How Codex is authenticated. API-key users pay per token and have no plan
/// windows, so there is nothing to report for them. The tokens themselves stay
/// in Codex's hands — the app-server does the authenticating.
#[derive(Debug, PartialEq)]
pub enum Auth {
    ChatGpt,
    ApiKey,
}

pub fn parse_auth(json: &str) -> Result<Auth> {
    let value: Value = serde_json::from_str(json)?;
    if value
        .get("tokens")
        .and_then(|t| t.get("access_token"))
        .and_then(Value::as_str)
        .is_some()
    {
        return Ok(Auth::ChatGpt);
    }
    if value
        .get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .is_some()
    {
        return Ok(Auth::ApiKey);
    }
    Err(anyhow!(
        "no usable credentials in auth.json — run `codex login`"
    ))
}

/// Parse a `/api/codex/usage` response into windows plus the plan name.
pub fn parse_live_usage(json: &str, now: Timestamp) -> Result<(Vec<Window>, Option<String>)> {
    let value: Value = serde_json::from_str(json)
        .with_context(|| format!("usage response was not JSON: {}", truncate(json)))?;
    // The endpoint may return the rate-limit snapshot bare or wrapped.
    let limits = find_rate_limits(&value).unwrap_or(&value);
    let windows = windows_from_limits(limits, Some(now));
    if windows.is_empty() {
        return Err(anyhow!(
            "unrecognized usage response shape: {}",
            truncate(json)
        ));
    }
    Ok((windows, plan_type(limits)))
}

/// Pick out the response to request `id` from one app-server output line.
/// `None` means this line is something else (a notification, another id).
fn extract_rpc_response(line: &str, id: i64) -> Option<Result<Value>> {
    let msg: Value = serde_json::from_str(line).ok()?;
    if msg.get("id").and_then(Value::as_i64) != Some(id) {
        return None;
    }
    if let Some(error) = msg.get("error") {
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        return Some(Err(anyhow!("{message}")));
    }
    Some(Ok(msg.get("result").cloned().unwrap_or(Value::Null)))
}

fn truncate(body: &str) -> String {
    let body = body.trim();
    match body.char_indices().nth(400) {
        Some((idx, _)) => format!("{}…", &body[..idx]),
        None => body.to_owned(),
    }
}

pub fn codex_home() -> Option<PathBuf> {
    let path = codex_home_dir(
        std::env::var_os("CODEX_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )?;
    path.join("auth.json").is_file().then_some(path)
}

/// `CODEX_HOME` wins over `~/.codex`, matching Codex's own resolution.
fn codex_home_dir(codex_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<PathBuf> {
    match codex_home {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => Some(PathBuf::from(home?).join(".codex")),
    }
}

/// Extract a rate-limit snapshot from one rollout JSONL line, if it has one.
pub fn parse_rollout_line(line: &str) -> Option<Snapshot> {
    let value: Value = serde_json::from_str(line).ok()?;
    let limits = find_rate_limits(&value)?;
    let at = value
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<Timestamp>().ok());
    let windows = windows_from_limits(limits, at);
    (!windows.is_empty()).then_some(Snapshot {
        at,
        plan: plan_type(limits),
        windows,
    })
}

/// Read the `primary`/`secondary` windows out of a rate-limit snapshot.
/// `anchor` is what relative (`resets_in_seconds`) resets are measured from.
fn windows_from_limits(limits: &Value, anchor: Option<Timestamp>) -> Vec<Window> {
    let mut windows = Vec::new();
    for (key, fallback) in [("primary", "Primary"), ("secondary", "Secondary")] {
        let Some(w) = limits.get(key) else { continue };
        let Some(used_percent) = field(w, "used_percent").and_then(Value::as_f64) else {
            continue;
        };
        // resets_at was RFC3339 in older codex versions, epoch seconds since ~0.14x
        let resets_at = match field(w, "resets_at") {
            Some(Value::String(s)) => s.parse().ok(),
            Some(Value::Number(n)) => n
                .as_i64()
                .and_then(|secs| Timestamp::from_second(secs).ok()),
            _ => None,
        }
        .or_else(|| {
            let seconds = field(w, "resets_in_seconds").and_then(Value::as_i64)?;
            Timestamp::from_second(anchor?.as_second() + seconds).ok()
        });
        windows.push(Window {
            label: window_label(field(w, "window_minutes").and_then(Value::as_i64), fallback),
            used_percent,
            resets_at,
        });
    }
    windows
}

fn plan_type(limits: &Value) -> Option<String> {
    field(limits, "plan_type")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Look up a snake_case field, falling back to its camelCase spelling —
/// rollout files use the former, the app-server the latter.
fn field<'v>(value: &'v Value, snake_case: &str) -> Option<&'v Value> {
    value.get(snake_case).or_else(|| {
        let mut camel = String::with_capacity(snake_case.len());
        let mut capitalize = false;
        for c in snake_case.chars() {
            match (c, capitalize) {
                ('_', _) => capitalize = true,
                (c, true) => {
                    camel.extend(c.to_uppercase());
                    capitalize = false;
                }
                (c, false) => camel.push(c),
            }
        }
        value.get(&camel)
    })
}

/// Newest snapshot across the session files under `sessions_dir`,
/// scanning files newest-first and each file bottom-up.
pub fn latest_snapshot(sessions_dir: &Path) -> Option<Snapshot> {
    let mut files = Vec::new();
    collect_jsonl(sessions_dir, &mut files);
    files.sort_by_key(|(mtime, _)| std::cmp::Reverse(*mtime));
    files.into_iter().take(MAX_FILES).find_map(|(_, path)| {
        let content = std::fs::read_to_string(&path).ok()?;
        content.lines().rev().find_map(parse_rollout_line)
    })
}

fn collect_jsonl(dir: &Path, out: &mut Vec<(std::time::SystemTime, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, out);
        } else if path.extension().is_some_and(|e| e == "jsonl")
            && let Ok(meta) = entry.metadata()
        {
            let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            out.push((mtime, path));
        }
    }
}

pub fn report() -> Result<Report> {
    let home = codex_home().context("no ~/.codex")?;
    let auth_path = home.join("auth.json");
    let auth = parse_auth(&std::fs::read_to_string(&auth_path)?)
        .with_context(|| format!("parsing {}", auth_path.display()))?;

    if matches!(auth, Auth::ApiKey) {
        return Ok(Report {
            provider: "Codex".into(),
            detail: Some("API key auth".into()),
            as_of: None,
            note: Some("billed per token — no plan limits to report".into()),
            windows: Vec::new(),
        });
    }

    let live = fetch_rate_limits().and_then(|body| parse_live_usage(&body, Timestamp::now()));
    match live {
        Ok((windows, plan)) => Ok(Report {
            provider: "Codex".into(),
            detail: plan,
            as_of: None,
            note: None,
            windows,
        }),
        // Fall back to the last snapshot Codex wrote to disk, which is better
        // than nothing but only as fresh as the last session.
        Err(err) => {
            let snapshot = latest_snapshot(&home.join("sessions")).with_context(|| {
                format!("live usage failed ({err:#}) and no session snapshot on disk")
            })?;
            Ok(Report {
                provider: "Codex".into(),
                detail: snapshot.plan,
                as_of: snapshot.at,
                note: Some(format!("live usage unavailable ({err:#})")),
                windows: snapshot.windows,
            })
        }
    }
}

/// Ask `codex app-server` for current rate limits over JSON-RPC — the same
/// interface Codex's own UI uses, so Codex refreshes the OAuth token and talks
/// to the backend for us instead of us reimplementing either.
fn fetch_rate_limits() -> Result<String> {
    let mut child = Command::new("codex")
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("could not run `codex app-server`")?;
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let stdout = child.stdout.take().expect("stdout is piped");

    // Read on another thread so a wedged app-server can't hang us forever.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(response) = extract_rpc_response(&line, RATE_LIMITS_ID) {
                let _ = tx.send(response);
                return;
            }
        }
        let _ = tx.send(Err(anyhow!("app-server closed without answering")));
    });

    let sent = writeln!(
        stdin,
        concat!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":"#,
            r#"{{"clientInfo":{{"name":"llm-usage","title":"llm-usage","version":"{}"}}}}}}"#
        ),
        env!("CARGO_PKG_VERSION")
    )
    .and_then(|()| writeln!(stdin, r#"{{"jsonrpc":"2.0","method":"initialized","params":{{}}}}"#))
    .and_then(|()| {
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":{RATE_LIMITS_ID},"method":"account/rateLimits/read","params":{{}}}}"#
        )
    });

    let result = sent
        .context("writing to app-server")
        .and_then(|()| {
            rx.recv_timeout(RPC_TIMEOUT)
                .context("timed out waiting for app-server")
        })
        .and_then(|response| response);
    let _ = child.kill();
    let _ = child.wait();
    Ok(serde_json::to_string(&result?)?)
}

/// Depth-first search for a `rate_limits` object anywhere in the event —
/// rollout schemas have moved it around across Codex versions.
fn find_rate_limits(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(_) => field(value, "rate_limits")
            .filter(|v| v.is_object())
            .or_else(|| value.as_object()?.values().find_map(find_rate_limits)),
        Value::Array(items) => items.iter().find_map(find_rate_limits),
        _ => None,
    }
}

fn window_label(minutes: Option<i64>, fallback: &str) -> String {
    match minutes {
        None => fallback.into(),
        Some(10080) => "Week".into(),
        Some(m) if m % 1440 == 0 => format!("{}d", m / 1440),
        Some(m) if m % 60 == 0 => format!("{}h", m / 60),
        Some(m) => format!("{m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const TOKEN_COUNT_LINE: &str = r#"{"timestamp":"2026-07-30T10:00:00.000Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":50}},"rate_limits":{"primary":{"used_percent":42.5,"window_minutes":300,"resets_in_seconds":3600},"secondary":{"used_percent":81.0,"window_minutes":10080,"resets_in_seconds":86400}}}}"#;

    // Real line from codex-cli 0.145.0: epoch-seconds resets_at, null
    // secondary, plan_type inside rate_limits.
    const CODEX_0145_LINE: &str = r#"{"timestamp":"2026-07-29T19:22:36.135Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":875573,"cached_input_tokens":829696,"cache_write_input_tokens":0,"output_tokens":6257,"reasoning_output_tokens":2427,"total_tokens":881830},"model_context_window":258400},"rate_limits":{"limit_id":"codex","limit_name":null,"primary":{"used_percent":2.0,"window_minutes":10080,"resets_at":1785914410},"secondary":null,"credits":{"has_credits":false,"unlimited":false,"balance":"0"},"individual_limit":null,"spend_control_reached":null,"plan_type":"prolite","rate_limit_reached_type":null}}}"#;

    #[test]
    fn parses_codex_0145_schema() {
        let snap = parse_rollout_line(CODEX_0145_LINE).unwrap();
        assert_eq!(snap.windows.len(), 1);
        assert_eq!(snap.windows[0].label, "Week");
        assert_eq!(snap.windows[0].used_percent, 2.0);
        // resets_at is epoch seconds in this schema
        assert_eq!(
            snap.windows[0].resets_at,
            Some(Timestamp::from_second(1785914410).unwrap())
        );
        assert_eq!(snap.plan.as_deref(), Some("prolite"));
    }

    #[test]
    fn parses_token_count_event() {
        let snap = parse_rollout_line(TOKEN_COUNT_LINE).unwrap();
        assert_eq!(snap.at.unwrap().to_string(), "2026-07-30T10:00:00Z");
        assert_eq!(snap.windows.len(), 2);
        assert_eq!(snap.windows[0].label, "5h");
        assert_eq!(snap.windows[0].used_percent, 42.5);
        // resets_in_seconds is relative to the event timestamp
        assert_eq!(
            snap.windows[0].resets_at.unwrap().to_string(),
            "2026-07-30T11:00:00Z"
        );
        assert_eq!(snap.windows[1].label, "Week");
        assert_eq!(snap.windows[1].used_percent, 81.0);
    }

    #[test]
    fn parses_absolute_resets_at_variant() {
        let line = r#"{"timestamp":"2026-07-30T10:00:00Z","payload":{"rate_limits":{"primary":{"used_percent":10.0,"window_minutes":300,"resets_at":"2026-07-30T12:34:56Z"}}}}"#;
        let snap = parse_rollout_line(line).unwrap();
        assert_eq!(
            snap.windows[0].resets_at.unwrap().to_string(),
            "2026-07-30T12:34:56Z"
        );
    }

    #[test]
    fn labels_unusual_windows_by_duration() {
        let line = r#"{"payload":{"rate_limits":{"primary":{"used_percent":1.0,"window_minutes":60},"secondary":{"used_percent":2.0}}}}"#;
        let snap = parse_rollout_line(line).unwrap();
        assert_eq!(snap.windows[0].label, "1h");
        assert_eq!(snap.windows[1].label, "Secondary");
        assert!(snap.windows[0].resets_at.is_none());
    }

    #[test]
    fn ignores_lines_without_rate_limits() {
        assert!(parse_rollout_line(r#"{"type":"response_item","payload":{}}"#).is_none());
        assert!(parse_rollout_line("not json at all").is_none());
        // has the key but no usable windows
        assert!(parse_rollout_line(r#"{"payload":{"rate_limits":{}}}"#).is_none());
    }

    #[test]
    fn latest_snapshot_prefers_newest_file_and_last_line() {
        let dir = tempfile::tempdir().unwrap();
        let day = dir.path().join("2026/07/30");
        fs::create_dir_all(&day).unwrap();

        let old = day.join("rollout-old.jsonl");
        fs::write(&old, format!("{TOKEN_COUNT_LINE}\n")).unwrap();

        let newer = day.join("rollout-new.jsonl");
        let last = TOKEN_COUNT_LINE.replace("42.5", "99.0");
        fs::write(&newer, format!("junk\n{TOKEN_COUNT_LINE}\n{last}\n")).unwrap();
        // ensure mtime ordering regardless of write speed
        let past = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        let f = fs::File::options().append(true).open(&old).unwrap();
        f.set_modified(past).unwrap();

        let snap = latest_snapshot(dir.path()).unwrap();
        assert_eq!(snap.windows[0].used_percent, 99.0);
    }

    #[test]
    fn codex_home_prefers_codex_home_env() {
        let home = OsStr::new("/home/somebody");
        assert_eq!(
            codex_home_dir(Some(OsStr::new("/custom/codex")), Some(home)).unwrap(),
            PathBuf::from("/custom/codex")
        );
        assert_eq!(
            codex_home_dir(None, Some(home)).unwrap(),
            PathBuf::from("/home/somebody/.codex")
        );
        // an empty CODEX_HOME is treated as unset
        assert_eq!(
            codex_home_dir(Some(OsStr::new("")), Some(home)).unwrap(),
            PathBuf::from("/home/somebody/.codex")
        );
        assert!(codex_home_dir(None, None).is_none());
    }

    #[test]
    fn parses_chatgpt_auth() {
        let json = r#"{
          "OPENAI_API_KEY": null,
          "tokens": {
            "id_token": "fake-id-token",
            "access_token": "fake-access-token",
            "refresh_token": "fake-refresh-token",
            "account_id": "acct-123"
          },
          "last_refresh": "2026-07-29T19:00:00.000Z"
        }"#;
        assert_eq!(parse_auth(json).unwrap(), Auth::ChatGpt);
    }

    #[test]
    fn parses_api_key_auth() {
        let json = r#"{"OPENAI_API_KEY": "fake-api-key", "tokens": null}"#;
        assert!(matches!(parse_auth(json).unwrap(), Auth::ApiKey));
    }

    #[test]
    fn rejects_auth_without_credentials() {
        assert!(parse_auth(r#"{"OPENAI_API_KEY": null, "tokens": null}"#).is_err());
    }

    #[test]
    fn parses_live_usage_wrapped_in_rate_limits() {
        let now: Timestamp = "2026-07-30T10:00:00Z".parse().unwrap();
        let json = r#"{"rate_limits":{"primary":{"used_percent":7.0,"window_minutes":10080,"resets_at":1785914410},"secondary":null,"plan_type":"prolite"}}"#;
        let (windows, plan) = parse_live_usage(json, now).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].label, "Week");
        assert_eq!(windows[0].used_percent, 7.0);
        assert_eq!(
            windows[0].resets_at,
            Some(Timestamp::from_second(1785914410).unwrap())
        );
        assert_eq!(plan.as_deref(), Some("prolite"));
    }

    #[test]
    fn parses_live_usage_bare_windows() {
        let now: Timestamp = "2026-07-30T10:00:00Z".parse().unwrap();
        let json = r#"{"primary":{"used_percent":7.5,"window_minutes":300,"resets_in_seconds":600},"secondary":{"used_percent":12.0,"window_minutes":10080}}"#;
        let (windows, _) = parse_live_usage(json, now).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].label, "5h");
        assert_eq!(windows[0].used_percent, 7.5);
        // relative resets are anchored to now when there's no event timestamp
        assert_eq!(
            windows[0].resets_at.unwrap().to_string(),
            "2026-07-30T10:10:00Z"
        );
    }

    #[test]
    fn parses_live_usage_camel_case() {
        // The app-server serializes its payloads camelCase.
        let now: Timestamp = "2026-07-30T10:00:00Z".parse().unwrap();
        let json = r#"{"rateLimits":{"primary":{"usedPercent":7.0,"windowMinutes":10080,"resetsAt":1785914410},"secondary":null,"planType":"prolite"}}"#;
        let (windows, plan) = parse_live_usage(json, now).unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].label, "Week");
        assert_eq!(windows[0].used_percent, 7.0);
        assert_eq!(
            windows[0].resets_at,
            Some(Timestamp::from_second(1785914410).unwrap())
        );
        assert_eq!(plan.as_deref(), Some("prolite"));
    }

    #[test]
    fn parses_camel_case_relative_reset() {
        let now: Timestamp = "2026-07-30T10:00:00Z".parse().unwrap();
        let json = r#"{"primary":{"usedPercent":1.0,"windowMinutes":300,"resetsInSeconds":1800}}"#;
        let (windows, _) = parse_live_usage(json, now).unwrap();
        assert_eq!(
            windows[0].resets_at.unwrap().to_string(),
            "2026-07-30T10:30:00Z"
        );
    }

    #[test]
    fn rpc_result_is_returned_for_matching_id() {
        let line = r#"{"id":2,"result":{"rateLimits":{"primary":{"usedPercent":3.0}}}}"#;
        let got = extract_rpc_response(line, 2).unwrap().unwrap();
        assert!(got.get("rateLimits").is_some());
    }

    #[test]
    fn rpc_error_becomes_an_error_with_its_message() {
        let line = r#"{"error":{"code":-32600,"message":"codex account authentication required to read rate limits"},"id":2}"#;
        let err = extract_rpc_response(line, 2)
            .unwrap()
            .unwrap_err()
            .to_string();
        assert!(err.contains("authentication required"), "got: {err}");
    }

    #[test]
    fn rpc_ignores_other_ids_and_notifications() {
        assert!(extract_rpc_response(r#"{"id":1,"result":{}}"#, 2).is_none());
        assert!(extract_rpc_response(r#"{"method":"configWarning"}"#, 2).is_none());
        assert!(extract_rpc_response("not json", 2).is_none());
    }

    #[test]
    fn live_usage_error_quotes_unrecognized_body() {
        let now: Timestamp = "2026-07-30T10:00:00Z".parse().unwrap();
        let err = parse_live_usage(r#"{"totally":"different"}"#, now)
            .unwrap_err()
            .to_string();
        assert!(err.contains(r#"{"totally":"different"}"#), "got: {err}");
    }

    #[test]
    fn latest_snapshot_none_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(latest_snapshot(dir.path()).is_none());
        assert!(latest_snapshot(&dir.path().join("missing")).is_none());
    }
}
