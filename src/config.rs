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
    /// The provider's own dashboard, opened from the usage card. The
    /// catalogue supplies a sensible default; override it here when a
    /// provider moves its console.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub console_url: Option<String>,
    /// Which compiled-in adapter to run: "claude", "codex", "grok", "kimi",
    /// "minimax", "billing", or "json". Absent means "guess from the id",
    /// which is what every config written before this key did.
    ///
    /// Naming it explicitly is what lets one machine track two logins to the
    /// same provider: two entries, different ids, same adapter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// Read this login's credentials from here instead of the default
    /// location, e.g. "~/.claude-work" alongside "~/.claude".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials_dir: Option<String>,
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
    /// UI theme: midnight, paper, acid, prism, slate, neon, sakura, mecha,
    /// pitch, arcade. Retired
    /// names (flux, ember, aurora, chrome, tokyo-night, catppuccin, gruvbox)
    /// still load, mapped to their nearest survivor.
    #[serde(default)]
    pub theme: String,
    /// Providers shown on the collapsed pill before the rest folds into "+N".
    #[serde(default = "default_max_visible")]
    pub max_visible_collapsed: usize,
    /// Dim the notch's *contents* until the pointer is over it. The panel
    /// keeps whatever `notch_opacity` says: being quiet means drawing less
    /// attention, not turning into a window onto the desktop.
    #[serde(default)]
    pub quiet_mode: bool,
    /// Show used percentage text beneath gauges in the side rail.
    #[serde(default)]
    pub show_rail_percent: bool,
    /// Hide the notch while the session is idle or locked, and bring it back
    /// on activity. Off by default: a quota pill that quietly disappears is
    /// surprising unless you asked for it.
    #[serde(default)]
    pub hide_when_idle: bool,
    /// Opacity of the notch body, 0..1. The desktop shows through below 1.
    #[serde(default = "default_notch_opacity")]
    pub notch_opacity: f32,
    /// Opacity of the hover usage card, 0..1.
    #[serde(default = "default_card_opacity")]
    pub card_opacity: f32,
    /// Send a desktop notification when a window runs low.
    #[serde(default)]
    pub notify: bool,
    /// Percent remaining at or below which the alert fires.
    #[serde(default = "default_threshold")]
    pub notify_threshold: f64,
    /// Also announce when a window refills.
    #[serde(default = "default_true")]
    pub notify_on_reset: bool,
    /// Shell command run when a window goes low. Details arrive in the
    /// environment (LIMITCUE_PROVIDER / _WINDOW / _PERCENT), never spliced
    /// into the command line.
    #[serde(default)]
    pub cmd_on_low: String,
    /// Shell command run when a window refills.
    #[serde(default)]
    pub cmd_on_reset: String,
    /// Row order in the notch: "manual" keeps config order, "urgency" puts
    /// whatever is closest to running out at the top.
    #[serde(default)]
    pub sort_by_urgency: bool,
    /// Show what the current burn rate means on the hover card.
    ///
    /// Samples are kept in `history.json` — percentages and timestamps only,
    /// the same posture as `state.json` — so a projection is available at the
    /// first hover rather than several minutes into every session. Turning
    /// this off deletes that file as well as hiding the line.
    #[serde(default = "default_true")]
    pub projections: bool,
}

/// What the poll interval is set to unless the config says otherwise, and the
/// point below which the settings screen starts warning.
///
/// Quota endpoints are not free to call. Anthropic's returns 429 at a 30s
/// interval; 240s is the spacing this machine's history shows succeeding every
/// time, so it is the one value we have evidence for rather than a guess. It
/// costs nothing in freshness either: the shortest window any adapter reports
/// is five hours, so even here we ask 75 times per window for a number that
/// moves once.
pub const DEFAULT_POLL_SECS: u64 = 240;

/// Hard floor. Below a minute the app is generating more load than signal
/// against every provider at once, and no window it tracks moves that fast.
/// Between this and [`DEFAULT_POLL_SECS`] is the caller's risk to take.
pub const MIN_POLL_SECS: u64 = 60;

fn default_poll() -> u64 {
    DEFAULT_POLL_SECS
}
/// Defaults reproduce the surfaces' previously hard-coded translucency.
fn default_notch_opacity() -> f32 {
    0.60
}
fn default_card_opacity() -> f32 {
    0.70
}
fn default_threshold() -> f64 {
    15.0
}
fn default_true() -> bool {
    true
}
fn default_max_visible() -> usize {
    4
}

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
            hide_when_idle: false,
            notch_opacity: default_notch_opacity(),
            card_opacity: default_card_opacity(),
            notify: false,
            notify_threshold: default_threshold(),
            notify_on_reset: true,
            cmd_on_low: String::new(),
            cmd_on_reset: String::new(),
            sort_by_urgency: false,
            projections: true,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("limitcue")
            .join("config.toml")
    }

    /// When the file was last written, for spotting edits made outside the app.
    pub fn mtime() -> Option<std::time::SystemTime> {
        std::fs::metadata(Self::path()).ok()?.modified().ok()
    }

    pub fn load() -> Self {
        let mut c = match std::fs::read_to_string(Self::path()) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                eprintln!(
                    "limitcue: bad config ({}): {e}, using defaults",
                    Self::path().display()
                );
                Config::default()
            }),
            Err(_) => Config::default(),
        };
        // Clamp rather than trust the file: the interval used to be settable
        // down to 30s, and a config written back then still says so.
        if c.poll_interval_secs < MIN_POLL_SECS {
            c.poll_interval_secs = MIN_POLL_SECS;
        }
        c
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
poll_interval_secs = 240         # seconds between checks; below this, providers
                                 # may answer 429 instead of a number (min 60)
hide_unconfigured = true
# disabled = ["codex"]
# theme = "midnight"             # midnight paper acid prism slate neon
#                                 # sakura mecha pitch arcade
# max_visible_collapsed = 4      # providers on the pill before folding into "+N"
# quiet_mode = false              # dim the gauges until you point at the notch
# show_rail_percent = false       # show used percentages beneath rail gauges
# hide_when_idle = false          # hide the notch while the session is idle/locked
# notch_opacity = 0.60            # notch body opacity, 0.15-1.0
# card_opacity = 0.70             # hover usage card opacity, 0.15-1.0
# notify = true                   # desktop alert when a window runs low
# notify_threshold = 15           # ...at or below this percent remaining
# notify_on_reset = true          # and announce when it refills
# cmd_on_low = "paplay /usr/share/sounds/freedesktop/stereo/dialog-warning.oga"
# cmd_on_reset = ""               # $LIMITCUE_PROVIDER / _WINDOW / _PERCENT
# projections = true              # "at this pace it runs out 40m before reset"
#                                 # keeps history.json: percentages + times only
# sort_by_urgency = false         # put whatever is closest to empty at the top

# Built-in adapters read credentials from disk automatically:
#   claude  -> ~/.claude/.credentials.json
#   codex   -> ~/.codex/auth.json
#   grok    -> ~/.grok/auth.json (or XAI_API_KEY)
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

# Any JSON usage endpoint can be a provider. Settings → Providers → Add does
# all of this for you, including a Test button; this is the same thing by hand.
#
# [[provider]]
# id = "openrouter"
# name = "OpenRouter"
# url = "https://openrouter.ai/api/v1/auth/key"
# auth_header = "Authorization: Bearer {key}"
# key_env = "OPENROUTER_API_KEY"
# [[provider.windows]]
# label = "credits"
# remaining_count_path = "data.limit_remaining"   # units left
# total_count_path     = "data.limit"             # ...of this many
#
# A window takes whichever pair of values the endpoint reports and derives the
# rest. Pick one of:
#   remaining_path            a percentage, 0-100
#   remaining_fraction_path   a fraction, 0-1
#   remaining_count_path  + total_count_path    "12 of 30 left"
#   used_count_path       + total_count_path    "18 of 30 used"
#   remaining_count_path  + total_const = 50    a balance against a cap you know
# and optionally one of:
#   resets_at_path            unix seconds, unix millis, or an RFC3339 string
#   resets_in_path            seconds from now
# Values may be JSON numbers or strings. Paths are dot-paths with [n] indices.
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
