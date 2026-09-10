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
    let util = block.get("utilization")?.as_f64()?;
    let resets_at = block
        .get("resets_at")
        .and_then(|d| d.as_str())
        .and_then(chrono_like_parse);
    Some(Window {
        label: label.into(),
        remaining_percent: Some(((1.0 - util) * 100.0).clamp(0.0, 100.0)),
        remaining_count: None,
        total_count: None,
        resets_at,
    })
}

/// Parse ISO-8601 like 2026-09-07T12:34:56Z into unix seconds without pulling in chrono.
fn chrono_like_parse(s: &str) -> Option<u64> {
    let s = s.trim_end_matches('Z');
    let (date, time) = s.split_once('T')?;
    let mut it = date.split('-').filter_map(|p| p.parse::<u64>().ok());
    let (y, mo, d) = (it.next()?, it.next()?, it.next()?);
    let mut it = time.split(':').filter_map(|p| p.parse::<u64>().ok());
    let (h, mi, sec) = (it.next()?, it.next()?, it.next().unwrap_or(0));
    // days from civil algorithm (Howard Hinnant)
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1 + yoe * 365 + yoe / 4 - yoe / 100;
    let days = era * 146097 + doy - 719468;
    Some(days * 86400 + h * 3600 + mi * 60 + sec)
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

    /// Shape of `GET /api/oauth/usage`, as observed.
    fn body() -> Value {
        json!({
            "five_hour":        {"utilization": 0.62, "resets_at": "2026-09-10T18:00:00Z"},
            "seven_day":        {"utilization": 0.31, "resets_at": "2026-09-14T00:00:00Z"},
            "seven_day_opus":   {"utilization": 0.08, "resets_at": "2026-09-14T00:00:00Z"}
        })
    }

    fn windows(r: &Reading) -> &[Window] {
        match r {
            Reading::Ok { windows, .. } => windows,
            other => panic!("expected a reading, got {other:?}"),
        }
    }

    #[test]
    fn utilisation_is_reported_as_what_is_left() {
        let r = parse(&body());
        let w = windows(&r);
        assert_eq!(w[0].label, "session");
        assert_eq!(w[0].remaining_percent, Some(38.0));
        assert_eq!(w[1].label, "weekly");
        assert_eq!(w[1].remaining_percent, Some(69.0));
    }

    #[test]
    fn only_the_windows_present_are_reported() {
        // No `seven_day_sonnet` in the fixture, so no such row.
        let r = parse(&body());
        assert_eq!(windows(&r).len(), 3);
        assert!(windows(&r).iter().all(|w| w.label != "weekly sonnet"));
    }

    #[test]
    fn reset_times_are_parsed_to_unix_seconds() {
        let r = parse(&body());
        assert_eq!(windows(&r)[0].resets_at, Some(1_789_063_200)); // 2026-09-10T18:00:00Z
    }

    #[test]
    fn a_shape_we_do_not_recognise_is_an_error_not_an_empty_reading() {
        // If the endpoint is reshaped, say so rather than quietly showing
        // nothing — an empty notch looks like "no quota used".
        let r = parse(&json!({"limits": {"five_hour": {"pct": 62}}}));
        assert!(matches!(r, Reading::Error(_)), "got {r:?}");
    }

    #[test]
    fn a_window_without_utilisation_is_skipped_rather_than_zeroed() {
        let r = parse(&json!({"five_hour": {"resets_at": "2026-09-10T18:00:00Z"}}));
        assert!(matches!(r, Reading::Error(_)), "got {r:?}");
    }
}
