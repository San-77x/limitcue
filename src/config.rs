use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// One quota window mapped out of a provider's JSON response.
///
/// Real APIs report quota in every shape there is, so rather than demanding a
/// ready-made percentage the mapping accepts whichever pair of values the
/// endpoint actually returns and derives the rest:
///
/// | endpoint gives            | set                                        |
/// |---------------------------|--------------------------------------------|
/// | a percentage 0-100        | `remaining_path`                           |
/// | a fraction 0-1            | `remaining_fraction_path`                  |
/// | "12 of 30 left"           | `remaining_count_path` + `total_count_path`|
/// | "18 of 30 used"           | `used_count_path` + `total_count_path`     |
/// | a balance with no ceiling | `remaining_count_path` + `total_const`     |
///
/// Numbers may arrive as JSON numbers or as strings; timestamps as unix
/// seconds, unix milliseconds, or RFC3339.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowConfig {
    pub label: String,
    /// Percentage remaining, 0-100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_path: Option<String>,
    /// Fraction remaining, 0-1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_fraction_path: Option<String>,
    /// Absolute units left, e.g. requests or dollars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_count_path: Option<String>,
    /// Absolute units spent — the inverse of `remaining_count_path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_count_path: Option<String>,
    /// The ceiling those absolute units are measured against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_count_path: Option<String>,
    /// A ceiling the endpoint does not report — a plan size you know yourself.
    /// This is what turns a bare balance into a gauge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_const: Option<f64>,
    /// Unix seconds (or millis, or RFC3339) when the window refills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at_path: Option<String>,
    /// Seconds from now until it refills, for APIs that report a countdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_in_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub url: Option<String>,
    pub auth_header: Option<String>,
    /// Extra request headers, `Name: value`, with `{key}` substituted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<String>,
    pub key_env: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    /// Where to send someone to fetch the key. Set by the provider catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_hint: Option<String>,
    #[serde(default)]
    pub windows: Vec<WindowConfig>,
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Use legacy OpenAI billing endpoints (New-API style gateways).
    #[serde(default)]
    pub billing: bool,
    /// Display order (lower = earlier). Absent = file order.
    #[serde(default)]
    pub priority: Option<u32>,
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
    /// UI theme: midnight | tokyo-night | catppuccin | gruvbox.
    #[serde(default)]
    pub theme: String,
    /// Providers shown on the collapsed pill before the rest folds into "+N".
    #[serde(default = "default_max_visible")]
    pub max_visible_collapsed: usize,
    /// Reduce idle rail/pill opacity until the pointer is over the surface.
    #[serde(default)]
    pub quiet_mode: bool,
    /// Show used percentage text beneath gauges in the side rail.
    #[serde(default)]
    pub show_rail_percent: bool,
    /// Opacity of the notch body, 0..1. The desktop shows through below 1.
    #[serde(default = "default_notch_opacity")]
    pub notch_opacity: f32,
    /// Opacity of the hover usage card, 0..1.
    #[serde(default = "default_card_opacity")]
    pub card_opacity: f32,
}

fn default_poll() -> u64 { 120 }
/// Defaults reproduce the surfaces' previously hard-coded translucency.
fn default_notch_opacity() -> f32 { 0.60 }
fn default_card_opacity() -> f32 { 0.70 }
fn default_true() -> bool { true }
fn default_max_visible() -> usize { 4 }

impl Default for Config {
    fn default() -> Self {
        Self {
            poll_interval_secs: default_poll(),
            hide_unconfigured: true,
            disabled: vec![],
            provider: vec![],
            theme: String::new(),
            max_visible_collapsed: default_max_visible(),
            quiet_mode: false,
            show_rail_percent: false,
            notch_opacity: default_notch_opacity(),
            card_opacity: default_card_opacity(),
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

    /// Persist the current config back to config.toml (settings screen Apply).
    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match toml::to_string_pretty(self) {
            Ok(s) => {
                let _ = std::fs::write(&p, s);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
                }
            }
            Err(e) => eprintln!("limitcue: could not save config: {e}"),
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
# theme = "midnight"             # midnight | tokyo-night | catppuccin | gruvbox
# max_visible_collapsed = 4      # providers on the pill before folding into "+N"
# quiet_mode = false              # dim idle UI until the pointer is over it
# show_rail_percent = false       # show used percentages beneath rail gauges
# notch_opacity = 0.60            # notch body opacity, 0.15-1.0
# card_opacity = 0.70             # hover usage card opacity, 0.15-1.0

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

# New-API style gateways with legacy billing endpoints:
[[provider]]
id = "agentrouter"
name = "AgentRouter"
base_url = "https://agentrouter.org/v1"
billing = true
# api_key = "sk-..."
key_env = "AGENTROUTER_API_KEY"

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
        let _ = std::fs::write(&p, example);
        // Keep provider config user-only (0600) in case users add inline API keys.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
        }
    }
}
