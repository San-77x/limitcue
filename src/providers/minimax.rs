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
        };
        let Some(key) = self.key() else { return make(Reading::NotConfigured) };
        let base = self.cfg.base_url.as_deref().unwrap_or("https://api.minimax.io");
        let url = format!("{base}/v1/token_plan/remains");
        let headers = [
            ("Authorization", format!("Bearer {key}")),
            ("Content-Type", "application/json".into()),
        ];
        match http_get_json(&url, &headers) {
            Ok(v) => {
                if json_path(&v, "base_resp.status_code").and_then(|c| c.as_i64()).unwrap_or(0) != 0 {
                    let msg = json_path(&v, "base_resp.status_msg").and_then(|m| m.as_str()).unwrap_or("");
                    return make(Reading::NeedsAuth(msg.into()));
                }
                let mut windows = Vec::new();
                if let Some(list) = json_path(&v, "model_remains").and_then(|m| m.as_array()) {
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
                    make(Reading::Error("no windows in response".into()))
                } else {
                    make(Reading::Ok { windows, detail: None })
                }
            }
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("invalid API key".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}
