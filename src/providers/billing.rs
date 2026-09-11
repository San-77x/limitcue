use super::{http_get_json, Provider};
use crate::config::ProviderConfig;
use crate::types::{Fidelity, Reading, Snapshot, Window, now_unix};

/// Adapter for New-API / legacy-OpenAI style gateways (AgentRouter, many
/// router-as-a-service offerings) that expose:
///   GET {base}/v1/dashboard/billing/subscription  -> hard_limit_usd
///   GET {base}/v1/dashboard/billing/usage         -> total_usage (cents)
/// Config: [[provider]] id/name/base_url/key_env + `billing = true`.
pub struct Billing {
    cfg: ProviderConfig,
}

impl Billing {
    pub fn new(cfg: ProviderConfig) -> Self { Self { cfg } }

    fn key(&self) -> Option<String> {
        self.cfg.api_key.clone().or_else(|| {
            self.cfg
                .key_env
                .as_deref()
                .and_then(|e| std::env::var(e).ok())
                .or_else(|| std::env::var("AGENTROUTER_API_KEY").ok())
        })
    }
}

/// Turn the two billing responses into a reading.
///
/// Split out from the requests so the shapes can be pinned by tests. The
/// cents-or-dollars heuristic below is a guess about somebody else's API, and
/// a guess is exactly the thing worth having tests for.
fn parse(sub: &serde_json::Value, usage: &serde_json::Value) -> Reading {
    let Some(limit) = sub.get("hard_limit_usd").and_then(|x| x.as_f64()).filter(|l| *l > 0.0) else {
        return Reading::Error("no hard_limit_usd in subscription".into());
    };
    let Some(raw) = usage.get("total_usage").and_then(|x| x.as_f64()) else {
        return Reading::Error("no total_usage in response".into());
    };
    // total_usage is cents on New-API deployments; sanity-check the heuristic
    let used_usd = if raw > limit * 10.0 { raw / 100.0 } else { raw };
    let remaining = ((limit - used_usd) / limit * 100.0).clamp(0.0, 100.0);
    let access_until = sub.get("access_until").and_then(|x| x.as_f64()).filter(|t| *t > 1e9).map(|t| t as u64);
    Reading::Ok {
        windows: vec![Window {
            label: format!("quota ${:.2} of ${:.0}", limit - used_usd.max(0.0), limit),
            remaining_percent: Some(remaining),
            remaining_count: None,
            total_count: None,
            resets_at: access_until,
        }],
        detail: Some(format!("${used_usd:.2} used")),
    }
}

impl Provider for Billing {
    fn id(&self) -> String { self.cfg.id.clone() }
    fn fidelity(&self) -> Fidelity { Fidelity::Official }
    fn is_present(&self) -> bool { self.cfg.base_url.is_some() && self.key().is_some() }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: self.cfg.id.clone(),
            display_name: self.cfg.name.clone(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
            fetch_error: None,
        };
        let Some(key) = self.key() else { return make(Reading::NotConfigured) };
        let Some(base) = &self.cfg.base_url else { return make(Reading::NotConfigured) };
        let auth = [("Authorization", format!("Bearer {key}"))];
        let sub = match http_get_json(&format!("{base}/dashboard/billing/subscription"), &auth) {
            Ok(v) => v,
            Err(e) if e == "auth-failed" => return make(Reading::NeedsAuth("invalid API key".into())),
            Err(e) => return make(Reading::Error(e)),
        };
        let usage = match http_get_json(&format!("{base}/dashboard/billing/usage"), &auth) {
            Ok(v) => v,
            Err(e) => return make(Reading::Error(e)),
        };
        make(parse(&sub, &usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn window0(r: &Reading) -> &Window {
        match r {
            Reading::Ok { windows, .. } => &windows[0],
            other => panic!("expected a reading, got {other:?}"),
        }
    }

    #[test]
    fn usage_in_cents_is_recognised_as_cents() {
        // New-API deployments report cents; $30.46 spent of a $50 cap.
        let r = parse(&json!({"hard_limit_usd": 50.0}), &json!({"total_usage": 3046.0}));
        assert_eq!(window0(&r).remaining_percent, Some(39.08));
        assert_eq!(window0(&r).label, "quota $19.54 of $50");
    }

    #[test]
    fn usage_already_in_dollars_is_left_alone() {
        // The heuristic is "too large to be dollars"; $12.50 of $50 is not.
        let r = parse(&json!({"hard_limit_usd": 50.0}), &json!({"total_usage": 12.5}));
        assert_eq!(window0(&r).remaining_percent, Some(75.0));
    }

    #[test]
    fn spending_past_the_cap_reads_as_empty_not_as_negative() {
        let r = parse(&json!({"hard_limit_usd": 10.0}), &json!({"total_usage": 5000.0}));
        assert_eq!(window0(&r).remaining_percent, Some(0.0));
    }

    #[test]
    fn an_access_expiry_becomes_the_reset_time() {
        let r = parse(
            &json!({"hard_limit_usd": 50.0, "access_until": 1_789_500_000.0}),
            &json!({"total_usage": 100.0}),
        );
        assert_eq!(window0(&r).resets_at, Some(1_789_500_000));
    }

    #[test]
    fn a_gateway_reporting_no_cap_says_so_rather_than_guessing_one() {
        let r = parse(&json!({}), &json!({"total_usage": 100.0}));
        assert!(matches!(r, Reading::Error(_)), "got {r:?}");
    }
}
