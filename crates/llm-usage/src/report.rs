use jiff::Timestamp;
use serde::Serialize;

/// Usage limits for one detected provider.
#[derive(Debug, Serialize)]
pub struct Report {
    pub provider: String,
    /// e.g. subscription plan / rate-limit tier.
    pub detail: Option<String>,
    /// When the data was captured, if it comes from a local snapshot
    /// rather than a live API call.
    pub as_of: Option<Timestamp>,
    /// Caveat worth showing under the heading, e.g. why data is stale.
    pub note: Option<String>,
    pub windows: Vec<Window>,
}

/// One rate-limit window (e.g. the 5-hour session or the weekly cap).
#[derive(Debug, Serialize)]
pub struct Window {
    pub label: String,
    pub used_percent: f64,
    pub resets_at: Option<Timestamp>,
}
