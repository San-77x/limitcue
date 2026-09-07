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
        };
        let Some(key) = self.key() else { return make(Reading::NotConfigured) };
        let Some(base) = &self.cfg.base_url else { return make(Reading::NotConfigured) };
        let auth = ("Authorization", format!("Bearer {key}"));
        let sub = match http_get_json(&format!("{base}/dashboard/billing/subscription"), &[auth.clone()]) {
            Ok(v) => v,
            Err(e) if e == "auth-failed" => return make(Reading::NeedsAuth("invalid API key".into())),
            Err(e) => return make(Reading::Error(e)),
        };
        let usage = match http_get_json(&format!("{base}/dashboard/billing/usage"), &[auth]) {
            Ok(v) => v,
            Err(e) => return make(Reading::Error(e)),
        };
        let Some(limit) = sub.get("hard_limit_usd").and_then(|x| x.as_f64()).filter(|l| *l > 0.0) else {
            return make(Reading::Error("no hard_limit_usd in subscription".into()));
        };
        let Some(raw) = usage.get("total_usage").and_then(|x| x.as_f64()) else {
            return make(Reading::Error("no total_usage in response".into()));
        };
        // total_usage is cents on New-API deployments; sanity-check the heuristic
        let used_usd = if raw > limit * 10.0 { raw / 100.0 } else { raw };
        let remaining = ((limit - used_usd) / limit * 100.0).clamp(0.0, 100.0);
        let access_until = sub.get("access_until").and_then(|x| x.as_f64()).filter(|t| *t > 1e9).map(|t| t as u64);
        make(Reading::Ok {
            windows: vec![Window {
                label: format!("quota ${:.2} of ${:.0}", limit - used_usd.max(0.0), limit),
                remaining_percent: Some(remaining),
                remaining_count: None,
                total_count: None,
                resets_at: access_until,
            }],
            detail: Some(format!("${used_usd:.2} used")),
        })
    }
}
