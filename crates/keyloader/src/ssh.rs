use std::collections::HashSet;
use std::process::Command;

use anyhow::{Context, Result, bail};
use zeroize::Zeroizing;

use crate::proc;

/// SHA256 fingerprints of the keys currently loaded in ssh-agent.
pub fn loaded_fingerprints() -> Result<HashSet<String>> {
    let out = Command::new("ssh-add")
        .arg("-l")
        .output()
        .context("failed to run `ssh-add`")?;
    match out.status.code() {
        Some(0) => Ok(parse_listing(&String::from_utf8_lossy(&out.stdout))),
        Some(1) => Ok(HashSet::new()), // "The agent has no identities."
        _ => bail!(
            "ssh-agent is unreachable: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
    }
}

pub fn parse_listing(listing: &str) -> HashSet<String> {
    listing
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter(|token| token.starts_with("SHA256:"))
        .map(str::to_string)
        .collect()
}

/// Pipe a private key into `ssh-add -`.
///
/// SSH_ASKPASS_REQUIRE=never keeps a passphrase-protected key from
/// hanging on a GUI prompt; it fails instead, with a hint. 1Password is
/// the encryption at rest, so keys are expected to be stored unencrypted.
pub fn add_key(private_key: &str) -> Result<()> {
    let mut input = Zeroizing::new(private_key.to_string());
    if !input.ends_with('\n') {
        input.push('\n');
    }
    let mut cmd = Command::new("ssh-add");
    cmd.arg("-").env("SSH_ASKPASS_REQUIRE", "never");
    let out = proc::run_with_stdin(cmd, input.as_bytes())?;
    if !out.status.success() {
        bail!(
            "`ssh-add` failed: {} (passphrase-protected keys are not supported; store the key unencrypted in 1Password)",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ssh_add_listing() {
        let listing = "\
3072 SHA256:TMAmOXDk6rvlD08LtGBEe4meMuQ/kI+0TCxAvteLn8M haf@nixos (RSA)
256 SHA256:W6Bx19O4VvM0c9DgY3gW3ZHbh/vnXH0ecgzdjqXsIYk haf@protonmail.ch (ED25519)
";
        let fingerprints = parse_listing(listing);
        assert_eq!(fingerprints.len(), 2);
        assert!(fingerprints.contains("SHA256:TMAmOXDk6rvlD08LtGBEe4meMuQ/kI+0TCxAvteLn8M"));
    }

    #[test]
    fn ignores_noise_lines() {
        assert!(parse_listing("The agent has no identities.\n").is_empty());
        assert!(parse_listing("").is_empty());
    }
}
