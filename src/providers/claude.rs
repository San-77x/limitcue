use serde_json::Value;

use super::{home, http_get_json, read_json_file, Provider};
use crate::types::{Fidelity, Reading, Snapshot, Window, now_unix};

pub struct Claude;

fn credentials() -> Option<String> {
    let v = read_json_file(&home().join(".claude/.credentials.json"))?;
    v.get("claudeAiOauth")?.get("accessToken")?.as_str().map(String::from)
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

impl Provider for Claude {
    fn id(&self) -> String { "claude".into() }
    fn fidelity(&self) -> Fidelity { Fidelity::Official }
    fn is_present(&self) -> bool { home().join(".claude/.credentials.json").exists() }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: "claude".into(),
            display_name: "Claude".into(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
        };
        let Some(token) = credentials() else { return make(Reading::NotConfigured) };
        let headers = [
            ("Authorization", format!("Bearer {token}")),
            ("anthropic-version", "2023-06-01".into()),
            ("anthropic-beta", "oauth-2025-04-20".into()),
            ("User-Agent", "claude-cli/1.0 (external, cli)".into()),
        ];
        match http_get_json("https://api.anthropic.com/api/oauth/usage", &headers) {
            Ok(v) => {
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
                    make(Reading::Error("no windows in response".into()))
                } else {
                    make(Reading::Ok { windows, detail: None })
                }
            }
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("run `claude login`".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}
