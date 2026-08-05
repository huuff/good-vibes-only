use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use zeroize::Zeroizing;

use crate::proc;

/// Human-readable retention policy of the agent addressed by
/// `SSH_AUTH_SOCK`. OpenSSH's agent protocol does not expose either
/// this default or a per-identity expiry time, so on Linux we recover
/// the command line of the process that owns the listening socket.
pub enum RetentionPolicy {
    Limited(u64),
    Forever,
    Unknown,
}

pub fn retention_policy() -> RetentionPolicy {
    let Some(args) = agent_command_line() else {
        return RetentionPolicy::Unknown;
    };
    match parse_agent_lifetime(&args) {
        Some(Some(lifetime)) => parse_duration(&lifetime)
            .map(RetentionPolicy::Limited)
            .unwrap_or(RetentionPolicy::Unknown),
        Some(None) => RetentionPolicy::Forever,
        None => RetentionPolicy::Unknown,
    }
}

fn parse_duration(value: &str) -> Option<u64> {
    let mut total = 0_u64;
    let mut digits = String::new();
    for character in value.chars() {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        let number = digits.parse::<u64>().ok()?;
        digits.clear();
        let multiplier = match character {
            's' | 'S' => 1,
            'm' | 'M' => 60,
            'h' | 'H' => 60 * 60,
            'd' | 'D' => 24 * 60 * 60,
            'w' | 'W' => 7 * 24 * 60 * 60,
            _ => return None,
        };
        total = total.checked_add(number.checked_mul(multiplier)?)?;
    }
    if !digits.is_empty() {
        total = total.checked_add(digits.parse().ok()?)?;
    }
    (total > 0).then_some(total)
}

/// `Some(None)` means this is OpenSSH ssh-agent with no default
/// lifetime; `None` means the socket belongs to some other/unknown
/// agent implementation.
fn parse_agent_lifetime(args: &[String]) -> Option<Option<String>> {
    let executable = Path::new(args.first()?).file_name()?;
    if executable != OsStr::new("ssh-agent") {
        return None;
    }
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "-t" {
            return args.next().cloned().map(Some);
        }
        if let Some(lifetime) = arg.strip_prefix("-t")
            && !lifetime.is_empty()
        {
            return Some(Some(lifetime.to_string()));
        }
    }
    Some(None)
}

fn agent_command_line() -> Option<Vec<String>> {
    let socket = std::env::var_os("SSH_AUTH_SOCK")?;
    let inode = fs::metadata(socket).ok()?.ino();
    let target = format!("socket:[{inode}]");

    for process in fs::read_dir("/proc").ok()?.flatten() {
        if !process
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let Ok(fds) = fs::read_dir(process.path().join("fd")) else {
            continue;
        };
        if !fds
            .flatten()
            .any(|fd| fs::read_link(fd.path()).is_ok_and(|link| link == Path::new(&target)))
        {
            continue;
        }
        let bytes = fs::read(process.path().join("cmdline")).ok()?;
        return Some(
            bytes
                .split(|byte| *byte == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect(),
        );
    }
    systemd_agent_command_line()
}

/// Sandboxes and hardened `/proc` mounts may hide another process's
/// file descriptors. For the conventional systemd user unit, its
/// effective (already evaluated) command is an equivalent read-only
/// source. Only accept it when `-a` names our current agent socket.
fn systemd_agent_command_line() -> Option<Vec<String>> {
    let out = Command::new("systemctl")
        .args([
            "--user",
            "show",
            "ssh-agent.service",
            "--property=ExecStart",
            "--value",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&out.stdout);
    let command = output.split("argv[]=").nth(1)?.split(" ;").next()?;
    let args: Vec<String> = command.split_whitespace().map(str::to_string).collect();
    let socket = std::env::var_os("SSH_AUTH_SOCK")?;
    agent_socket(&args)
        .is_some_and(|path| Path::new(path) == Path::new(&socket))
        .then_some(args)
}

fn agent_socket(args: &[String]) -> Option<&str> {
    let mut args = args.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "-a" {
            return args.next().map(String::as_str);
        }
        if let Some(path) = arg.strip_prefix("-a")
            && !path.is_empty()
        {
            return Some(path);
        }
    }
    None
}

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

    #[test]
    fn parses_agent_default_lifetime() {
        let args = strings(&["/usr/bin/ssh-agent", "-t", "1h", "-a", "/tmp/agent"]);
        assert_eq!(parse_agent_lifetime(&args), Some(Some("1h".into())));

        let args = strings(&["ssh-agent", "-D", "-t24h"]);
        assert_eq!(parse_agent_lifetime(&args), Some(Some("24h".into())));

        let args = strings(&["ssh-agent", "-D"]);
        assert_eq!(parse_agent_lifetime(&args), Some(None));

        let args = strings(&["gpg-agent", "--enable-ssh-support"]);
        assert_eq!(parse_agent_lifetime(&args), None);
    }

    #[test]
    fn parses_agent_socket() {
        let args = strings(&["ssh-agent", "-t", "1h", "-a", "/tmp/agent"]);
        assert_eq!(agent_socket(&args), Some("/tmp/agent"));

        let args = strings(&["ssh-agent", "-a/tmp/agent"]);
        assert_eq!(agent_socket(&args), Some("/tmp/agent"));
    }

    #[test]
    fn parses_openssh_durations() {
        assert_eq!(parse_duration("1h"), Some(3600));
        assert_eq!(parse_duration("1h30m"), Some(5400));
        assert_eq!(parse_duration("90"), Some(90));
        assert_eq!(parse_duration("2d"), Some(172800));
        assert_eq!(parse_duration("forever"), None);
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }
}
