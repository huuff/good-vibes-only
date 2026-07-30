//! Terminal output: one block per provider, one line per limit window.

use colored::Colorize;
use jiff::Timestamp;

use crate::report::Report;

const BAR_WIDTH: usize = 20;

/// `bar(59.0, 10)` → `██████░░░░`. Percent is clamped to 0..=100.
pub fn bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0).clamp(0.0, 1.0) * width as f64).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// Human duration until `at`, e.g. "2h 13m", "45m", "3d 4h". Never negative.
pub fn until(now: Timestamp, at: Timestamp) -> String {
    let minutes = (at.as_second() - now.as_second()).max(0) / 60;
    let (days, hours, minutes) = (minutes / 1440, minutes % 1440 / 60, minutes % 60);
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

pub fn print_report(report: &Report) {
    let now = Timestamp::now();
    let mut heading = report.provider.bold().to_string();
    if let Some(detail) = &report.detail {
        heading.push_str(&format!(" {}", format!("({detail})").dimmed()));
    }
    if let Some(at) = report.as_of {
        let ago = until(at, now);
        heading.push_str(&format!(" {}", format!("— as of {ago} ago").dimmed()));
    }
    println!("{heading}");
    if let Some(note) = &report.note {
        println!("  {}", note.dimmed());
    }
    let label_width = report
        .windows
        .iter()
        .map(|w| w.label.len())
        .max()
        .unwrap_or(0);
    for w in &report.windows {
        let pct = format!("{:>3.0}%", w.used_percent);
        let pct = match w.used_percent {
            p if p >= 90.0 => pct.red().bold(),
            p if p >= 70.0 => pct.yellow(),
            _ => pct.green(),
        };
        let reset = match w.resets_at {
            Some(at) => format!("resets in {}", until(now, at)).dimmed().to_string(),
            None => String::new(),
        };
        println!(
            "  {:<label_width$}  {} {pct}  {reset}",
            w.label,
            bar(w.used_percent, BAR_WIDTH),
        );
    }
}

pub fn print_not_detected(provider: &str) {
    println!("{} {}", provider.bold(), "— not detected".dimmed());
}

pub fn print_error(provider: &str, err: &anyhow::Error) {
    println!("{}", provider.bold());
    println!("  {} {err:#}", "error:".red());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_scales_and_clamps() {
        assert_eq!(bar(59.0, 10), "██████░░░░");
        assert_eq!(bar(0.0, 10), "░░░░░░░░░░");
        assert_eq!(bar(100.0, 10), "██████████");
        assert_eq!(bar(250.0, 10), "██████████");
        assert_eq!(bar(-5.0, 4), "░░░░");
    }

    #[test]
    fn until_humanizes_durations() {
        let now: Timestamp = "2026-07-30T10:00:00Z".parse().unwrap();
        let in_2h13m: Timestamp = "2026-07-30T12:13:40Z".parse().unwrap();
        assert_eq!(until(now, in_2h13m), "2h 13m");
        let in_45m: Timestamp = "2026-07-30T10:45:10Z".parse().unwrap();
        assert_eq!(until(now, in_45m), "45m");
        let in_3d4h: Timestamp = "2026-08-02T14:30:00Z".parse().unwrap();
        assert_eq!(until(now, in_3d4h), "3d 4h");
        let past: Timestamp = "2026-07-30T09:00:00Z".parse().unwrap();
        assert_eq!(until(now, past), "0m");
    }
}
