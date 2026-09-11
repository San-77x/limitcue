use super::{http_get_json, json_path, Provider};
use crate::config::ProviderConfig;
use crate::types::{Fidelity, Reading, Snapshot, Window, now_unix};

pub struct MiniMax {
    cfg: ProviderConfig,
}

impl MiniMax {
    pub fn new(cfg: ProviderConfig) -> Self { Self { cfg } }

    fn key(&self) -> Option<String> {
        self.cfg.api_key.clone().or_else(|| std::env::var("MINIMAX_API_KEY").ok())
    }
}

/// Turn the token-plan response into a reading.
///
/// Split out from the request so the response shape can be pinned by tests —
/// this one carries its own error channel inside a 200, which is the sort of
/// thing that silently starts reading as "no windows" when it changes.
fn parse(v: &serde_json::Value) -> Reading {
    if json_path(v, "base_resp.status_code").and_then(|c| c.as_i64()).unwrap_or(0) != 0 {
        let msg = json_path(v, "base_resp.status_msg").and_then(|m| m.as_str()).unwrap_or("");
        return Reading::NeedsAuth(msg.into());
    }
    let mut windows = Vec::new();
    if let Some(list) = json_path(v, "model_remains").and_then(|m| m.as_array()) {
        for m in list {
            let name = m.get("model_name").and_then(|n| n.as_str()).unwrap_or("?");
            let get = |p: &str| json_path(m, p).and_then(|x| x.as_f64());
            let getu = |p: &str| json_path(m, p).and_then(|x| x.as_u64());
            if let Some(pct) = get("current_interval_remaining_percent") {
                windows.push(Window {
                    label: format!("{name} 5h"),
                    remaining_percent: Some(pct),
                    remaining_count: getu("current_interval_usage_count"),
                    total_count: getu("current_interval_total_count"),
                    resets_at: getu("end_time").map(|ms| ms / 1000),
                });
            }
            if let Some(pct) = get("current_weekly_remaining_percent") {
                windows.push(Window {
                    label: format!("{name} weekly"),
                    remaining_percent: Some(pct),
                    remaining_count: None,
                    total_count: getu("current_weekly_total_count"),
                    resets_at: getu("weekly_end_time").map(|ms| ms / 1000),
                });
            }
        }
    }
    if windows.is_empty() {
        Reading::Error("no windows in response".into())
    } else {
        Reading::Ok { windows, detail: None }
    }
}

impl Provider for MiniMax {
    fn id(&self) -> String { "minimax".into() }
    fn fidelity(&self) -> Fidelity { Fidelity::Official }
    fn is_present(&self) -> bool { self.key().is_some() }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: "minimax".into(),
            display_name: "MiniMax".into(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
            fetch_error: None,
        };
        let Some(key) = self.key() else { return make(Reading::NotConfigured) };
        let base = self.cfg.base_url.as_deref().unwrap_or("https://api.minimax.io");
        let url = format!("{base}/v1/token_plan/remains");
        let headers = [
            ("Authorization", format!("Bearer {key}")),
            ("Content-Type", "application/json".into()),
        ];
        match http_get_json(&url, &headers) {
            Ok(v) => make(parse(&v)),
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("invalid API key".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Shape of `GET /v1/token_plan/remains`, as observed.
    fn body() -> serde_json::Value {
        json!({
            "base_resp": {"status_code": 0, "status_msg": "success"},
            "model_remains": [{
                "model_name": "general",
                "current_interval_remaining_percent": 42.0,
                "current_interval_usage_count": 12,
                "current_interval_total_count": 30,
                "end_time": 1_789_003_600_000i64,
                "current_weekly_remaining_percent": 71.0,
                "current_weekly_total_count": 500,
                "weekly_end_time": 1_789_500_000_000i64
            }]
        })
    }

    fn windows(r: &Reading) -> &[Window] {
        match r {
            Reading::Ok { windows, .. } => windows,
            other => panic!("expected a reading, got {other:?}"),
        }
    }

    #[test]
    fn each_model_contributes_its_two_windows() {
        let r = parse(&body());
        let w = windows(&r);
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].label, "general 5h");
        assert_eq!(w[1].label, "general weekly");
    }

    #[test]
    fn counts_come_through_alongside_the_percentage() {
        let r = parse(&body());
        let w = &windows(&r)[0];
        assert_eq!(w.remaining_percent, Some(42.0));
        assert_eq!((w.remaining_count, w.total_count), (Some(12), Some(30)));
    }

    #[test]
    fn millisecond_timestamps_are_converted_to_seconds() {
        let r = parse(&body());
        assert_eq!(windows(&r)[0].resets_at, Some(1_789_003_600));
    }

    #[test]
    fn an_error_carried_inside_a_200_is_treated_as_an_auth_problem() {
        // This API reports failure in the body, not the status line — so a
        // bad key arrives looking like a perfectly good response.
        let r = parse(&json!({"base_resp": {"status_code": 1004, "status_msg": "invalid api key"}}));
        match r {
            Reading::NeedsAuth(m) => assert_eq!(m, "invalid api key"),
            other => panic!("expected NeedsAuth, got {other:?}"),
        }
    }

    #[test]
    fn a_shape_we_do_not_recognise_is_an_error_not_an_empty_reading() {
        let r = parse(&json!({"base_resp": {"status_code": 0}, "models": []}));
        assert!(matches!(r, Reading::Error(_)), "got {r:?}");
    }
}
