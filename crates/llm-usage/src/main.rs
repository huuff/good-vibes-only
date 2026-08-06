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

/// Run detected providers concurrently while retaining their display order.
fn run_providers(
    providers: Vec<Provider>,
) -> Vec<(&'static str, Option<anyhow::Result<report::Report>>)> {
    let handles: Vec<_> = providers
        .into_iter()
        .map(|(name, found, run)| {
            let handle = found.then(|| std::thread::spawn(run));
            (name, handle)
        })
        .collect();

    handles
        .into_iter()
        .map(|(name, handle)| {
            let result = handle.map(|handle| {
                handle
                    .join()
                    .map_err(|_| anyhow::anyhow!("provider worker panicked"))?
            });
            (name, result)
        })
        .collect()
}

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
    for (name, result) in run_providers(providers) {
        match result {
            None => errors.push((name, None)),
            Some(Ok(report)) => {
                detected += 1;
                reports.push(report);
            }
            Some(Err(err)) => {
                detected += 1;
                errors.push((name, Some(err)));
            }
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    static ACTIVE: AtomicUsize = AtomicUsize::new(0);
    static PEAK_ACTIVE: AtomicUsize = AtomicUsize::new(0);

    fn tracked_report() -> anyhow::Result<report::Report> {
        let active = ACTIVE.fetch_add(1, Ordering::SeqCst) + 1;
        PEAK_ACTIVE.fetch_max(active, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(100));
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
        Ok(report::Report {
            provider: "test".into(),
            detail: None,
            as_of: None,
            note: None,
            windows: Vec::new(),
        })
    }

    #[test]
    fn providers_run_concurrently_and_keep_order() {
        ACTIVE.store(0, Ordering::SeqCst);
        PEAK_ACTIVE.store(0, Ordering::SeqCst);

        let results = run_providers(vec![
            ("first", true, tracked_report),
            ("missing", false, tracked_report),
            ("second", true, tracked_report),
        ]);

        assert_eq!(PEAK_ACTIVE.load(Ordering::SeqCst), 2);
        assert_eq!(results[0].0, "first");
        assert_eq!(results[1].0, "missing");
        assert!(results[1].1.is_none());
        assert_eq!(results[2].0, "second");
    }
}
