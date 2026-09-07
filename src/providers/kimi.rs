use serde_json::Value;

use super::{home, http_get_json, read_json_file, Provider};
use crate::config::ProviderConfig;
use crate::types::{Fidelity, Reading, Snapshot, Window, now_unix};

pub struct Kimi {
    cfg: Option<ProviderConfig>,
}

impl Kimi {
    pub fn new(cfg: Option<ProviderConfig>) -> Self { Self { cfg } }
}

/// Some Kimi responses use strings for numeric fields.
fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| {
        x.as_f64()
            .or_else(|| x.as_u64().map(|n| n as f64))
            .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
    })
}

fn parse_iso(s: &str) -> Option<u64> {
    let s = s.trim_end_matches('Z');
    let (date, time) = s.split_once('T')?;
    let mut it = date.split('-').filter_map(|p| p.parse::<u64>().ok());
    let (y, mo, d) = (it.next()?, it.next()?, it.next()?);
    let time = time.split('.').next().unwrap_or(time);
    let mut it = time.split(':').filter_map(|p| p.parse::<u64>().ok());
    let (h, mi, sec) = (it.next()?, it.next()?, it.next().unwrap_or(0));
    let y = if mo <= 2 { y - 1 } else { y };
    let era = y / 400;
    let yoe = y - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1 + yoe * 365 + yoe / 4 - yoe / 100;
    let days = era * 146097 + doy - 719468;
    Some(days * 86400 + h * 3600 + mi * 60 + sec)
}

fn row_to_window(detail: &Value, fallback_label: &str, window: &Value) -> Option<Window> {
    let limit = num(detail, "limit").filter(|l| *l > 0.0);
    let remaining = num(detail, "remaining").or_else(|| {
        let used = num(detail, "used")?;
        let l = limit?;
        Some(l - used)
    });
    let pct = match (remaining, limit) {
        (Some(r), Some(l)) => Some((r / l * 100.0).clamp(0.0, 100.0)),
        _ => None,
    };
    pct?;
    let label = detail
        .get("name")
        .and_then(|n| n.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            let dur = num(window, "duration").unwrap_or(0.0);
            let unit = window.get("timeUnit").and_then(|u| u.as_str()).unwrap_or("");
            if dur > 0.0 {
                format!("{dur:.0} {unit}")
            } else {
                fallback_label.into()
            }
        });
    let resets_at = detail
        .get("resetTime")
        .or_else(|| detail.get("reset_at"))
        .or_else(|| detail.get("resetAt"))
        .and_then(|v| v.as_str())
        .and_then(parse_iso);
    Some(Window {
        label,
        remaining_percent: pct,
        remaining_count: remaining.map(|r| r as u64),
        total_count: limit.map(|l| l as u64),
        resets_at,
    })
}

/// api_key = "..." in ~/.kimi/config.toml under [providers.kimi-for-coding]
fn key_from_kimi_config() -> Option<String> {
    let text = std::fs::read_to_string(home().join(".kimi/config.toml")).ok()?;
    let mut in_provider = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_provider = l.starts_with("[providers");
            continue;
        }
        if in_provider {
            if let Some(rest) = l.strip_prefix("api_key") {
                let val = rest.split('=').nth(1)?.trim().trim_matches('"').trim_matches('\'');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

impl Provider for Kimi {
    fn id(&self) -> String { "kimi".into() }
    fn fidelity(&self) -> Fidelity { Fidelity::Derived }
    fn is_present(&self) -> bool {
        self.cfg.as_ref().and_then(|c| c.api_key.clone()).is_some()
            || std::env::var("KIMI_API_KEY").is_ok()
            || key_from_kimi_config().is_some()
            || home().join(".kimi-code/credentials/kimi-code.json").exists()
    }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: "kimi".into(),
            display_name: "Kimi".into(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
        };
        let token = self
            .cfg
            .as_ref()
            .and_then(|c| c.api_key.clone())
            .or_else(|| std::env::var("KIMI_API_KEY").ok())
            .or_else(key_from_kimi_config)
            .or_else(|| {
                let v = read_json_file(&home().join(".kimi-code/credentials/kimi-code.json"))?;
                v.get("oauth")
                    .and_then(|o| o.get("accessToken"))
                    .and_then(|s| s.as_str())
                    .map(String::from)
                    .or_else(|| v.get("accessToken").and_then(|s| s.as_str()).map(String::from))
            });
        let Some(token) = token else { return make(Reading::NotConfigured) };
        let base = self
            .cfg
            .as_ref()
            .and_then(|c| c.base_url.clone())
            .unwrap_or_else(|| std::env::var("KIMI_CODE_BASE_URL").unwrap_or_else(|_| "https://api.kimi.com/coding/v1".into()));
        let url = format!("{base}/usages");
        match http_get_json(&url, &[("Authorization", format!("Bearer {token}"))]) {
            Ok(v) => {
                let mut windows = Vec::new();
                if let Some(u) = v.get("usage") {
                    if let Some(w) = row_to_window(u, "weekly", &Value::Null) {
                        windows.push(w);
                    }
                }
                if let Some(list) = v.get("limits").and_then(|l| l.as_array()) {
                    for item in list {
                        let detail = item.get("detail").cloned().unwrap_or_else(|| item.clone());
                        let window = item.get("window").cloned().unwrap_or_default();
                        if let Some(w) = row_to_window(&detail, "window", &window) {
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
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("check KIMI_API_KEY / kimi login".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}
