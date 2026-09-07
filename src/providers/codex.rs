use super::{home, http_get_json, read_json_file, Provider};
use crate::types::{Fidelity, Reading, Snapshot, Window, now_unix};

pub struct Codex;

impl Provider for Codex {
    fn id(&self) -> String { "codex".into() }
    fn fidelity(&self) -> Fidelity { Fidelity::Official }
    fn is_present(&self) -> bool { home().join(".codex/auth.json").exists() }

    fn snapshot(&self) -> Snapshot {
        let make = |reading| Snapshot {
            provider_id: "codex".into(),
            display_name: "ChatGPT / Codex".into(),
            fidelity: self.fidelity(),
            reading,
            fetched_at: now_unix(),
        };
        let Some(v) = read_json_file(&home().join(".codex/auth.json")) else {
            return make(Reading::NotConfigured);
        };
        let Some(token) = v.get("tokens").and_then(|t| t.get("access_token")).and_then(|s| s.as_str()) else {
            return make(Reading::NotConfigured);
        };
        let account_id = v.get("last_account").and_then(|a| a.get("account_id")).and_then(|s| s.as_str()).unwrap_or("");
        let headers = [
            ("Authorization", format!("Bearer {token}")),
            ("chatgpt-account-id", account_id.to_string()),
            ("User-Agent", "CodexBar/limitcue".into()),
        ];
        match http_get_json("https://chatgpt.com/backend-api/wham/usage", &headers) {
            Ok(j) => {
                let mut windows = Vec::new();
                let rl = j.get("rate_limit").cloned().unwrap_or_default();
                for (key, label) in [("primary_window", "5h"), ("secondary_window", "weekly")] {
                    if let Some(w) = rl.get(key) {
                        let used = w.get("used_percent").and_then(|p| p.as_f64());
                        let resets_in = w.get("resets_in_seconds").and_then(|s| s.as_u64());
                        if used.is_none() && resets_in.is_none() {
                            continue;
                        }
                        windows.push(Window {
                            label: label.into(),
                            remaining_percent: used.map(|u| (100.0 - u).clamp(0.0, 100.0)),
                            remaining_count: None,
                            total_count: None,
                            resets_at: resets_in.map(|s| now_unix() + s),
                        });
                    }
                }
                if windows.is_empty() {
                    make(Reading::Error("no windows in response".into()))
                } else {
                    make(Reading::Ok { windows, detail: None })
                }
            }
            Err(e) if e == "auth-failed" => make(Reading::NeedsAuth("run `codex login`".into())),
            Err(e) => make(Reading::Error(e)),
        }
    }
}
