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
    pub base_url: &'static str,
    pub url: &'static str,
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
            billing: self.source == Source::Billing,
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
        base_url: "",
        url: "",
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
        base_url: "",
        url: "",
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
        base_url: "",
        url: "",
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
        base_url: "https://api.minimax.io",
        url: "",
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
        base_url: "",
        url: "https://openrouter.ai/api/v1/auth/key",
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
        base_url: "https://agentrouter.org/v1",
        url: "",
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
        base_url: "",
        url: "",
        auth_header: "",
        windows: NONE_W,
    },
    Preset {
        id: "custom",
        name: "Anything with a JSON endpoint",
        blurb: "Point it at a URL and say which fields hold the numbers.",
        source: Source::Json,
        fidelity: Fidelity::Manual,
        key_hint: "",
        base_url: "",
        url: "",
        auth_header: "Authorization: Bearer {key}",
        windows: NONE_W,
    },
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Entries that are configured by toggling rather than by being added.
pub fn built_ins() -> impl Iterator<Item = &'static Preset> {
    PRESETS.iter().filter(|p| p.is_built_in())
}

/// Entries a user can add, minus the ones already in their config.
pub fn addable(existing: &[ProviderConfig]) -> Vec<&'static Preset> {
    PRESETS
        .iter()
        .filter(|p| !p.is_built_in())
        .filter(|p| p.id == "custom" || p.id == "gateway" || !existing.iter().any(|e| e.id == p.id))
        .collect()
}
