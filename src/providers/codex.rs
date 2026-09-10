use super::{home, http_get_json, read_json_file, Provider};
use crate::types::{Fidelity, Reading, Snapshot, Window, now_unix};

/// One Codex login, read from a CLI config directory.
pub struct Codex {
    dir: std::path::PathBuf,
    id: String,
    name: String,
}

impl Default for Codex {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Codex {
    pub fn new(cfg: Option<&crate::config::ProviderConfig>) -> Self {
        let dir = cfg
            .and_then(|c| c.credentials_dir.clone())
            .map(super::expand_home)
            .unwrap_or_else(|| home().join(".codex"));
        Self {
            dir,
            id: cfg.map(|c| c.id.clone()).unwrap_or_else(|| "codex".into()),
            name: cfg
                .filter(|c| !c.name.is_empty())
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "ChatGPT / Codex".into()),
        }
    }
}

/// Turn the usage endpoint's body into a reading. `now` is passed in because
/// this endpoint reports a countdown rather than a timestamp, and a test needs
/// the arithmetic to be reproducible.
fn parse(j: &serde_json::Value, now: u64) -> Reading {
    let mut windows = Vec::new();
    let rl = j.get("rate_limit").cloned().unwrap_or_default();
    for (key, label) in [("primary_window", "5h"), ("secondary_window", "weekly")] {
        if let Some(w) = rl.get(key) {
            let used = w.get("used_percent").and_then(|p| p.as_f64());
            let resets_in = w.get("resets_in_seconds").and_then(|s| s.as_u64());
            if used.is_none() && resets_in.is_none() {
                continue;
            }
            windows.push(Window {
                label: label.into(),
                remaining_percent: used.map(|u| (100.0 - u).clamp(0.0, 100.0)),
                remaining_count: None,
                total_count: None,
                resets_at: resets_in.map(|s| now + s),
            });
        }
    }
    if windows.is_empty() {
        Reading::Error("no windows in response".into())
    } else {
        Reading::Ok { windows, detail: None }
    }
}

impl Provider for Codex {
    fn id(&self) -> String { self.id.clone() }
    fn fidelity(&self) -> Fidelity { Fidelity::Official }
    fn is_present(&self) -> bool { self.dir.join("auth.json").exists() }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: self.id.clone(),
            display_name: self.name.clone(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
        };
        let Some(v) = read_json_file(&self.dir.join("auth.json")) else {
            return make(Reading::NotConfigured);
        };
        let Some(token) = v.get("tokens").and_then(|t| t.get("access_token")).and_then(|s| s.as_str()) else {
            return make(Reading::NotConfigured);
        };
        let account_id = v.get("last_account").and_then(|a| a.get("account_id")).and_then(|s| s.as_str()).unwrap_or("");
        let headers = [
            ("Authorization", format!("Bearer {token}")),
            ("chatgpt-account-id", account_id.to_string()),
            ("User-Agent", "CodexBar/limitcue".into()),
        ];
        match http_get_json("https://chatgpt.com/backend-api/wham/usage", &headers) {
            Ok(j) => make(parse(&j, now_unix())),
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("run `codex login`".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 1_789_000_000;

    /// Shape of `GET /backend-api/wham/usage`, as observed.
    fn body() -> serde_json::Value {
        json!({"rate_limit": {
            "primary_window":   {"used_percent": 27.0, "resets_in_seconds": 3600},
            "secondary_window": {"used_percent": 5.5,  "resets_in_seconds": 86400}
        }})
    }

    fn windows(r: &Reading) -> &[Window] {
        match r {
            Reading::Ok { windows, .. } => windows,
            other => panic!("expected a reading, got {other:?}"),
        }
    }

    #[test]
    fn used_percent_is_reported_as_what_is_left() {
        let r = parse(&body(), NOW);
        let w = windows(&r);
        assert_eq!((w[0].label.as_str(), w[0].remaining_percent), ("5h", Some(73.0)));
        assert_eq!((w[1].label.as_str(), w[1].remaining_percent), ("weekly", Some(94.5)));
    }

    #[test]
    fn a_countdown_becomes_a_wall_clock_reset() {
        let r = parse(&body(), NOW);
        assert_eq!(windows(&r)[0].resets_at, Some(NOW + 3600));
    }

    #[test]
    fn a_window_reporting_only_a_countdown_still_counts() {
        // Seen in the wild between resets: no percentage yet, but a timer.
        let r = parse(&json!({"rate_limit": {"primary_window": {"resets_in_seconds": 60}}}), NOW);
        let w = windows(&r);
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].remaining_percent, None);
        assert_eq!(w[0].resets_at, Some(NOW + 60));
    }

    #[test]
    fn over_use_is_clamped_rather_than_reported_as_negative() {
        let r = parse(&json!({"rate_limit": {"primary_window": {"used_percent": 130.0}}}), NOW);
        assert_eq!(windows(&r)[0].remaining_percent, Some(0.0));
    }

    #[test]
    fn a_shape_we_do_not_recognise_is_an_error_not_an_empty_reading() {
        let r = parse(&json!({"rate_limits": []}), NOW);
        assert!(matches!(r, Reading::Error(_)), "got {r:?}");
    }
}
