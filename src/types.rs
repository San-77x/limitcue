use std::time::{SystemTime, UNIX_EPOCH};

/// How trustworthy a reading is. The UI must never present a guess as official.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fidelity {
    /// Vendor's own published/used endpoint
    Official,
    /// Inferred from local logs or unofficial endpoints — may break silently
    Derived,
    /// User-configured generic source
    Manual,
}

/// One quota window, e.g. "5h session" or "weekly".
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Window {
    pub label: String,
    /// Percentage remaining, 0..=100. None when the provider doesn't report a percent.
    pub remaining_percent: Option<f64>,
    /// Remaining count out of total, when the provider reports counts.
    pub remaining_count: Option<u64>,
    pub total_count: Option<u64>,
    /// Unix seconds when this window resets.
    pub resets_at: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reading {
    Ok {
        windows: Vec<Window>,
        detail: Option<String>,
    },
    /// Credentials exist but are expired/invalid — UI shows "needs auth".
    NeedsAuth(String),
    /// Provider not set up on this machine — hidden unless enabled.
    NotConfigured,
    /// Fetch failed; a last-good snapshot may still be shown.
    Error(String),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub provider_id: String,
    pub display_name: String,
    pub fidelity: Fidelity,
    pub reading: Reading,
    pub fetched_at: u64,
}

impl Snapshot {
    /// Worst remaining percentage across all windows (the number you care about).
    pub fn min_remaining(&self) -> Option<f64> {
        match &self.reading {
            Reading::Ok { windows, .. } => windows.iter().filter_map(|w| w.remaining_percent).min_by(|a, b| a.partial_cmp(b).unwrap()),
            _ => None,
        }
    }

    /// Soonest reset among windows that have counts or percents left.
    #[allow(dead_code)]
    pub fn next_reset(&self) -> Option<u64> {
        match &self.reading {
            Reading::Ok { windows, .. } => windows.iter().filter_map(|w| w.resets_at).min(),
            _ => None,
        }
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Human copy for a reset time: near resets count down ("in 51 min",
/// "in 5h 12m"), far ones show the wall-clock moment ("Thu 12:00 AM").
pub fn fmt_reset(resets_at: u64, now: u64) -> String {
    let secs = resets_at.saturating_sub(now);
    if secs < 3600 {
        return format!("Resets in {} min", (secs / 60).max(1));
    }
    if secs < 48 * 3600 {
        return format!("Resets in {}h {:02}m", secs / 3600, (secs % 3600) / 60);
    }
    use chrono::TimeZone;
    match chrono::Local.timestamp_opt(resets_at as i64, 0) {
        chrono::LocalResult::Single(dt) => format!("Resets {}", dt.format("%a %-I:%M %p")),
        _ => format!("Resets in {}", fmt_countdown(secs)),
    }
}

pub fn fmt_countdown(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h}h {m:02}m")
    } else {
        format!("{m}m")
    }
}
