use serde_json::Value;

use super::{expand_home, home, http_get_json, read_json_file, Provider};
use crate::types::{now_unix, Fidelity, Reading, Snapshot, Window};

/// One Grok / xAI login. `dir` is the Grok Build CLI config directory, so a
/// second account is a second instance pointed at `~/.grok-work`.
pub struct Grok {
    dir: std::path::PathBuf,
    id: String,
    name: String,
    api_key: Option<String>,
    /// When true, also look at OpenCode's xAI OAuth — same client id as
    /// `grok login`, just a different file.
    default_login: bool,
}

impl Default for Grok {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Grok {
    pub fn new(cfg: Option<&crate::config::ProviderConfig>) -> Self {
        let dir = cfg
            .and_then(|c| c.credentials_dir.clone())
            .map(expand_home)
            .unwrap_or_else(grok_home);
        Self {
            dir,
            id: cfg.map(|c| c.id.clone()).unwrap_or_else(|| "grok".into()),
            name: cfg
                .filter(|c| !c.name.is_empty())
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Grok".into()),
            api_key: cfg.and_then(|c| c.api_key.clone()),
            default_login: cfg
                .map(|c| c.credentials_dir.is_none())
                .unwrap_or(true),
        }
    }
}

fn grok_home() -> std::path::PathBuf {
    std::env::var("GROK_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(expand_home)
        .unwrap_or_else(|| home().join(".grok"))
}

/// `{"https://auth.x.ai::…": {"key": "…"}, "https://accounts.x.ai/sign-in": {"key": "…"}}`
fn token_from_auth_json(v: &Value) -> Option<String> {
    let obj = v.as_object()?;
    let mut legacy = None;
    for (scope, entry) in obj {
        let Some(key) = entry.get("key").and_then(|k| k.as_str()).filter(|s| !s.is_empty()) else {
            continue;
        };
        if scope.contains("auth.x.ai") {
            return Some(key.into());
        }
        if legacy.is_none() {
            legacy = Some(key.into());
        }
    }
    legacy.or_else(|| v.get("key").and_then(|k| k.as_str()).map(String::from))
}

fn token_from_opencode() -> Option<String> {
    let path = dirs::data_local_dir()?.join("opencode/auth.json");
    let v = read_json_file(&path)?;
    v.get("xai")
        .and_then(|x| x.get("access"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn cents(v: &Value) -> Option<f64> {
    v.get("val")
        .and_then(|x| x.as_f64().or_else(|| x.as_i64().map(|n| n as f64)))
        .or_else(|| v.as_f64())
}

fn usd(cents: f64) -> f64 {
    cents / 100.0
}

/// Turn the Grok Build billing body into a reading.
///
/// Split out from the request so the shape can be pinned by tests. `val` is
/// USD cents, matching the rest of xAI's billing API. A zero monthly limit
/// is an included plan, not an empty one — we still show the period, we do
/// not invent a percentage.
fn parse(v: &Value) -> Reading {
    let Some(cfg) = v.get("config") else {
        return Reading::Error("unrecognised response".into());
    };
    let limit = cents(cfg.get("monthlyLimit").unwrap_or(&Value::Null)).unwrap_or(0.0);
    let used = cents(cfg.get("used").unwrap_or(&Value::Null)).unwrap_or(0.0);
    let on_demand = cents(cfg.get("onDemandCap").unwrap_or(&Value::Null)).unwrap_or(0.0);
    let resets_at = cfg
        .get("billingPeriodEnd")
        .and_then(|s| s.as_str())
        .and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|d| d.timestamp().max(0) as u64)
        });

    let mut windows = Vec::new();
    if limit > 0.0 {
        let remaining = ((limit - used) / limit * 100.0).clamp(0.0, 100.0);
        windows.push(Window {
            label: format!("quota ${:.2} of ${:.0}", usd((limit - used).max(0.0)), usd(limit)),
            remaining_percent: Some(remaining),
            remaining_count: None,
            total_count: None,
            resets_at,
        });
    } else {
        windows.push(Window {
            label: "included plan".into(),
            remaining_percent: None,
            remaining_count: None,
            total_count: None,
            resets_at,
        });
    }
    if on_demand > 0.0 && (limit - on_demand).abs() > f64::EPSILON {
        windows.push(Window {
            label: format!("on-demand ${:.0}", usd(on_demand)),
            remaining_percent: None,
            remaining_count: None,
            total_count: None,
            resets_at,
        });
    }

    if windows.iter().all(|w| w.remaining_percent.is_none() && w.resets_at.is_none()) {
        return Reading::Error("no windows in response".into());
    }
    Reading::Ok {
        windows,
        detail: Some(format!("${:.2} used", usd(used))),
    }
}

impl Provider for Grok {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Derived
    }
    fn is_present(&self) -> bool {
        self.api_key.as_ref().is_some_and(|k| !k.is_empty())
            || std::env::var("XAI_API_KEY").is_ok()
            || std::env::var("GROK_API_KEY").is_ok()
            || self.dir.join("auth.json").exists()
            || (self.default_login && token_from_opencode().is_some())
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
        let token = self
            .api_key
            .clone()
            .filter(|k| !k.is_empty())
            .or_else(|| std::env::var("XAI_API_KEY").ok())
            .or_else(|| std::env::var("GROK_API_KEY").ok())
            .or_else(|| read_json_file(&self.dir.join("auth.json")).as_ref().and_then(token_from_auth_json))
            .or_else(|| if self.default_login { token_from_opencode() } else { None });
        let Some(token) = token else {
            return make(Reading::NotConfigured);
        };
        let headers = [("Authorization", format!("Bearer {token}"))];
        match http_get_json("https://cli-chat-proxy.grok.com/v1/billing", &headers) {
            Ok(v) => make(parse(&v)),
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("run `grok login`".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Recorded from a live `GET cli-chat-proxy.grok.com/v1/billing`, with
    /// the zeroed included-plan numbers replaced by a prepaid cap so the
    /// arithmetic is actually exercised. Identifying fields were never in
    /// this payload.
    fn body() -> Value {
        json!({"config": {
            "monthlyLimit": {"val": 20000},
            "used": {"val": 4500},
            "onDemandCap": {"val": 0},
            "billingPeriodStart": "2026-09-01T00:00:00+00:00",
            "billingPeriodEnd": "2026-10-01T00:00:00+00:00",
            "history": []
        }})
    }

    fn windows(r: &Reading) -> &[Window] {
        match r {
            Reading::Ok { windows, .. } => windows,
            other => panic!("expected a reading, got {other:?}"),
        }
    }

    #[test]
    fn cents_become_a_percentage_and_a_dollar_label() {
        let r = parse(&body());
        let w = &windows(&r)[0];
        assert_eq!(w.remaining_percent, Some(77.5));
        assert_eq!(w.label, "quota $155.00 of $200");
        assert_eq!(
            w.resets_at,
            chrono::DateTime::parse_from_rfc3339("2026-10-01T00:00:00+00:00")
                .ok()
                .map(|d| d.timestamp() as u64)
        );
    }

    #[test]
    fn a_zero_cap_is_an_included_plan_not_an_empty_one() {
        let r = parse(&json!({"config": {
            "monthlyLimit": {"val": 0},
            "used": {"val": 0},
            "onDemandCap": {"val": 0},
            "billingPeriodEnd": "2026-10-01T00:00:00+00:00"
        }}));
        let w = &windows(&r)[0];
        assert_eq!(w.label, "included plan");
        assert_eq!(w.remaining_percent, None);
        assert!(w.resets_at.is_some());
    }

    #[test]
    fn spending_past_the_cap_reads_as_empty_not_as_negative() {
        let r = parse(&json!({"config": {
            "monthlyLimit": {"val": 1000},
            "used": {"val": 5000},
            "billingPeriodEnd": "2026-10-01T00:00:00+00:00"
        }}));
        assert_eq!(windows(&r)[0].remaining_percent, Some(0.0));
    }

    #[test]
    fn a_shape_we_do_not_recognise_is_an_error_not_an_empty_reading() {
        let r = parse(&json!({"balance": 12}));
        assert!(matches!(r, Reading::Error(_)), "got {r:?}");
    }

    #[test]
    fn oidc_scope_wins_over_legacy_in_auth_json() {
        let v = json!({
            "https://accounts.x.ai/sign-in": {"key": "legacy-token"},
            "https://auth.x.ai::b1a00492-073a-47ea-816f-4c329264a828": {"key": "oidc-token"}
        });
        assert_eq!(token_from_auth_json(&v).as_deref(), Some("oidc-token"));
    }
}
