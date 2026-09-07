use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub label: String,
    pub remaining_path: Option<String>,
    pub resets_at_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub url: Option<String>,
    pub auth_header: Option<String>,
    pub key_env: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    #[serde(default)]
    pub windows: Vec<WindowConfig>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Seconds between polls when everything is healthy.
    #[serde(default = "default_poll")]
    pub poll_interval_secs: u64,
    /// Hide providers with no data at all (not installed, not configured).
    #[serde(default = "default_true")]
    pub hide_unconfigured: bool,
    /// Providers disabled by id.
    #[serde(default)]
    pub disabled: Vec<String>,
    /// Extra user-declared providers.
    #[serde(default)]
    pub provider: Vec<ProviderConfig>,
}

fn default_poll() -> u64 { 120 }
fn default_true() -> bool { true }

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll(),
            hide_unconfigured: true,
            disabled: vec![],
            provider: vec![],
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("limitcue").join("config.toml")
    }

    pub fn load() -> Self {
        match std::fs::read_to_string(Self::path()) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                eprintln!("limitcue: bad config ({}): {e}, using defaults", Self::path().display());
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    pub fn write_example_if_missing() {
        let p = Self::path();
        if p.exists() {
            return;
        }
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let example = r#"# LimitCue configuration
poll_interval_secs = 120
hide_unconfigured = true
# disabled = ["codex"]

# Built-in adapters read credentials from disk automatically:
#   claude  -> ~/.claude/.credentials.json
#   codex   -> ~/.codex/auth.json
#   minimax -> provide the token-plan key below (or MINIMAX_API_KEY env)

[[provider]]
id = "minimax"
name = "MiniMax"
base_url = "https://api.minimax.io"        # use https://api.minimaxi.com for CN plans
# api_key = "sk-cp-..."
key_env = "MINIMAX_API_KEY"

# User-defined providers: point at any JSON usage endpoint.
# [[provider]]
# id = "kimi"
# name = "Kimi"
# url = "https://api.kimi.com/coding/v1/usage"
# auth_header = "Authorization: Bearer {key}"
# key_env = "KIMI_API_KEY"
# windows = [
#   { label = "5h",     remaining_path = "usage.limit_remaining" },
#   { label = "weekly", remaining_path = "usage.week_remaining", resets_at_path = "usage.week_reset_at" },
# ]
"#;
        let _ = std::fs::write(p, example);
    }
}
