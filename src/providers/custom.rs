use serde_json::Value;

use super::{http_get_json, json_path, Provider};
use crate::config::{ProviderConfig, WindowConfig};
use crate::types::{now_unix, Fidelity, Reading, Snapshot, Window};

/// A user-declared provider: point it at any JSON usage endpoint and map the
/// fields with dot-paths. This is what lets LimitCue cover a provider the day
/// it ships an endpoint, without waiting for a first-party adapter.
///
/// ```toml
/// [[provider]]
/// id = "openrouter"
/// name = "OpenRouter"
/// url = "https://openrouter.ai/api/v1/auth/key"
/// auth_header = "Authorization: Bearer {key}"
/// key_env = "OPENROUTER_API_KEY"
/// windows = [
///   { label = "credits", remaining_count_path = "data.limit_remaining", total_count_path = "data.limit" },
/// ]
/// ```
pub struct Custom {
    cfg: ProviderConfig,
}

/// A number, however the endpoint chose to encode it. Plenty of APIs return
/// balances as strings ("110.00"), which used to read as "no value".
fn num(v: &Value) -> Option<f64> {
    if let Some(f) = v.as_f64() {
        return Some(f);
    }
    v.as_str()?.trim().parse::<f64>().ok()
}

/// A unix timestamp from seconds, milliseconds, or RFC3339.
fn timestamp(v: &Value) -> Option<u64> {
    if let Some(n) = num(v) {
        return match n {
            n if n > 1e12 => Some((n / 1000.0) as u64), // millis
            n if n > 1e9 => Some(n as u64),             // seconds
            _ => None,                                  // too small to be a date
        };
    }
    let s = v.as_str()?;
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.timestamp().max(0) as u64)
}

impl Custom {
    pub fn new(cfg: ProviderConfig) -> Self {
        Self { cfg }
    }

    fn resolve_key(&self) -> Option<String> {
        if let Some(k) = &self.cfg.api_key {
            if !k.trim().is_empty() {
                return Some(k.clone());
            }
        }
        self.cfg
            .key_env
            .as_deref()
            .filter(|e| !e.trim().is_empty())
            .and_then(|e| std::env::var(e).ok())
    }

    /// Build one window from whichever pair of values the mapping names.
    /// Returns None when nothing in the response matched, so a partly-wrong
    /// mapping degrades to "this window is missing" rather than to a zero.
    fn window(&self, w: &WindowConfig, v: &Value, now: u64) -> Option<Window> {
        let at = |p: &Option<String>| p.as_deref().and_then(|p| json_path(v, p));
        let pct = at(&w.remaining_path).and_then(num);
        let frac = at(&w.remaining_fraction_path).and_then(num).map(|f| f * 100.0);
        let total = at(&w.total_count_path).and_then(num).or(w.total_const);
        let remaining = at(&w.remaining_count_path).and_then(num).or_else(|| {
            // "used" is the same fact told backwards
            match (at(&w.used_count_path).and_then(num), total) {
                (Some(used), Some(total)) => Some(total - used),
                _ => None,
            }
        });
        let derived = match (remaining, total) {
            (Some(r), Some(t)) if t > 0.0 => Some((r / t * 100.0).clamp(0.0, 100.0)),
            _ => None,
        };
        let remaining_percent = pct.or(frac).or(derived);

        let resets_at = at(&w.resets_at_path).and_then(timestamp).or_else(|| {
            at(&w.resets_in_path)
                .and_then(num)
                .filter(|s| *s >= 0.0)
                .map(|s| now + s as u64)
        });

        // Counts only read as counts when they are whole units of something.
        let (remaining_count, total_count) = match (remaining, total) {
            (Some(r), Some(t)) if r.fract() == 0.0 && t.fract() == 0.0 && t > 0.0 => {
                (Some(r.max(0.0) as u64), Some(t as u64))
            }
            _ => (None, None),
        };

        if remaining_percent.is_none() && resets_at.is_none() {
            return None;
        }
        Some(Window {
            label: w.label.clone(),
            remaining_percent,
            remaining_count,
            total_count,
            resets_at,
        })
    }

    /// Every header the request should carry, `{key}` already substituted.
    fn headers(&self) -> Vec<(String, String)> {
        let key = self.resolve_key().unwrap_or_default();
        self.cfg
            .auth_header
            .iter()
            .chain(self.cfg.headers.iter())
            .filter_map(|h| {
                let (name, value) = h.split_once(':')?;
                Some((name.trim().to_string(), value.trim().replace("{key}", &key)))
            })
            .collect()
    }
}

impl Provider for Custom {
    fn id(&self) -> String {
        self.cfg.id.clone()
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Manual
    }
    fn is_present(&self) -> bool {
        // A key is only required if the request actually interpolates one.
        let wants_key = self
            .cfg
            .auth_header
            .iter()
            .chain(self.cfg.headers.iter())
            .any(|h| h.contains("{key}"));
        self.cfg.url.is_some() && (!wants_key || self.resolve_key().is_some())
    }

    fn snapshot(&self) -> Snapshot {
        let now = now_unix();
        let make = |reading| Snapshot {
            provider_id: self.cfg.id.clone(),
            display_name: if self.cfg.name.is_empty() {
                self.cfg.id.clone()
            } else {
                self.cfg.name.clone()
            },
            fidelity: self.fidelity(),
            reading,
            fetched_at: now,
        };
        let Some(url) = &self.cfg.url else {
            return make(Reading::NotConfigured);
        };
        let headers = self.headers();
        let refs: Vec<(&str, String)> = headers.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        match http_get_json(url, &refs) {
            Ok(v) => {
                let windows: Vec<Window> =
                    self.cfg.windows.iter().filter_map(|w| self.window(w, &v, now)).collect();
                if windows.is_empty() {
                    make(Reading::Error(if self.cfg.windows.is_empty() {
                        "no quota windows mapped yet".into()
                    } else {
                        "the mapped paths matched nothing in the response".into()
                    }))
                } else {
                    make(Reading::Ok { windows, detail: None })
                }
            }
            Err(e) if e == "auth-failed" => {
                make(Reading::NeedsAuth("the API key was rejected".into()))
            }
            Err(e) => make(Reading::Error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: u64 = 1_700_000_000;

    fn map(w: WindowConfig, body: serde_json::Value) -> Option<Window> {
        let c = Custom::new(ProviderConfig::default());
        c.window(&w, &body, NOW)
    }

    fn labelled() -> WindowConfig {
        WindowConfig { label: "w".into(), ..Default::default() }
    }

    #[test]
    fn reads_a_ready_made_percentage() {
        let w = WindowConfig { remaining_path: Some("u.pct".into()), ..labelled() };
        let got = map(w, json!({"u": {"pct": 63.5}})).unwrap();
        assert_eq!(got.remaining_percent, Some(63.5));
    }

    #[test]
    fn scales_a_fraction() {
        let w = WindowConfig { remaining_fraction_path: Some("f".into()), ..labelled() };
        assert_eq!(map(w, json!({"f": 0.25})).unwrap().remaining_percent, Some(25.0));
    }

    #[test]
    fn derives_a_percentage_from_counts() {
        let w = WindowConfig {
            remaining_count_path: Some("left".into()),
            total_count_path: Some("cap".into()),
            ..labelled()
        };
        let got = map(w, json!({"left": 12, "cap": 30})).unwrap();
        assert_eq!(got.remaining_percent, Some(40.0));
        assert_eq!((got.remaining_count, got.total_count), (Some(12), Some(30)));
    }

    #[test]
    fn derives_a_percentage_from_usage() {
        let w = WindowConfig {
            used_count_path: Some("spent".into()),
            total_count_path: Some("cap".into()),
            ..labelled()
        };
        assert_eq!(map(w, json!({"spent": 18, "cap": 30})).unwrap().remaining_percent, Some(40.0));
    }

    #[test]
    fn a_known_ceiling_turns_a_bare_balance_into_a_gauge() {
        let w = WindowConfig {
            remaining_count_path: Some("bal[0].total".into()),
            total_const: Some(50.0),
            ..labelled()
        };
        let got = map(w, json!({"bal": [{"total": "12.50"}]})).unwrap();
        assert_eq!(got.remaining_percent, Some(25.0));
        // dollars are not whole units, so they are not shown as a count
        assert_eq!(got.remaining_count, None);
    }

    #[test]
    fn numbers_encoded_as_strings_still_count() {
        let w = WindowConfig { remaining_path: Some("p".into()), ..labelled() };
        assert_eq!(map(w, json!({"p": "42"})).unwrap().remaining_percent, Some(42.0));
    }

    #[test]
    fn accepts_seconds_millis_and_rfc3339_timestamps() {
        let secs = WindowConfig { resets_at_path: Some("t".into()), ..labelled() };
        assert_eq!(map(secs.clone(), json!({"t": 1_700_003_600u64})).unwrap().resets_at, Some(1_700_003_600));
        assert_eq!(map(secs.clone(), json!({"t": 1_700_003_600_000u64})).unwrap().resets_at, Some(1_700_003_600));
        assert_eq!(
            map(secs, json!({"t": "2023-11-14T22:33:20Z"})).unwrap().resets_at,
            Some(1_700_001_200)
        );
    }

    #[test]
    fn a_countdown_is_resolved_against_now() {
        let w = WindowConfig { resets_in_path: Some("in".into()), ..labelled() };
        assert_eq!(map(w, json!({"in": 3600})).unwrap().resets_at, Some(NOW + 3600));
    }

    #[test]
    fn a_window_that_matched_nothing_is_dropped_rather_than_zeroed() {
        let w = WindowConfig { remaining_path: Some("nope.missing".into()), ..labelled() };
        assert!(map(w, json!({"u": 1})).is_none());
    }

    #[test]
    fn a_zero_ceiling_does_not_divide() {
        let w = WindowConfig {
            remaining_count_path: Some("left".into()),
            total_count_path: Some("cap".into()),
            ..labelled()
        };
        assert!(map(w, json!({"left": 0, "cap": 0})).is_none());
    }
}
