use serde_json::Value;

use super::{home, http_get_json, read_json_file, Provider};
use crate::types::{Fidelity, Reading, Snapshot, Window, now_unix};

/// One Claude login. `dir` is the CLI's config directory, so a second
/// account is just a second instance pointed at `~/.claude-work`.
pub struct Claude {
    dir: std::path::PathBuf,
    id: String,
    name: String,
}

impl Default for Claude {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Claude {
    pub fn new(cfg: Option<&crate::config::ProviderConfig>) -> Self {
        let dir = cfg
            .and_then(|c| c.credentials_dir.clone())
            .map(super::expand_home)
            .unwrap_or_else(|| home().join(".claude"));
        Self {
            dir,
            id: cfg.map(|c| c.id.clone()).unwrap_or_else(|| "claude".into()),
            name: cfg
                .filter(|c| !c.name.is_empty())
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Claude".into()),
        }
    }

    fn credentials(&self) -> Option<String> {
        let v = read_json_file(&self.dir.join(".credentials.json"))?;
        v.get("claudeAiOauth")?.get("accessToken")?.as_str().map(String::from)
    }
}

fn window(label: &str, block: &Value) -> Option<Window> {
    // `utilization` is a percentage, 0-100 — not a fraction. Reading it as a
    // fraction made every window come back exhausted: at 8 % used,
    // (1.0 - 8.0) * 100 is -700, which clamps to nothing left.
    let util = block.get("utilization")?.as_f64()?;
    let resets_at = block
        .get("resets_at")
        .and_then(|d| d.as_str())
        .and_then(parse_timestamp);
    Some(Window {
        label: label.into(),
        remaining_percent: Some((100.0 - util).clamp(0.0, 100.0)),
        remaining_count: None,
        total_count: None,
        resets_at,
    })
}

/// The endpoint reports RFC3339 with fractional seconds and an offset
/// ("2026-09-10T14:09:59.970852+00:00"). chrono is already a dependency and
/// handles both; the hand-rolled parser this replaces dropped the seconds and
/// assumed UTC.
fn parse_timestamp(s: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp().max(0) as u64)
}

/// Turn the usage endpoint's body into a reading.
///
/// Split out from the request so the response shape can be pinned by tests.
/// This endpoint is not one we control: the shape changing is a thing that
/// happens, and finding out from a test beats finding out from a user.
fn parse(v: &Value) -> Reading {
    let mut windows = Vec::new();
    for (key, label) in [
        ("five_hour", "session"),
        ("seven_day", "weekly"),
        ("seven_day_sonnet", "weekly sonnet"),
        ("seven_day_opus", "weekly opus"),
    ] {
        if let Some(b) = v.get(key) {
            if let Some(w) = window(label, b) {
                windows.push(w);
            }
        }
    }
    if windows.is_empty() {
        Reading::Error("no windows in response".into())
    } else {
        Reading::Ok { windows, detail: None }
    }
}

impl Provider for Claude {
    fn id(&self) -> String { self.id.clone() }
    fn fidelity(&self) -> Fidelity { Fidelity::Official }
    fn is_present(&self) -> bool { self.dir.join(".credentials.json").exists() }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: self.id.clone(),
            display_name: self.name.clone(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
        };
        let Some(token) = self.credentials() else { return make(Reading::NotConfigured) };
        let headers = [
            ("Authorization", format!("Bearer {token}")),
            ("anthropic-version", "2023-06-01".into()),
            ("anthropic-beta", "oauth-2025-04-20".into()),
            ("User-Agent", "claude-cli/1.0 (external, cli)".into()),
        ];
        match http_get_json("https://api.anthropic.com/api/oauth/usage", &headers) {
            Ok(v) => make(parse(&v)),
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("run `claude login`".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Recorded from a live `GET /api/oauth/usage`, structure intact:
    /// `utilization` is a **percentage**, most window slots are null, and
    /// timestamps carry fractional seconds and an offset.
    ///
    /// The previous version of this fixture was invented from what the parser
    /// assumed rather than recorded from the endpoint — it used `0.62` for a
    /// utilisation — so it agreed with the bug instead of catching it. A
    /// fixture is only worth anything if it comes from the wire.
    fn body() -> Value {
        json!({
            "five_hour": {
                "utilization": 8.0,
                "resets_at": "2026-09-10T14:09:59.970852+00:00",
                "limit_dollars": null, "used_dollars": null,
                "remaining_dollars": null, "locked_reason": null
            },
            "seven_day": {
                "utilization": 83.0,
                "resets_at": "2026-09-12T01:59:59.970876+00:00",
                "limit_dollars": null, "used_dollars": null,
                "remaining_dollars": null, "locked_reason": null
            },
            "seven_day_opus": null,
            "seven_day_sonnet": null,
            "extra_usage": {"is_enabled": false, "monthly_limit": null}
        })
    }

    fn windows(r: &Reading) -> &[Window] {
        match r {
            Reading::Ok { windows, .. } => windows,
            other => panic!("expected a reading, got {other:?}"),
        }
    }

    #[test]
    fn utilisation_is_a_percentage_not_a_fraction() {
        // The regression that mattered: 8 % used is 92 % left, not nothing
        // left. Reading the field as a fraction clamped every window to zero,
        // so a healthy plan showed as completely spent.
        let r = parse(&body());
        let w = windows(&r);
        assert_eq!((w[0].label.as_str(), w[0].remaining_percent), ("session", Some(92.0)));
        assert_eq!((w[1].label.as_str(), w[1].remaining_percent), ("weekly", Some(17.0)));
    }

    #[test]
    fn null_window_slots_are_skipped() {
        // The response carries a slot for every window kind that could exist,
        // most of them null on any given plan.
        assert_eq!(windows(&parse(&body())).len(), 2);
    }

    #[test]
    fn timestamps_with_fractional_seconds_and_an_offset_are_parsed() {
        let r = parse(&body());
        // 2026-09-10T14:09:59Z
        assert_eq!(windows(&r)[0].resets_at, Some(1_789_049_399));
    }

    #[test]
    fn a_fully_spent_window_still_reads_as_spent() {
        let r = parse(&json!({"five_hour": {"utilization": 100.0}}));
        assert_eq!(windows(&r)[0].remaining_percent, Some(0.0));
    }

    #[test]
    fn over_use_is_clamped_rather_than_reported_as_negative() {
        let r = parse(&json!({"five_hour": {"utilization": 140.0}}));
        assert_eq!(windows(&r)[0].remaining_percent, Some(0.0));
    }

    #[test]
    fn a_shape_we_do_not_recognise_is_an_error_not_an_empty_reading() {
        let r = parse(&json!({"limits": {"five_hour": {"pct": 62}}}));
        assert!(matches!(r, Reading::Error(_)), "got {r:?}");
    }
}
