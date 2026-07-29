mod cache;
mod gpg;
mod op;
mod proc;
mod ssh;

use std::collections::{HashMap, HashSet};
use std::process::ExitCode;

use anyhow::{Context, Error, Result, bail};
use clap::{Parser, Subcommand};
use colored::Colorize;

#[derive(Parser)]
#[command(
    name = "keyloader",
    version,
    about = "Load GPG and SSH keys from 1Password into gpg-agent and ssh-agent"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the key items in 1Password and refresh the local cache
    Discover,
    /// Show which cached keys are usable locally (no 1Password access)
    Status,
    /// Import/add missing keys into gpg-agent and ssh-agent
    Load {
        /// Print what would happen without changing anything
        #[arg(long)]
        dry_run: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Discover => discover(),
        Command::Status => status(),
        Command::Load { dry_run } => load(dry_run),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        // Locked/signed-out 1Password and a not-yet-created cache are
        // expected states, not faults: exit 2 so scripts can tell
        // them apart from real errors.
        Err(err) if is_soft(&err) => {
            eprintln!("{} {err}", "keyloader:".yellow());
            ExitCode::from(2)
        }
        Err(err) => {
            eprintln!("{} {err:#}", "keyloader: error:".red().bold());
            ExitCode::FAILURE
        }
    }
}

fn is_soft(err: &Error) -> bool {
    err.downcast_ref::<op::OpUnavailable>().is_some()
        || err.downcast_ref::<cache::NoCache>().is_some()
}

fn header(title: &str, source: &str) {
    println!("{}  {}", title.bold(), format!("· {source}").dimmed());
}

fn none_line(what: &str) {
    println!("  {}", format!("({what})").dimmed());
}

/// The cache `discover` maintains; `status` and `load` refuse to guess
/// without it rather than silently falling back to a 1Password roundtrip.
fn load_cache() -> Result<cache::Cache> {
    cache::load()?.ok_or_else(|| cache::NoCache.into())
}

/// Per-section column widths so vault and detail columns line up.
struct Columns {
    title: usize,
    vault: usize,
}

impl Columns {
    fn of(items: &[cache::Item]) -> Self {
        let max = |f: fn(&cache::Item) -> &str| {
            items
                .iter()
                .map(|i| f(i).chars().count())
                .max()
                .unwrap_or(0)
        };
        Columns {
            title: max(|i| &i.summary.title),
            vault: max(|i| &i.summary.vault.name),
        }
    }

    /// `  <glyph> <title> (<vault>)  <rest>`, padded to this section's
    /// column widths. Padding is applied outside the color codes:
    /// `{:width$}` would count the invisible ANSI escapes.
    fn line(&self, glyph: colored::ColoredString, item: &cache::Item, rest: &str) {
        let title = &item.summary.title;
        let vault = &item.summary.vault.name;
        let tpad = " ".repeat(self.title.saturating_sub(title.chars().count()));
        let vpad = " ".repeat(self.vault.saturating_sub(vault.chars().count()));
        println!(
            "  {glyph} {}{tpad}  {}{vpad}  {rest}",
            title.bold(),
            format!("({vault})").dimmed()
        );
    }
}

fn discover() -> Result<()> {
    // Fingerprints learned by `load` exist only in the local cache;
    // carry them into the refreshed cache — except when the item was
    // edited in 1Password since (version bump), which may mean a
    // rotated key, so the next `load` re-imports and re-learns.
    let learned: HashMap<String, (Option<u64>, String)> = cache::load()
        .ok()
        .flatten()
        .map(|old| {
            old.gpg
                .into_iter()
                .filter_map(|item| {
                    Some((item.summary.id, (item.summary.version, item.fingerprint?)))
                })
                .collect()
        })
        .unwrap_or_default();

    let ssh_items = fetch_items(op::list_ssh_items()?)?;
    header(
        "SSH keys",
        &format!("1Password category \"{}\"", op::SSH_CATEGORY),
    );
    discover_section(&ssh_items);

    println!();
    let mut gpg_items = fetch_items(op::list_gpg_items()?)?;
    for item in &mut gpg_items {
        if item.fingerprint.is_none()
            && let Some((version, fingerprint)) = learned.get(&item.summary.id)
            && *version == item.summary.version
        {
            item.fingerprint = Some(fingerprint.clone());
        }
    }
    header("GPG keys", &format!("1Password tag \"{}\"", op::GPG_TAG));
    discover_section(&gpg_items);

    let path = cache::store(&cache::Cache::new(ssh_items, gpg_items))?;
    println!();
    println!("{}", format!("cached to {}", path.display()).dimmed());
    Ok(())
}

/// Resolve item summaries into cacheable entries by fetching each
/// item's fingerprint from 1Password.
fn fetch_items(items: Vec<op::ItemSummary>) -> Result<Vec<cache::Item>> {
    items
        .into_iter()
        .map(|summary| {
            let detail = op::get_item(&summary.id)?;
            Ok(cache::Item {
                fingerprint: detail.field(op::FIELD_FINGERPRINT).map(str::to_string),
                summary,
            })
        })
        .collect()
}

fn discover_section(items: &[cache::Item]) {
    if items.is_empty() {
        none_line("none");
        return;
    }
    let columns = Columns::of(items);
    for item in items {
        let fpr = item.fingerprint.as_deref().unwrap_or("-");
        columns.line("•".cyan(), item, &fpr.dimmed().to_string());
    }
}

fn status() -> Result<()> {
    let cached_items = load_cache()?;
    let age = cached_items.age();

    let loaded = ssh::loaded_fingerprints()?;
    header("SSH keys", &format!("ssh-agent · discovered {age}"));
    if cached_items.ssh.is_empty() {
        none_line("none in 1Password");
    }
    let columns = Columns::of(&cached_items.ssh);
    for item in &cached_items.ssh {
        let (glyph, state) = match item.fingerprint.as_deref() {
            Some(fpr) if loaded.contains(fpr) => ("✓".green(), "loaded in ssh-agent"),
            Some(_) => ("✗".yellow(), "not loaded"),
            None => ("?".yellow(), "unknown (item has no fingerprint field)"),
        };
        columns.line(glyph, item, state);
    }

    println!();
    let cached_grips = gpg::cached_keygrips()?;
    header("GPG keys", &format!("gpg-agent · discovered {age}"));
    if cached_items.gpg.is_empty() {
        none_line("none in 1Password");
    }
    let columns = Columns::of(&cached_items.gpg);
    for item in &cached_items.gpg {
        let (glyph, state) = match item.fingerprint.as_deref() {
            None => (
                "?".yellow(),
                "unknown; `keyloader load` will learn its fingerprint".to_string(),
            ),
            Some(fpr) => match gpg::secret_key(fpr)? {
                None => ("✗".yellow(), "not in keyring".to_string()),
                Some(key) => {
                    let hot = key
                        .keygrips
                        .iter()
                        .filter(|grip| cached_grips.contains(*grip))
                        .count();
                    let total = key.keygrips.len();
                    let glyph = if hot == total {
                        "✓".green()
                    } else {
                        "◐".yellow()
                    };
                    (
                        glyph,
                        format!("in keyring, passphrase cached for {hot}/{total} keygrip(s)"),
                    )
                }
            },
        };
        columns.line(glyph, item, &state);
    }
    Ok(())
}

fn load(dry_run: bool) -> Result<()> {
    let mut cached_items = load_cache()?;
    let mut failures = 0;

    // `→` marks lines that only describe what would happen.
    let ok = if dry_run { "→".cyan() } else { "✓".green() };

    let loaded = ssh::loaded_fingerprints()?;
    for item in &cached_items.ssh {
        match load_ssh_item(item, &loaded, dry_run) {
            Ok(msg) => println!(
                "  {ok} {} {} — {msg}",
                "ssh".dimmed(),
                item.summary.title.bold()
            ),
            Err(err) => {
                eprintln!(
                    "  {} {} {} — {err:#}",
                    "✗".red(),
                    "ssh".dimmed(),
                    item.summary.title.bold()
                );
                failures += 1;
            }
        }
    }

    let cached_grips = gpg::cached_keygrips()?;
    let mut learned = false;
    for item in &mut cached_items.gpg {
        let had_fingerprint = item.fingerprint.is_some();
        match load_gpg_item(item, &cached_grips, dry_run) {
            Ok(msg) => println!(
                "  {ok} {} {} — {msg}",
                "gpg".dimmed(),
                item.summary.title.bold()
            ),
            Err(err) => {
                eprintln!(
                    "  {} {} {} — {err:#}",
                    "✗".red(),
                    "gpg".dimmed(),
                    item.summary.title.bold()
                );
                failures += 1;
            }
        }
        learned |= !had_fingerprint && item.fingerprint.is_some();
    }
    if learned {
        cache::store(&cached_items)?;
    }

    if failures > 0 {
        bail!(
            "{failures} key(s) failed to load (if a key changed in 1Password, re-run `keyloader discover`)"
        );
    }
    Ok(())
}

fn load_ssh_item(
    item: &cache::Item,
    loaded: &HashSet<String>,
    dry_run: bool,
) -> Result<&'static str> {
    if let Some(fpr) = &item.fingerprint
        && loaded.contains(fpr)
    {
        return Ok("already loaded");
    }
    if dry_run {
        return Ok("would add to ssh-agent");
    }
    let key = op::read_ssh_private_key(&item.summary.vault.id, &item.summary.id)?;
    ssh::add_key(&key)?;
    Ok("added to ssh-agent")
}

fn load_gpg_item(
    item: &mut cache::Item,
    cached_grips: &HashSet<String>,
    dry_run: bool,
) -> Result<String> {
    let known = item
        .fingerprint
        .as_deref()
        .map(gpg::secret_key)
        .transpose()?
        .flatten();

    let (key, mut done) = match known {
        Some(key) => (Some(key), "already in keyring".to_string()),
        None if dry_run => return Ok("would import into keyring".to_string()),
        None => {
            let armored =
                op::reveal_field(&item.summary.id, op::FIELD_SECRET_KEY)?.with_context(|| {
                    format!("item has no `{}` field to import", op::FIELD_SECRET_KEY)
                })?;
            let imported = gpg::import(&armored)?;
            let done = match &item.fingerprint {
                Some(declared) if !declared.eq_ignore_ascii_case(&imported) => bail!(
                    "imported key {imported} but the item's `{}` field says {declared}; \
                     fix or remove the field, then re-run `keyloader discover`",
                    op::FIELD_FINGERPRINT
                ),
                Some(_) => "imported into keyring".to_string(),
                None => {
                    item.fingerprint = Some(imported.clone());
                    format!("imported into keyring, learned fingerprint {imported}")
                }
            };
            // Re-list to learn the imported key's keygrips for presetting.
            (gpg::secret_key(&imported)?, done)
        }
    };

    let Some(key) = key.filter(|k| !k.keygrips.is_empty()) else {
        return Ok(done);
    };
    let uncached: Vec<&String> = key
        .keygrips
        .iter()
        .filter(|grip| !cached_grips.contains(*grip))
        .collect();
    if uncached.is_empty() {
        done.push_str(", passphrase already cached");
        return Ok(done);
    }
    let Some(passphrase) = op::reveal_field(&item.summary.id, op::FIELD_PASSPHRASE)? else {
        done.push_str(", no `passphrase` field so nothing preset in gpg-agent");
        return Ok(done);
    };
    if dry_run {
        done.push_str(&format!(
            ", would preset passphrase for {} keygrip(s)",
            uncached.len()
        ));
        return Ok(done);
    }
    for grip in &uncached {
        gpg::preset_passphrase(grip, &passphrase)?;
    }
    done.push_str(&format!(
        ", passphrase preset for {} keygrip(s)",
        uncached.len()
    ));
    Ok(done)
}
