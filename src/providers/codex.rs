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

/// Name a window after how long it actually is.
///
/// The two slots are not fixed durations: on a Go plan the primary window is
/// 30 days, so calling it "5h" — as the positional fallback did — told the
/// user their monthly quota resets this afternoon. The response says how long
/// the window is; use it, and keep the positional name only for a plan that
/// does not.
fn window_label(seconds: Option<u64>, fallback: &str) -> String {
    match seconds {
        Some(s) if s >= 86_400 => match s / 86_400 {
            1 => "daily".into(),
            7 => "weekly".into(),
            28..=31 => "monthly".into(),
            d => format!("{d}d"),
        },
        Some(s) if s >= 3_600 => format!("{}h", s / 3_600),
        Some(s) if s >= 60 => format!("{}m", s / 60),
        _ => fallback.into(),
    }
}

/// Turn the usage endpoint's body into a reading. `now` is passed in because
/// this endpoint reports a countdown rather than a timestamp, and a test needs
/// the arithmetic to be reproducible.
fn parse(j: &serde_json::Value, now: u64) -> Reading {
    let mut windows = Vec::new();
    let rl = j.get("rate_limit").cloned().unwrap_or_default();
    for (key, fallback) in [("primary_window", "5h"), ("secondary_window", "weekly")] {
        if let Some(w) = rl.get(key) {
            let used = w.get("used_percent").and_then(|p| p.as_f64());
            // The reset arrives as an absolute timestamp, or as a countdown
            // under either of two names depending on the plan.
            let resets_at = w
                .get("reset_at")
                .and_then(|t| t.as_u64())
                .filter(|t| *t > 1_000_000_000)
                .or_else(|| {
                    ["reset_after_seconds", "resets_in_seconds"]
                        .iter()
                        .find_map(|k| w.get(*k).and_then(|s| s.as_u64()))
                        .map(|s| now + s)
                });
            if used.is_none() && resets_at.is_none() {
                continue;
            }
            windows.push(Window {
                label: window_label(w.get("limit_window_seconds").and_then(|s| s.as_u64()), fallback),
                remaining_percent: used.map(|u| (100.0 - u).clamp(0.0, 100.0)),
                remaining_count: None,
                total_count: None,
                resets_at,
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
            fetch_error: None,
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

    /// Recorded from a live `GET /backend-api/wham/usage` (a Go plan), with
    /// the identifying fields dropped. The window is 30 days, the reset is an
    /// absolute timestamp, and the secondary slot is null.
    ///
    /// The invented fixture this replaces used `resets_in_seconds`, a field
    /// this endpoint does not send — so it happily proved that a reset the
    /// adapter could never find was being read correctly.
    fn body() -> serde_json::Value {
        json!({"rate_limit": {
            "allowed": true,
            "limit_reached": false,
            "primary_window": {
                "used_percent": 27,
                "limit_window_seconds": 2_592_000,
                "reset_after_seconds": 2_408_464,
                "reset_at": 1_791_449_547u64
            },
            "secondary_window": null
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
        assert_eq!(windows(&r)[0].remaining_percent, Some(73.0));
    }

    #[test]
    fn a_window_is_named_after_how_long_it_actually_is() {
        // The regression: this slot was hardcoded "5h", so a 30-day quota
        // claimed to reset this afternoon.
        let r = parse(&body(), NOW);
        assert_eq!(windows(&r)[0].label, "monthly");
        assert_eq!(window_label(Some(18_000), "x"), "5h");
        assert_eq!(window_label(Some(604_800), "x"), "weekly");
        assert_eq!(window_label(None, "5h"), "5h", "fall back when unsaid");
    }

    #[test]
    fn an_absolute_reset_is_used_as_given() {
        // Not `now + reset`: this is a timestamp, not a countdown.
        let r = parse(&body(), NOW);
        assert_eq!(windows(&r)[0].resets_at, Some(1_791_449_547));
    }

    #[test]
    fn a_countdown_is_resolved_against_now_when_there_is_no_timestamp() {
        let r = parse(
            &json!({"rate_limit": {"primary_window": {"used_percent": 10, "reset_after_seconds": 3600}}}),
            NOW,
        );
        assert_eq!(windows(&r)[0].resets_at, Some(NOW + 3600));
    }

    #[test]
    fn a_null_secondary_window_is_skipped() {
        assert_eq!(windows(&parse(&body(), NOW)).len(), 1);
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
