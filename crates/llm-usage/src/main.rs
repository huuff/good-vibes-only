mod claude;
mod codex;
mod render;
mod report;

use clap::Parser;

/// Detect installed LLM coding CLIs and print their usage limits.
#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Print reports as JSON instead of a table.
    #[arg(long)]
    json: bool,
}

/// Name, whether the CLI's config was found, and how to build its report.
type Provider = (&'static str, bool, fn() -> anyhow::Result<report::Report>);

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let providers: Vec<Provider> = vec![
        (
            "Claude Code",
            claude::credentials_path().is_some(),
            claude::report,
        ),
        ("Codex", codex::codex_home().is_some(), codex::report),
    ];

    let mut detected = 0;
    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for (name, found, run) in providers {
        if !found {
            errors.push((name, None));
            continue;
        }
        detected += 1;
        match run() {
            Ok(report) => reports.push(report),
            Err(err) => errors.push((name, Some(err))),
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        let mut first = true;
        for report in &reports {
            if !std::mem::take(&mut first) {
                println!();
            }
            render::print_report(report);
        }
        for (name, err) in &errors {
            if !std::mem::take(&mut first) {
                println!();
            }
            match err {
                Some(err) => render::print_error(name, err),
                None => render::print_not_detected(name),
            }
        }
    }

    if detected == 0 {
        anyhow::bail!("no LLM CLIs detected");
    }
    Ok(())
}
