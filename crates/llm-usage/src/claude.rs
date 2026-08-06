//! Claude Code: reads the OAuth token from `~/.claude/.credentials.json` and
//! queries the same endpoint the in-app `/usage` screen uses.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use jiff::Timestamp;
use serde::Deserialize;

use crate::report::{Report, Window};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct Credentials {
    pub access_token: String,
    pub subscription_type: Option<String>,
    pub rate_limit_tier: Option<String>,
    pub expires_at_ms: Option<i64>,
}

pub fn credentials_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home).join(".claude/.credentials.json");
    path.is_file().then_some(path)
}

pub fn parse_credentials(json: &str) -> Result<Credentials> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        claude_ai_oauth: Oauth,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Oauth {
        access_token: String,
        subscription_type: Option<String>,
        rate_limit_tier: Option<String>,
        expires_at: Option<i64>,
    }
    let file: File = serde_json::from_str(json)?;
    Ok(Credentials {
        access_token: file.claude_ai_oauth.access_token,
        subscription_type: file.claude_ai_oauth.subscription_type,
        rate_limit_tier: file.claude_ai_oauth.rate_limit_tier,
        expires_at_ms: file.claude_ai_oauth.expires_at,
    })
}

/// Turn an `oauth/usage` response body into limit windows.
pub fn parse_usage(json: &str) -> Result<Vec<Window>> {
    let usage: UsageResponse = serde_json::from_str(json)?;
    let mut windows: Vec<Window> = usage
        .limits
        .into_iter()
        .map(|l| Window {
            label: limit_label(&l),
            used_percent: l.percent,
            resets_at: l.resets_at,
        })
        .collect();
    if windows.is_empty() {
        for (limit, label) in [
            (usage.five_hour, "Session"),
            (usage.seven_day, "Week (all models)"),
        ] {
            if let Some(w) = limit {
                windows.push(Window {
                    label: label.into(),
                    used_percent: w.utilization,
                    resets_at: w.resets_at,
                });
            }
        }
    }
    if windows.is_empty() {
        return Err(anyhow!("usage response contained no limit windows"));
    }
    Ok(windows)
}

fn limit_label(limit: &Limit) -> String {
    let scope_model = limit
        .scope
        .as_ref()
        .and_then(|s| s.model.as_ref())
        .and_then(|m| m.display_name.as_deref());
    match (limit.kind.as_str(), scope_model) {
        ("session", _) => "Session".into(),
        ("weekly_all", _) => "Week (all models)".into(),
        ("weekly_scoped", Some(model)) => format!("Week ({model})"),
        ("weekly_scoped", None) => "Week (scoped)".into(),
        (kind, Some(model)) => format!("{kind} ({model})"),
        (kind, None) => kind.into(),
    }
}

pub fn report() -> Result<Report> {
    let path = credentials_path().context("no credentials file")?;
    let creds = parse_credentials(&std::fs::read_to_string(&path)?)
        .with_context(|| format!("parsing {}", path.display()))?;
    if let Some(ms) = creds.expires_at_ms
        && Timestamp::from_millisecond(ms).is_ok_and(|t| t < Timestamp::now())
    {
        return Err(anyhow!("OAuth token expired — open `claude` to refresh it"));
    }
    let body = fetch_usage(&creds.access_token)?;
    let detail = match (&creds.subscription_type, &creds.rate_limit_tier) {
        (Some(sub), Some(tier)) if tier != sub => Some(format!("{sub}, {tier}")),
        (Some(sub), _) => Some(sub.clone()),
        (None, tier) => tier.clone(),
    };
    Ok(Report {
        provider: "Claude Code".into(),
        detail,
        as_of: None,
        note: None,
        windows: parse_usage(&body)?,
    })
}

fn fetch_usage(token: &str) -> Result<String> {
    let response = ureq::get(USAGE_URL)
        .timeout(REQUEST_TIMEOUT)
        .set("Authorization", &format!("Bearer {token}"))
        .set("anthropic-beta", "oauth-2025-04-20")
        .call();
    match response {
        Ok(r) => Ok(r.into_string()?),
        Err(ureq::Error::Status(401, _)) => Err(anyhow!(
            "OAuth token rejected (401) — open `claude` to refresh it"
        )),
        Err(e) => Err(e.into()),
    }
}

#[derive(Deserialize)]
struct UsageResponse {
    #[serde(default)]
    limits: Vec<Limit>,
    five_hour: Option<LegacyWindow>,
    seven_day: Option<LegacyWindow>,
}

#[derive(Deserialize)]
struct Limit {
    kind: String,
    percent: f64,
    resets_at: Option<Timestamp>,
    scope: Option<Scope>,
}

#[derive(Deserialize)]
struct Scope {
    model: Option<ScopeModel>,
}

#[derive(Deserialize)]
struct ScopeModel {
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct LegacyWindow {
    utilization: f64,
    resets_at: Option<Timestamp>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREDENTIALS: &str = r#"{
      "claudeAiOauth": {
        "accessToken": "fake-access-token",
        "refreshToken": "fake-refresh-token",
        "expiresAt": 1785443932000,
        "scopes": ["user:inference", "user:profile"],
        "subscriptionType": "max",
        "rateLimitTier": "default_max_20x"
      }
    }"#;

    #[test]
    fn parses_credentials() {
        let creds = parse_credentials(CREDENTIALS).unwrap();
        assert_eq!(creds.access_token, "fake-access-token");
        assert_eq!(creds.subscription_type.as_deref(), Some("max"));
        assert_eq!(creds.rate_limit_tier.as_deref(), Some("default_max_20x"));
        assert_eq!(creds.expires_at_ms, Some(1785443932000));
    }

    #[test]
    fn rejects_credentials_without_token() {
        assert!(parse_credentials(r#"{"claudeAiOauth": {}}"#).is_err());
    }

    // Trimmed from a real response; extra unknown fields kept on purpose.
    const USAGE: &str = r#"{
      "five_hour": {"utilization": 59.0, "resets_at": "2026-07-30T18:10:00.954998+00:00", "limit_dollars": null},
      "seven_day": {"utilization": 41.0, "resets_at": "2026-07-31T12:00:00.955015+00:00"},
      "seven_day_opus": null,
      "extra_usage": {"is_enabled": false},
      "limits": [
        {"kind": "session", "group": "session", "percent": 59, "severity": "normal",
         "resets_at": "2026-07-30T18:10:00.954998+00:00", "scope": null, "is_active": false},
        {"kind": "weekly_all", "group": "weekly", "percent": 41, "severity": "normal",
         "resets_at": "2026-07-31T12:00:00.955015+00:00", "scope": null, "is_active": false},
        {"kind": "weekly_scoped", "group": "weekly", "percent": 73, "severity": "normal",
         "resets_at": "2026-07-31T12:00:00.955289+00:00",
         "scope": {"model": {"id": null, "display_name": "Fable"}, "surface": null}, "is_active": true}
      ],
      "spend": {"percent": 0}
    }"#;

    #[test]
    fn parses_usage_limits_array() {
        let windows = parse_usage(USAGE).unwrap();
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].label, "Session");
        assert_eq!(windows[0].used_percent, 59.0);
        assert_eq!(
            windows[0].resets_at.unwrap().to_string(),
            "2026-07-30T18:10:00.954998Z"
        );
        assert_eq!(windows[1].label, "Week (all models)");
        assert_eq!(windows[1].used_percent, 41.0);
        assert_eq!(windows[2].label, "Week (Fable)");
        assert_eq!(windows[2].used_percent, 73.0);
    }

    #[test]
    fn falls_back_to_legacy_windows_when_limits_missing() {
        let json = r#"{
          "five_hour": {"utilization": 12.5, "resets_at": "2026-07-30T18:10:00+00:00"},
          "seven_day": {"utilization": 34.0, "resets_at": "2026-07-31T12:00:00+00:00"}
        }"#;
        let windows = parse_usage(json).unwrap();
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].label, "Session");
        assert_eq!(windows[0].used_percent, 12.5);
        assert_eq!(windows[1].label, "Week (all models)");
    }

    #[test]
    fn unknown_limit_kinds_keep_their_name() {
        let json = r#"{"limits": [{"kind": "monthly_special", "percent": 5}]}"#;
        let windows = parse_usage(json).unwrap();
        assert_eq!(windows[0].label, "monthly_special");
        assert!(windows[0].resets_at.is_none());
    }
}
