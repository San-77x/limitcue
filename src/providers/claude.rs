use serde_json::Value;

use super::{home, http_get_json, read_json_file, Provider};
use crate::types::{now_unix, Fidelity, Reading, Snapshot, Window};

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
        v.get("claudeAiOauth")?
            .get("accessToken")?
            .as_str()
            .map(String::from)
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

/// Name a window after the key it arrived under.
///
/// `five_hour` and `seven_day` are the plan-wide windows; anything
/// `seven_day_*` is a weekly allowance of its own, so `seven_day_cowork`
/// reads "weekly cowork". Keys we have never seen are prettified rather than
/// hidden — a quota nobody has taught this app about is still the user's
/// quota.
fn label_for(key: &str) -> String {
    match key {
        "five_hour" => "session".into(),
        "seven_day" => "weekly".into(),
        k => match k.strip_prefix("seven_day_") {
            Some(rest) => format!("weekly {}", rest.replace('_', " ")),
            None => k.replace('_', " "),
        },
    }
}

/// Should a window the app does not recognise be shown?
///
/// The response carries a slot for every window kind that could exist, most
/// of them null and some of them internal code names sitting at zero. A named
/// window always counts; an unfamiliar one counts once it is actually in use
/// or has a reset scheduled, which is the point at which it means something to
/// the person reading it.
fn worth_showing(key: &str, util: f64, resets_at: Option<u64>) -> bool {
    key == "five_hour"
        || key == "seven_day"
        || key.starts_with("seven_day_")
        || util > 0.0
        || resets_at.is_some()
}

/// Turn the usage endpoint's body into a reading.
///
/// Every window in the response is read, rather than a hardcoded four. The
/// endpoint enumerates a slot per window kind — per-model weeklies among them
/// — and which ones an account has depends on its plan, so listing them in
/// code meant a new one stayed invisible until somebody edited Rust.
fn parse(v: &Value) -> Reading {
    let Some(map) = v.as_object() else {
        return Reading::Error("unrecognised response".into());
    };
    let mut windows = Vec::new();
    for (key, block) in map {
        // `extra_usage` and friends are objects without a utilisation; they
        // are not quota windows.
        let Some(w) = window(&label_for(key), block) else {
            continue;
        };
        let util = block
            .get("utilization")
            .and_then(|u| u.as_f64())
            .unwrap_or(0.0);
        if worth_showing(key, util, w.resets_at) {
            windows.push(w);
        }
    }
    // Plan-wide windows first, then the rest in a stable order.
    windows.sort_by_key(|w| match w.label.as_str() {
        "session" => (0, String::new()),
        "weekly" => (1, String::new()),
        other => (2, other.to_string()),
    });
    if windows.is_empty() {
        Reading::Error("no windows in response".into())
    } else {
        Reading::Ok {
            windows,
            detail: None,
        }
    }
}

impl Provider for Claude {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Official
    }
    fn is_present(&self) -> bool {
        self.dir.join(".credentials.json").exists()
    }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: self.id.clone(),
            display_name: self.name.clone(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
            fetch_error: None,
        };
        let Some(token) = self.credentials() else {
            return make(Reading::NotConfigured);
        };
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
            // A window the hardcoded list never included, taken from a real
            // response rather than imagined.
            "seven_day_cowork": {
                "utilization": 12.0,
                "resets_at": "2026-09-12T01:59:59.970876+00:00",
                "limit_dollars": null, "used_dollars": null,
                "remaining_dollars": null, "locked_reason": null
            },
            // An internal code name, dormant on this account.
            "nimbus_quill": {"utilization": 0.0, "resets_at": null},
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
        assert_eq!(
            (w[0].label.as_str(), w[0].remaining_percent),
            ("session", Some(92.0))
        );
        assert_eq!(
            (w[1].label.as_str(), w[1].remaining_percent),
            ("weekly", Some(17.0))
        );
    }

    #[test]
    fn null_window_slots_are_skipped() {
        // The response carries a slot for every window kind that could exist,
        // most of them null on any given plan.
        let r = parse(&body());
        let labels: Vec<&str> = windows(&r).iter().map(|w| w.label.as_str()).collect();
        assert_eq!(labels, ["session", "weekly", "weekly cowork"]);
    }

    #[test]
    fn a_window_outside_the_old_hardcoded_four_still_appears() {
        // The endpoint enumerates a slot per window kind and which ones an
        // account has depends on its plan. Listing four of them in code meant
        // the rest stayed invisible until somebody edited Rust.
        let r = parse(&body());
        let w = windows(&r)
            .iter()
            .find(|w| w.label == "weekly cowork")
            .expect("cowork window");
        assert_eq!(w.remaining_percent, Some(88.0));
    }

    #[test]
    fn a_dormant_internal_slot_is_not_shown() {
        // `nimbus_quill` and friends sit at zero with no reset. Showing an
        // unexplained code name at 100% would be noise, so an unfamiliar
        // window has to be in use or scheduled before it earns a row.
        let r = parse(&body());
        assert!(windows(&r).iter().all(|w| w.label != "nimbus quill"));
    }

    #[test]
    fn an_unfamiliar_window_that_is_in_use_does_earn_a_row() {
        let r = parse(&json!({"copper_kite": {"utilization": 30.0, "resets_at": null}}));
        let w = &windows(&r)[0];
        assert_eq!(w.label, "copper kite");
        assert_eq!(w.remaining_percent, Some(70.0));
    }

    #[test]
    fn plan_wide_windows_come_first() {
        let r = parse(&body());
        assert_eq!(windows(&r)[0].label, "session");
        assert_eq!(windows(&r)[1].label, "weekly");
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
