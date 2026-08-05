use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::op::ItemSummary;

/// A key item mirrored from 1Password. Metadata only — ids, titles,
/// vault names and public fingerprints; secret material never enters
/// the cache.
#[derive(Serialize, Deserialize)]
pub struct Item {
    #[serde(flatten)]
    pub summary: ItemSummary,
    pub fingerprint: Option<String>,
}

/// No cache exists yet — `discover` has never run. Like
/// `op::OpUnavailable`, this is an expected state rather than a fault:
/// `main` maps it to exit code 2 so scripts can tell it apart from a
/// real error.
#[derive(Debug)]
pub struct NoCache;

impl fmt::Display for NoCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no local key cache yet; run `keyloader discover` to create it"
        )
    }
}

impl std::error::Error for NoCache {}

/// The local mirror of what `discover` saw in 1Password, letting
/// `status` and `load` skip the `op` roundtrip for listing.
#[derive(Serialize, Deserialize)]
pub struct Cache {
    /// Unix seconds when `discover` last refreshed the cache.
    pub updated_at: u64,
    pub ssh: Vec<Item>,
    pub gpg: Vec<Item>,
    /// Last time keyloader itself added each 1Password SSH item.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub ssh_loaded_at: HashMap<String, u64>,
    /// Last time keyloader itself preset each GPG keygrip, by item id.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub gpg_preset_at: HashMap<String, HashMap<String, u64>>,
}

impl Cache {
    pub fn new(ssh: Vec<Item>, gpg: Vec<Item>) -> Self {
        Cache {
            updated_at: now(),
            ssh,
            gpg,
            ssh_loaded_at: HashMap::new(),
            gpg_preset_at: HashMap::new(),
        }
    }

    /// Carry load timestamps through discovery only for unchanged
    /// 1Password items. An edit may represent key rotation.
    pub fn carry_tracking_from(&mut self, previous: &Self) {
        let unchanged = |item: &Item, old: &[Item]| {
            old.iter().any(|candidate| {
                candidate.summary.id == item.summary.id
                    && candidate.summary.version == item.summary.version
            })
        };
        for item in &self.ssh {
            if unchanged(item, &previous.ssh)
                && let Some(loaded_at) = previous.ssh_loaded_at.get(&item.summary.id)
            {
                self.ssh_loaded_at
                    .insert(item.summary.id.clone(), *loaded_at);
            }
        }
        for item in &self.gpg {
            if unchanged(item, &previous.gpg)
                && let Some(preset_at) = previous.gpg_preset_at.get(&item.summary.id)
            {
                self.gpg_preset_at
                    .insert(item.summary.id.clone(), preset_at.clone());
            }
        }
    }

    /// Human-readable cache age, e.g. "12m ago".
    pub fn age(&self) -> String {
        humanize(now().saturating_sub(self.updated_at))
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn humanize(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s ago"),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86399 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

pub fn human_duration(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        3600..=86399 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86400),
    }
}

fn path() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .context("cannot locate the cache directory: neither $XDG_CACHE_HOME nor $HOME is set")?;
    Ok(base.join("keyloader").join("items.json"))
}

/// Read the cache. Ok(None) means no `discover` has run yet.
pub fn load() -> Result<Option<Cache>> {
    let path = path()?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", path.display())),
    };
    let cache = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "failed to parse {}; re-run `keyloader discover` to rebuild it",
            path.display()
        )
    })?;
    Ok(Some(cache))
}

/// Write the cache atomically (temp file + rename), owner-readable
/// only. Returns the cache path for display.
pub fn store(cache: &Cache) -> Result<PathBuf> {
    let path = path()?;
    let dir = path.parent().expect("cache path has a parent");
    fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let json = serde_json::to_vec_pretty(cache).context("failed to serialize cache")?;
    let tmp = path.with_extension("json.tmp");
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .and_then(|mut file| file.write_all(&json))
        .and_then(|()| fs::rename(&tmp, &path))
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_through_json() {
        let mut cache = Cache::new(
            vec![Item {
                summary: serde_json::from_str(
                    r#"{"id":"abc","title":"key","vault":{"id":"v1","name":"Personal"}}"#,
                )
                .unwrap(),
                fingerprint: Some("SHA256:abc".into()),
            }],
            vec![],
        );
        cache.ssh_loaded_at.insert("abc".into(), 1234);
        let json = serde_json::to_string(&cache).unwrap();
        let back: Cache = serde_json::from_str(&json).unwrap();
        assert_eq!(back.ssh.len(), 1);
        assert_eq!(back.ssh[0].summary.id, "abc");
        assert_eq!(back.ssh[0].summary.vault.name, "Personal");
        assert_eq!(back.ssh[0].fingerprint.as_deref(), Some("SHA256:abc"));
        assert_eq!(back.ssh_loaded_at.get("abc"), Some(&1234));
        assert!(back.gpg.is_empty());
    }

    #[test]
    fn humanizes_ages() {
        assert_eq!(humanize(5), "5s ago");
        assert_eq!(humanize(90), "1m ago");
        assert_eq!(humanize(7200), "2h ago");
        assert_eq!(humanize(200_000), "2d ago");
    }

    #[test]
    fn humanizes_durations() {
        assert_eq!(human_duration(45), "45s");
        assert_eq!(human_duration(1800), "30m");
        assert_eq!(human_duration(7200), "2h");
        assert_eq!(human_duration(86400), "1d");
        assert_eq!(human_duration(90), "1m");
    }
}
