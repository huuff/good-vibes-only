use std::fmt;
use std::io::IsTerminal;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use linux_keyutils::{KeyRing, KeyRingIdentifier};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Items carrying a GPG secret key are discovered by this tag.
pub const GPG_TAG: &str = "keyloader/gpg";
/// SSH keys are discovered by 1Password's native category.
pub const SSH_CATEGORY: &str = "SSH Key";

/// Field labels keyloader expects on items (case-insensitive).
pub const FIELD_FINGERPRINT: &str = "fingerprint";
pub const FIELD_SECRET_KEY: &str = "secret key";
pub const FIELD_PASSPHRASE: &str = "passphrase";

/// 1Password is installed but cannot serve us right now (locked, signed
/// out, desktop app unreachable). Callers treat this as "fail soft":
/// `main` maps it to exit code 2 so scripts can ignore it quietly.
#[derive(Debug)]
pub struct OpUnavailable(String);

impl fmt::Display for OpUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "1Password is not available: {}", self.0)
    }
}

impl std::error::Error for OpUnavailable {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ItemSummary {
    pub id: String,
    pub title: String,
    pub vault: Vault,
    /// 1Password bumps this on every edit; `discover` uses it to drop
    /// learned fingerprints for items that changed (possibly rotated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Vault {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct ItemDetail {
    #[serde(default)]
    pub fields: Vec<Field>,
}

#[derive(Debug, Deserialize)]
pub struct Field {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub value: Option<String>,
}

impl ItemDetail {
    pub fn field(&self, label: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.label.eq_ignore_ascii_case(label))
            .and_then(|f| f.value.as_deref())
    }
}

/// Session obtained by an in-process `op signin`: the `OP_SESSION_*`
/// variable name and its token. `None` means we already tried (or
/// couldn't ask) — signin is attempted at most once per run.
static SESSION: OnceLock<Option<(String, Zeroizing<String>)>> = OnceLock::new();

fn invoke(args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = Command::new("op");
    cmd.args(args);
    if let Some(Some((var, token))) = SESSION.get() {
        cmd.env(var, token.as_str());
    }
    cmd.output()
        .context("failed to run `op`; is the 1Password CLI installed?")
}

fn run(args: &[&str]) -> Result<Vec<u8>> {
    let mut out = invoke(args)?;
    // A missing/expired session is fixable: reuse a cached session from
    // the kernel keyring, or sign in on the user's terminal, then retry
    // once. Tokens live only in kernel/process memory and reach `op`
    // via the environment — never argv or the filesystem.
    let mut signed_out = false;
    if !out.status.success() && SESSION.get().is_none() && !whoami_ok() {
        if establish_session() {
            out = invoke(args)?;
        } else {
            signed_out = true;
        }
    }
    if out.status.success() {
        return Ok(out.stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if signed_out {
        // whoami confirmed there is no session and we couldn't get one.
        return Err(OpUnavailable(stderr).into());
    }
    let lower = stderr.to_lowercase();
    const UNAVAILABLE: &[&str] = &[
        "signed in",
        "no account",
        "locked",
        "session expired",
        "authorization prompt dismissed",
        "connecting to desktop app",
    ];
    if UNAVAILABLE.iter().any(|pat| lower.contains(pat)) {
        return Err(OpUnavailable(stderr).into());
    }
    bail!("`op {}` failed: {stderr}", args.join(" "));
}

/// A failed command is worth retrying after re-establishing a session
/// only when the session itself is the problem. Rather than
/// pattern-match `op`'s error text (which varies across versions), ask
/// `op whoami`: it fails exactly when there is no usable session.
fn whoami_ok() -> bool {
    matches!(invoke(&["whoami"]), Ok(out) if out.status.success())
}

/// Get a session and cache it in `SESSION`, trying at most once per
/// run: first a still-valid token from the kernel keyring (works even
/// without a terminal), then an interactive `op signin`. Returns
/// whether a session was established.
fn establish_session() -> bool {
    let session = keyring_load()
        .filter(|(var, token)| {
            let valid = matches!(
                Command::new("op")
                    .arg("whoami")
                    .env(var, token.as_str())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status(),
                Ok(status) if status.success()
            );
            if valid {
                keyring_store(var, token); // restart the expiry clock
            }
            valid
        })
        .or_else(signin);
    let established = session.is_some();
    let _ = SESSION.set(session);
    established
}

/// Interactively sign in to 1Password, caching the session token in the
/// kernel keyring for later runs. Never asks without a terminal, so
/// scripted invocations keep failing soft with exit code 2.
fn signin() -> Option<(String, Zeroizing<String>)> {
    if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
        return None;
    }
    eprintln!("keyloader: not signed in to 1Password, running `op signin`");
    // stdin/stderr go to the terminal for the password prompt; stdout is
    // captured because that's where `op` prints the session export line.
    let out = Command::new("op")
        .arg("signin")
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .ok()
        .filter(|out| out.status.success())?;
    let stdout = Zeroizing::new(String::from_utf8_lossy(&out.stdout).into_owned());
    let (var, token) = parse_session(&stdout)?;
    keyring_store(&var, &token);
    Some((var, token))
}

/// The cached session lives in the kernel *session* keyring: in kernel
/// memory only (never on disk), possessed only by processes in this
/// login session (other sessions of the same user get view-only, so
/// they cannot read the token), and expired by the kernel on the same
/// 30-minute idle clock `op` itself uses.
const KEYRING_DESC: &str = "keyloader:op-session";
const SESSION_TTL_SECS: usize = 30 * 60;

fn keyring() -> Option<KeyRing> {
    KeyRing::from_special_id(KeyRingIdentifier::Session, false).ok()
}

fn keyring_load() -> Option<(String, Zeroizing<String>)> {
    let key = keyring()?.search(KEYRING_DESC).ok()?;
    let payload = Zeroizing::new(key.read_to_vec().ok()?);
    let text = std::str::from_utf8(&payload).ok()?;
    let (var, token) = text.split_once('=')?;
    if token.is_empty() {
        return None;
    }
    Some((var.to_string(), Zeroizing::new(token.to_string())))
}

fn keyring_store(var: &str, token: &str) {
    let Some(ring) = keyring() else { return };
    let payload = Zeroizing::new(format!("{var}={token}"));
    // add_key replaces an existing key with the same description.
    if let Ok(key) = ring.add_key(KEYRING_DESC, payload.as_bytes()) {
        let _ = key.set_timeout(SESSION_TTL_SECS);
    }
}

/// Extract the `OP_SESSION_<account>` variable and token from `op
/// signin` output, which is shell code like
/// `export OP_SESSION_my="…"` (or fish's `set -gx OP_SESSION_my "…"`).
fn parse_session(stdout: &str) -> Option<(String, Zeroizing<String>)> {
    let rest = &stdout[stdout.find("OP_SESSION_")?..];
    let name_len = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let (name, rest) = rest.split_at(name_len);
    let rest = &rest[rest.find('"')? + 1..];
    let token = &rest[..rest.find('"')?];
    if token.is_empty() {
        return None;
    }
    Some((name.to_string(), Zeroizing::new(token.to_string())))
}

pub fn list_ssh_items() -> Result<Vec<ItemSummary>> {
    let out = run(&[
        "item",
        "list",
        "--categories",
        SSH_CATEGORY,
        "--format",
        "json",
    ])?;
    serde_json::from_slice(&out).context("failed to parse `op item list` output")
}

pub fn list_gpg_items() -> Result<Vec<ItemSummary>> {
    let out = run(&["item", "list", "--tags", GPG_TAG, "--format", "json"])?;
    serde_json::from_slice(&out).context("failed to parse `op item list` output")
}

pub fn get_item(id: &str) -> Result<ItemDetail> {
    let out = run(&["item", "get", id, "--format", "json"])?;
    serde_json::from_slice(&out).context("failed to parse `op item get` output")
}

/// Fetch a single (possibly concealed) field value, or None if the item
/// has no such field.
pub fn reveal_field(item_id: &str, label: &str) -> Result<Option<Zeroizing<String>>> {
    let field_arg = format!("label={label}");
    let out = match run(&[
        "item", "get", item_id, "--fields", &field_arg, "--reveal", "--format", "json",
    ]) {
        Ok(out) => out,
        Err(err) => {
            if format!("{err:#}").to_lowercase().contains("isn't a field") {
                return Ok(None);
            }
            return Err(err);
        }
    };
    let value: serde_json::Value =
        serde_json::from_slice(&out).context("failed to parse `op item get --fields` output")?;
    // op returns a single field as an object and multiple matches as an array.
    let field = match value {
        serde_json::Value::Array(fields) => fields.into_iter().next(),
        other => Some(other),
    };
    Ok(field
        .as_ref()
        .and_then(|f| f.get("value"))
        .and_then(|v| v.as_str())
        .map(|s| Zeroizing::new(s.to_string())))
}

/// Fetch an SSH private key in OpenSSH format, ready for `ssh-add -`.
pub fn read_ssh_private_key(vault_id: &str, item_id: &str) -> Result<Zeroizing<String>> {
    let reference = format!("op://{vault_id}/{item_id}/private key?ssh-format=openssh");
    let out = run(&["read", &reference])?;
    let key = String::from_utf8(out).context("private key is not valid UTF-8")?;
    Ok(Zeroizing::new(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_item_list() {
        let json = r#"[
          {"id":"abc123","title":"my ssh key","version":3,
           "vault":{"id":"v1","name":"Personal"},
           "category":"SSH_KEY","last_edited_by":"u1"}
        ]"#;
        let items: Vec<ItemSummary> = serde_json::from_str(json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "my ssh key");
        assert_eq!(items[0].vault.id, "v1");
    }

    #[test]
    fn parses_signin_output() {
        let posix = "export OP_SESSION_myaccount=\"abc123-_token\"\n";
        let (name, token) = parse_session(posix).unwrap();
        assert_eq!(name, "OP_SESSION_myaccount");
        assert_eq!(token.as_str(), "abc123-_token");

        let fish = "set -gx OP_SESSION_my \"tok\"\n";
        let (name, token) = parse_session(fish).unwrap();
        assert_eq!(name, "OP_SESSION_my");
        assert_eq!(token.as_str(), "tok");

        assert!(parse_session("no session here").is_none());
        assert!(parse_session("export OP_SESSION_x=\"\"").is_none());
    }

    #[test]
    fn finds_fields_case_insensitively() {
        let json = r#"{
          "id":"abc123",
          "fields":[
            {"id":"f1","type":"STRING","label":"Fingerprint","value":"SHA256:abc"},
            {"id":"f2","type":"CONCEALED","label":"passphrase"}
          ]
        }"#;
        let detail: ItemDetail = serde_json::from_str(json).unwrap();
        assert_eq!(detail.field("fingerprint"), Some("SHA256:abc"));
        assert_eq!(detail.field("passphrase"), None); // concealed: no value without --reveal
        assert_eq!(detail.field("missing"), None);
    }
}
