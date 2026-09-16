mod analysis;
mod git;
mod snapshot;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Plain,
    Json,
}

#[derive(Parser)]
#[command(
    name = "cargo affected",
    version,
    about = "Find changed workspace packages and their transitive dependents"
)]
struct Args {
    /// Base Git revision; fetch this revision before invoking the command.
    #[arg(long)]
    base: String,
    /// Head Git revision. Uncommitted changes are not included.
    #[arg(long, default_value = "HEAD")]
    head: String,
    /// Compare revision tips instead of comparing head to their merge base.
    #[arg(long)]
    exact: bool,
    #[arg(long, value_enum, default_value = "plain")]
    format: Format,
    /// Print selection reasons to stderr.
    #[arg(long)]
    explain: bool,
    /// Do not allow Cargo to access the network.
    #[arg(long)]
    offline: bool,
    /// Workspace manifest (defaults to Cargo.toml in the current directory).
    #[arg(long, default_value = "Cargo.toml")]
    manifest_path: PathBuf,
}

fn run() -> Result<()> {
    let mut arguments: Vec<_> = std::env::args_os().collect();
    if arguments.get(1).is_some_and(|arg| arg == "affected") {
        arguments.remove(1);
    }
    let args = Args::parse_from(arguments);
    let manifest = std::fs::canonicalize(&args.manifest_path)
        .context("locating workspace manifest; use --manifest-path")?;
    let repo = git::Repository::open(manifest.parent().context("manifest has no parent")?)?;
    let relative_manifest = manifest.strip_prefix(&repo.root)?;
    let head = repo.resolve(&args.head)?;
    let base = repo.resolve(&args.base)?;
    let base = if args.exact {
        base
    } else {
        repo.merge_base(&base, &head)?
    };
    let changed = repo.changed(&base, &head)?;
    let head_dir = repo.export(&head)?;
    let base_dir = repo.export(&base)?;
    let head = snapshot::Snapshot::load(head_dir.path(), relative_manifest, args.offline)
        .context("head revision")?;
    let base = if base_dir.path().join(relative_manifest).exists() {
        snapshot::Snapshot::load(base_dir.path(), relative_manifest, args.offline)
            .context("base revision")?
    } else {
        snapshot::Snapshot::empty_like(&head)
    };
    let selected = analysis::analyze(&base, &head, &changed);
    if args.explain {
        for (package, reasons) in &selected {
            eprintln!(
                "{package}: {}",
                reasons.iter().cloned().collect::<Vec<_>>().join("; ")
            );
        }
    }
    match args.format {
        Format::Json => println!(
            "{}",
            serde_json::to_string(&selected.keys().collect::<Vec<_>>())?
        ),
        Format::Plain => {
            for package in selected.keys() {
                println!("{package}");
            }
        }
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
