use super::{http_get_json, json_path, Provider};
use crate::config::ProviderConfig;
use crate::types::{Fidelity, Reading, Snapshot, Window, now_unix};

/// A user-declared provider: point it at any JSON usage endpoint and map fields
/// with dot-paths. This is what lets LimitCue cover "all available providers"
/// (Kimi, OpenCode Go, GLM, ...) without waiting for first-party adapters.
///
/// Config example (config.toml):
///
/// ```toml
/// [[provider]]
/// id = "kimi"
/// name = "Kimi"
/// url = "https://api.kimi.com/coding/v1/usage"
/// auth_header = "Authorization: Bearer {key}"
/// key_env = "KIMI_API_KEY"
/// windows = [
///   { label = "5h",     remaining_path = "usage.limit_remaining" },
///   { label = "weekly", remaining_path = "usage.week_remaining", resets_at_path = "usage.week_reset_at" },
/// ]
/// ```
pub struct Custom {
    cfg: ProviderConfig,
}

impl Custom {
    pub fn new(cfg: ProviderConfig) -> Self { Self { cfg } }

    fn resolve_key(&self) -> Option<String> {
        if let Some(k) = &self.cfg.api_key {
            return Some(k.clone());
        }
        self.cfg.key_env.as_deref().and_then(|e| std::env::var(e).ok())
    }
}

impl Provider for Custom {
    fn id(&self) -> String { self.cfg.id.clone() }
    fn fidelity(&self) -> Fidelity { Fidelity::Manual }
    fn is_present(&self) -> bool { self.resolve_key().is_some() }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: self.cfg.id.clone(),
            display_name: self.cfg.name.clone(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
        };
        let Some(url) = &self.cfg.url else { return make(Reading::NotConfigured) };
        let mut headers: Vec<(String, String)> = Vec::new();
        if let Some(ah) = &self.cfg.auth_header {
            let (name, value) = ah.split_once(':').unwrap_or((ah, ""));
            let value = value.trim().replace("{key}", self.resolve_key().as_deref().unwrap_or(""));
            headers.push((name.trim().to_string(), value));
        }
        let refs: Vec<(&str, String)> = headers.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        match http_get_json(url, &refs) {
            Ok(v) => {
                let mut windows = Vec::new();
                for w in &self.cfg.windows {
                    let remaining = w.remaining_path.as_deref().and_then(|p| json_path(&v, p)).and_then(|x| x.as_f64());
                    let resets = w.resets_at_path.as_deref().and_then(|p| json_path(&v, p)).and_then(|x| {
                        x.as_u64().or_else(|| x.as_f64().map(|f| f as u64))
                    });
                    if remaining.is_some() || resets.is_some() {
                        windows.push(Window {
                            label: w.label.clone(),
                            remaining_percent: remaining,
                            remaining_count: None,
                            total_count: None,
                            resets_at: resets,
                        });
                    }
                }
                if windows.is_empty() {
                    make(Reading::Error("configured paths matched nothing".into()))
                } else {
                    make(Reading::Ok { windows, detail: None })
                }
            }
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("check api key / auth_header".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}
