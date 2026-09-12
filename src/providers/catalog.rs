//! The provider catalogue: what LimitCue knows how to track, and what each
//! entry needs from the user.
//!
//! Adding a provider used to mean knowing its endpoint, its auth header and
//! the shape of its JSON, then hand-editing `config.toml`. The catalogue turns
//! that into picking a name and pasting a key — while `Entry::Json` keeps the
//! escape hatch open for anything not listed yet.

use crate::config::{ProviderConfig, WindowConfig};
use crate::types::Fidelity;

/// How an entry gets its numbers, which decides what the UI has to ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A compiled-in adapter reading credentials the vendor's own CLI wrote.
    /// Nothing to configure — it either found the file or it did not.
    Cli(&'static str),
    /// A compiled-in adapter that needs an API key.
    Keyed,
    /// A New-API style billing gateway: base URL plus key.
    Billing,
    /// A compiled-in adapter reading a CLI config directory the user names,
    /// which is how a second login to the same provider is tracked.
    CliProfile,
    /// A JSON endpoint mapped by dot-paths.
    Json,
}

pub struct PresetWindow {
    pub label: &'static str,
    pub remaining_count_path: Option<&'static str>,
    pub total_count_path: Option<&'static str>,
    pub remaining_path: Option<&'static str>,
    pub resets_at_path: Option<&'static str>,
}

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// One line on what this tracks, shown under the name in the picker.
    pub blurb: &'static str,
    pub source: Source,
    pub fidelity: Fidelity,
    /// Where to go to get a key. Empty when the entry needs none.
    pub key_hint: &'static str,
    /// The environment variable people conventionally keep this key in, which
    /// is what `limitcue init` looks for. Empty when there is no convention.
    pub key_env: &'static str,
    pub base_url: &'static str,
    pub url: &'static str,
    /// Compiled-in adapter to run, when it is not implied by the id.
    pub adapter: &'static str,
    /// Default CLI config directory for a `CliProfile` entry.
    pub credentials_dir: &'static str,
    /// The provider's dashboard. Kept conservative — a landing page that is
    /// certainly right beats a deep link that might not be — and overridable
    /// per entry in config.
    pub console_url: &'static str,
    pub auth_header: &'static str,
    pub windows: &'static [PresetWindow],
}

impl Preset {
    /// Is it configured by toggling rather than by adding a config entry?
    pub fn is_built_in(&self) -> bool {
        matches!(self.source, Source::Cli(_))
    }

    /// The `[[provider]]` entry this preset stands for, ready for a key.
    pub fn to_config(&self) -> ProviderConfig {
        ProviderConfig {
            id: self.id.into(),
            name: self.name.into(),
            url: (!self.url.is_empty()).then(|| self.url.into()),
            auth_header: (!self.auth_header.is_empty()).then(|| self.auth_header.into()),
            base_url: (!self.base_url.is_empty()).then(|| self.base_url.into()),
            key_hint: (!self.key_hint.is_empty()).then(|| self.key_hint.into()),
            key_env: (!self.key_env.is_empty()).then(|| self.key_env.into()),
            billing: self.source == Source::Billing,
            adapter: (!self.adapter.is_empty()).then(|| self.adapter.into()),
            credentials_dir: (!self.credentials_dir.is_empty())
                .then(|| self.credentials_dir.into()),
            console_url: (!self.console_url.is_empty()).then(|| self.console_url.into()),
            enabled: Some(true),
            windows: self
                .windows
                .iter()
                .map(|w| WindowConfig {
                    label: w.label.into(),
                    remaining_path: w.remaining_path.map(Into::into),
                    remaining_count_path: w.remaining_count_path.map(Into::into),
                    total_count_path: w.total_count_path.map(Into::into),
                    resets_at_path: w.resets_at_path.map(Into::into),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }
}

const NONE_W: &[PresetWindow] = &[];

/// Everything the app can track. Order is the order the picker shows.
///
/// Entries only claim an endpoint that is actually published; anything else
/// belongs behind `Source::Json`, where the user supplies the paths and the
/// reading is labelled `manual` rather than pretending to be official.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "claude",
        name: "Claude",
        blurb: "Session and weekly limits, read from the Claude Code login on this machine.",
        source: Source::Cli("~/.claude/.credentials.json"),
        fidelity: Fidelity::Official,
        key_hint: "",
        key_env: "",
        base_url: "",
        url: "",
        adapter: "",
        credentials_dir: "",
        console_url: "https://claude.ai/settings/usage",
        auth_header: "",
        windows: NONE_W,
    },
    Preset {
        id: "codex",
        name: "ChatGPT / Codex",
        blurb: "Rolling limits, read from the Codex CLI login on this machine.",
        source: Source::Cli("~/.codex/auth.json"),
        fidelity: Fidelity::Official,
        key_hint: "",
        key_env: "",
        base_url: "",
        url: "",
        adapter: "",
        credentials_dir: "",
        console_url: "https://chatgpt.com/",
        auth_header: "",
        windows: NONE_W,
    },
    Preset {
        id: "grok",
        name: "Grok",
        blurb: "Monthly spend cap, read from the Grok Build login on this machine.",
        source: Source::Cli("~/.grok/auth.json"),
        fidelity: Fidelity::Derived,
        key_hint: "",
        key_env: "XAI_API_KEY",
        base_url: "",
        url: "",
        adapter: "",
        credentials_dir: "",
        console_url: "https://console.x.ai/team/default/billing",
        auth_header: "",
        windows: NONE_W,
    },
    Preset {
        id: "kimi",
        name: "Kimi",
        blurb: "Coding-plan limits. Unofficial endpoint — it can change without notice.",
        source: Source::Cli("~/.kimi"),
        fidelity: Fidelity::Derived,
        key_hint: "",
        key_env: "",
        base_url: "",
        url: "",
        adapter: "",
        credentials_dir: "",
        console_url: "",
        auth_header: "",
        windows: NONE_W,
    },
    Preset {
        id: "minimax",
        name: "MiniMax",
        blurb: "Token-plan remains for the coding plan. Use api.minimaxi.com for CN accounts.",
        source: Source::Keyed,
        fidelity: Fidelity::Official,
        key_hint: "MiniMax console → API keys (a key starting sk-cp-)",
        key_env: "MINIMAX_API_KEY",
        base_url: "https://api.minimax.io",
        url: "",
        adapter: "",
        credentials_dir: "",
        console_url: "https://platform.minimax.io/",
        auth_header: "",
        windows: NONE_W,
    },
    Preset {
        id: "openrouter",
        name: "OpenRouter",
        blurb: "Credits left on the key, from OpenRouter's own key endpoint.",
        source: Source::Json,
        fidelity: Fidelity::Official,
        key_hint: "openrouter.ai/settings/keys",
        key_env: "OPENROUTER_API_KEY",
        base_url: "",
        url: "https://openrouter.ai/api/v1/auth/key",
        adapter: "",
        credentials_dir: "",
        console_url: "https://openrouter.ai/settings/credits",
        auth_header: "Authorization: Bearer {key}",
        windows: &[PresetWindow {
            label: "credits",
            remaining_count_path: Some("data.limit_remaining"),
            total_count_path: Some("data.limit"),
            remaining_path: None,
            resets_at_path: None,
        }],
    },
    Preset {
        id: "agentrouter",
        name: "AgentRouter",
        blurb: "Balance against the plan cap, over the legacy billing endpoints.",
        source: Source::Billing,
        fidelity: Fidelity::Official,
        key_hint: "AgentRouter dashboard → API tokens",
        key_env: "AGENTROUTER_API_KEY",
        base_url: "https://agentrouter.org/v1",
        url: "",
        adapter: "",
        credentials_dir: "",
        console_url: "https://agentrouter.org/",
        auth_header: "",
        windows: NONE_W,
    },
    Preset {
        id: "gateway",
        name: "New-API gateway",
        blurb: "Any router exposing /dashboard/billing. Give it the base URL and a key.",
        source: Source::Billing,
        fidelity: Fidelity::Official,
        key_hint: "the gateway's own dashboard",
        key_env: "",
        base_url: "",
        url: "",
        adapter: "billing",
        credentials_dir: "",
        console_url: "",
        auth_header: "",
        windows: NONE_W,
    },
    Preset {
        id: "claude-account",
        name: "Claude — another account",
        blurb: "A second login living in its own CLI config directory.",
        source: Source::CliProfile,
        fidelity: Fidelity::Official,
        key_hint: "",
        key_env: "",
        base_url: "",
        url: "",
        auth_header: "",
        adapter: "claude",
        credentials_dir: "~/.claude-work",
        console_url: "https://claude.ai/settings/usage",
        windows: NONE_W,
    },
    Preset {
        id: "codex-account",
        name: "Codex — another account",
        blurb: "A second Codex login from a different config directory.",
        source: Source::CliProfile,
        fidelity: Fidelity::Official,
        key_hint: "",
        key_env: "",
        base_url: "",
        url: "",
        auth_header: "",
        adapter: "codex",
        credentials_dir: "~/.codex-work",
        console_url: "https://chatgpt.com/",
        windows: NONE_W,
    },
    Preset {
        id: "grok-account",
        name: "Grok — another account",
        blurb: "A second Grok Build login from a different config directory.",
        source: Source::CliProfile,
        fidelity: Fidelity::Derived,
        key_hint: "",
        key_env: "",
        base_url: "",
        url: "",
        auth_header: "",
        adapter: "grok",
        credentials_dir: "~/.grok-work",
        console_url: "https://console.x.ai/team/default/billing",
        windows: NONE_W,
    },
    Preset {
        id: "custom",
        name: "Anything with a JSON endpoint",
        blurb: "Point it at a URL and say which fields hold the numbers.",
        source: Source::Json,
        fidelity: Fidelity::Manual,
        key_hint: "",
        key_env: "",
        base_url: "",
        url: "",
        adapter: "",
        credentials_dir: "",
        console_url: "",
        auth_header: "Authorization: Bearer {key}",
        windows: NONE_W,
    },
];

/// Entries that are configured by toggling rather than by being added.
pub fn built_ins() -> impl Iterator<Item = &'static Preset> {
    PRESETS.iter().filter(|p| p.is_built_in())
}

/// Entries a user can add, minus the ones already in their config.
pub fn addable(existing: &[ProviderConfig]) -> Vec<&'static Preset> {
    PRESETS
        .iter()
        .filter(|p| !p.is_built_in())
        .filter(|p| {
            matches!(p.source, Source::CliProfile)
                || p.id == "custom"
                || p.id == "gateway"
                || !existing.iter().any(|e| e.id == p.id)
        })
        .collect()
}

/// What `limitcue init` found on this machine.
pub struct Found {
    pub preset: &'static Preset,
    /// Why it counts as present: a credential file, or a key in the env.
    pub because: String,
    /// Already in the config, so nothing to add.
    pub already: bool,
}

/// Look for providers this machine is already signed in to.
///
/// Deliberately conservative: it reports a credential file existing, or a key
/// sitting in the environment. It never opens a credential file, and it never
/// guesses a URL for something it cannot see.
pub fn detect(cfg: &crate::config::Config) -> Vec<Found> {
    let home = crate::providers::home();
    let mut out = Vec::new();
    for p in PRESETS {
        let already = match p.source {
            Source::Cli(_) => !cfg.disabled.iter().any(|d| *d == *p.id),
            _ => cfg.provider.iter().any(|e| e.id == p.id),
        };
        let because = match p.source {
            Source::Cli(path) => {
                let real = home.join(path.trim_start_matches("~/"));
                if real.exists() {
                    format!("{path} is here")
                } else {
                    continue;
                }
            }
            _ if !p.key_env.is_empty() && std::env::var(p.key_env).is_ok() => {
                format!("${} is set", p.key_env)
            }
            _ => continue,
        };
        out.push(Found { preset: p, because, already });
    }
    out
}
