use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::proc;

/// A secret key present in the local GnuPG keyring.
pub struct SecretKey {
    /// One keygrip per (sub)key; gpg-agent caches passphrases per keygrip.
    pub keygrips: Vec<String>,
}

/// Look up a secret key by fingerprint. Ok(None) means "not in keyring".
pub fn secret_key(fingerprint: &str) -> Result<Option<SecretKey>> {
    let out = Command::new("gpg")
        .args([
            "--batch",
            "--with-colons",
            "--with-keygrip",
            "--list-secret-keys",
            fingerprint,
        ])
        .output()
        .context("failed to run `gpg`")?;
    if out.status.success() {
        return Ok(parse_secret_key_listing(&String::from_utf8_lossy(
            &out.stdout,
        )));
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    if stderr.contains("No secret key") || stderr.contains("not found") {
        return Ok(None);
    }
    bail!("`gpg --list-secret-keys` failed: {}", stderr.trim());
}

pub fn parse_secret_key_listing(listing: &str) -> Option<SecretKey> {
    let mut seen_sec = false;
    let mut keygrips = Vec::new();
    for line in listing.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        match fields.first() {
            Some(&"sec") => seen_sec = true,
            Some(&"grp") if fields.len() > 9 => keygrips.push(fields[9].to_string()),
            _ => {}
        }
    }
    seen_sec.then_some(SecretKey { keygrips })
}

/// Import an ASCII-armored secret key into the keyring (idempotent)
/// and return the primary key's fingerprint as gpg reports it.
pub fn import(armored_key: &str) -> Result<String> {
    let mut cmd = Command::new("gpg");
    cmd.args(["--batch", "--quiet", "--status-fd=1", "--import"]);
    let out = proc::run_with_stdin(cmd, armored_key.as_bytes())?;
    if !out.status.success() {
        bail!(
            "`gpg --import` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    parse_import_status(&String::from_utf8_lossy(&out.stdout))
        .context("`gpg --import` succeeded but reported no imported key")
}

/// Fingerprint from `[GNUPG:] IMPORT_OK <flags> <fingerprint>` status
/// lines. gpg emits one per imported part (public, secret) and also for
/// keys it already had, always with the primary key's fingerprint.
pub fn parse_import_status(status: &str) -> Option<String> {
    status.lines().find_map(|line| {
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some("[GNUPG:]") || tokens.next() != Some("IMPORT_OK") {
            return None;
        }
        tokens.nth(1).map(str::to_string) // skip <flags>
    })
}

/// Keygrips whose passphrase gpg-agent currently has cached.
pub fn cached_keygrips() -> Result<HashSet<String>> {
    let out = Command::new("gpg-connect-agent")
        .args(["keyinfo --list", "/bye"])
        .output()
        .context("failed to run `gpg-connect-agent`")?;
    if !out.status.success() {
        bail!(
            "gpg-agent is unreachable: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(parse_keyinfo(&String::from_utf8_lossy(&out.stdout)))
}

/// Effective maximum cache TTL reported by gpgconf. This bounds entries
/// inserted by gpg-preset-passphrase.
pub fn max_cache_ttl() -> Result<u64> {
    let out = Command::new("gpgconf")
        .args(["--list-options", "gpg-agent"])
        .output()
        .context("failed to run `gpgconf`")?;
    if !out.status.success() {
        bail!(
            "`gpgconf --list-options gpg-agent` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    parse_max_cache_ttl(&String::from_utf8_lossy(&out.stdout))
        .context("`gpgconf` did not report gpg-agent's maximum cache TTL")
}

fn parse_max_cache_ttl(output: &str) -> Option<u64> {
    output.lines().find_map(|line| {
        let fields: Vec<&str> = line.split(':').collect();
        (fields.first() == Some(&"max-cache-ttl")).then(|| {
            // Field 10 is the configured value. If absent, field 8
            // contains gpgconf's built-in default.
            fields
                .get(9)
                .filter(|value| !value.is_empty())
                .or_else(|| fields.get(7))?
                .parse::<u64>()
                .ok()
        })?
    })
}

/// Parse `KEYINFO <keygrip> <type> <serialno> <idstr> <cached> ...` lines
/// as emitted by gpg-connect-agent (prefixed with `S `); the `cached`
/// column is `1` when the passphrase is in the agent's cache.
pub fn parse_keyinfo(output: &str) -> HashSet<String> {
    output
        .lines()
        .filter_map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            (tokens.len() > 6 && tokens[0] == "S" && tokens[1] == "KEYINFO" && tokens[6] == "1")
                .then(|| tokens[2].to_string())
        })
        .collect()
}

/// Cache a passphrase in gpg-agent for one keygrip.
///
/// Requires `allow-preset-passphrase` in gpg-agent.conf; the passphrase
/// travels via stdin only. Preset entries live until the agent's
/// `max-cache-ttl` expires or the agent restarts.
pub fn preset_passphrase(keygrip: &str, passphrase: &str) -> Result<()> {
    let tool = libexecdir()?.join("gpg-preset-passphrase");
    let mut cmd = Command::new(&tool);
    cmd.args(["--preset", keygrip]);
    let out = proc::run_with_stdin(cmd, passphrase.as_bytes())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let lower = stderr.to_lowercase();
        if lower.contains("not supported") || lower.contains("forbidden") {
            bail!(
                "gpg-agent refused the preset ({stderr}); add `allow-preset-passphrase` to \
                 ~/.gnupg/gpg-agent.conf and run `gpgconf --kill gpg-agent`"
            );
        }
        bail!("`gpg-preset-passphrase` failed: {stderr}");
    }
    Ok(())
}

fn libexecdir() -> Result<PathBuf> {
    let out = Command::new("gpgconf")
        .args(["--list-dirs", "libexecdir"])
        .output()
        .context("failed to run `gpgconf`")?;
    if !out.status.success() {
        bail!(
            "`gpgconf --list-dirs` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_secret_key_listing_with_keygrips() {
        let listing = "\
sec:u:255:22:0123456789ABCDEF:1600000000:::u:::scESC:::+:::ed25519:::0:
fpr:::::::::AAAA1111BBBB2222CCCC3333DDDD4444EEEE5555:
grp:::::::::1111111111111111111111111111111111111111:
uid:u::::1600000000::HASH::Test User <test@example.com>::::::::::0:
ssb:u:255:18:FEDCBA9876543210:1600000000::::::e:::+:::cv25519::
fpr:::::::::9999888877776666555544443333222211110000:
grp:::::::::2222222222222222222222222222222222222222:
";
        let key = parse_secret_key_listing(listing).unwrap();
        assert_eq!(
            key.keygrips,
            vec![
                "1111111111111111111111111111111111111111",
                "2222222222222222222222222222222222222222"
            ]
        );
    }

    #[test]
    fn empty_listing_means_no_key() {
        assert!(parse_secret_key_listing("").is_none());
    }

    #[test]
    fn parses_import_status_fingerprint() {
        let status = "\
[GNUPG:] KEY_CONSIDERED AAAA1111BBBB2222CCCC3333DDDD4444EEEE5555 0
[GNUPG:] IMPORT_OK 17 AAAA1111BBBB2222CCCC3333DDDD4444EEEE5555
[GNUPG:] IMPORT_RES 1 0 1 0 0 0 0 0 0 1 1 0 0 0 0
";
        assert_eq!(
            parse_import_status(status).as_deref(),
            Some("AAAA1111BBBB2222CCCC3333DDDD4444EEEE5555")
        );
        assert!(parse_import_status("gpg: key ABC: secret key imported\n").is_none());
    }

    #[test]
    fn parses_keyinfo_cached_flags() {
        let output = "\
S KEYINFO 1111111111111111111111111111111111111111 D - - 1 P - - -
S KEYINFO 2222222222222222222222222222222222222222 D - - - P - - -
OK
";
        let cached = parse_keyinfo(output);
        assert_eq!(cached.len(), 1);
        assert!(cached.contains("1111111111111111111111111111111111111111"));
    }

    #[test]
    fn parses_effective_max_cache_ttl() {
        let options = "\
default-cache-ttl:24:0:description:3:3:N:600::1800\n\
max-cache-ttl:24:2:description:3:3:N:7200::86400\n";
        assert_eq!(parse_max_cache_ttl(options), Some(86400));
    }

    #[test]
    fn max_cache_ttl_falls_back_to_gpg_default() {
        let options = "\
default-cache-ttl:24:0:description:3:3:N:600::\n\
max-cache-ttl:24:2:description:3:3:N:7200::\n";
        assert_eq!(parse_max_cache_ttl(options), Some(7200));
    }
}
