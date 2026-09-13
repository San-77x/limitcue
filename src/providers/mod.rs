pub mod billing;
pub mod catalog;
pub mod claude;
pub mod codex;
pub mod custom;
pub mod grok;
pub mod kimi;
pub mod minimax;

use std::time::Duration;

use serde_json::Value;

use crate::config::{Config, ProviderConfig};
use crate::types::{Fidelity, Snapshot};

pub trait Provider: Send {
    fn id(&self) -> String;
    fn snapshot(&self) -> Snapshot;
    /// Cheap check: do credentials/config for this provider exist on disk?
    fn is_present(&self) -> bool;
    fn fidelity(&self) -> Fidelity;
}

/// Every adapter a config asks for, in display order. Built-ins come first,
/// then user entries, then `priority` decides.
/// Every adapter the config asks for, in display order: the compiled-in ones
/// first, then user entries, with `priority` overriding file order.
pub fn build_all(cfg: &Config) -> Vec<Box<dyn Provider>> {
    let mut v: Vec<Box<dyn Provider>> = vec![
        Box::new(claude::Claude::default()),
        Box::new(codex::Codex::default()),
        Box::new(grok::Grok::default()),
    ];
    for p in &cfg.provider {
        if p.enabled == Some(false) {
            continue;
        }
        v.push(adapter_for(p));
    }
    if !cfg
        .provider
        .iter()
        .any(|p| p.id == "kimi" && p.enabled != Some(false))
    {
        v.push(Box::new(kimi::Kimi::new(None)));
    }
    v.retain(|p| !cfg.disabled.contains(&p.id()));
    // `priority` (lower = earlier) wins over file order; None sorts last.
    let rank = |id: &str| {
        cfg.provider
            .iter()
            .find(|p| p.id == id)
            .and_then(|p| p.priority)
            .unwrap_or(u32::MAX)
    };
    let mut keyed: Vec<(u32, usize, Box<dyn Provider>)> = v
        .into_iter()
        .enumerate()
        .map(|(i, p)| (rank(&p.id().to_string()), i, p))
        .collect();
    keyed.sort_by_key(|(rank, i, _)| (*rank, *i));
    keyed.into_iter().map(|(_, _, p)| p).collect()
}

/// The adapter a `[[provider]]` entry describes. One place decides this, so
/// the poll loop and the settings sheet's Test button can never disagree
/// about what a given config actually does.
pub fn adapter_for(cfg: &ProviderConfig) -> Box<dyn Provider> {
    // An explicit `adapter` wins; otherwise the id picks one, which is how
    // every config written before that key behaves.
    let kind = cfg.adapter.clone().unwrap_or_else(|| {
        if cfg.billing {
            "billing".into()
        } else {
            cfg.id.clone()
        }
    });
    match kind.as_str() {
        "billing" => Box::new(billing::Billing::new(cfg.clone())),
        "claude" => Box::new(claude::Claude::new(Some(cfg))),
        "codex" => Box::new(codex::Codex::new(Some(cfg))),
        "grok" => Box::new(grok::Grok::new(Some(cfg))),
        "kimi" => Box::new(kimi::Kimi::new(Some(cfg.clone()))),
        "minimax" => Box::new(minimax::MiniMax::new(cfg.clone())),
        _ => Box::new(custom::Custom::new(cfg.clone())),
    }
}

/// Expand a leading `~` so config files can name a home-relative directory.
pub fn expand_home(p: String) -> std::path::PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None => std::path::PathBuf::from(p),
    }
}

/// Take one reading from a config the user is still editing, so the settings
/// sheet can say whether it works before they save it.
pub fn probe(cfg: &ProviderConfig) -> Snapshot {
    adapter_for(cfg).snapshot()
}

/// GET a URL with headers; returns parsed JSON. Errors carry a readable message.
pub fn http_get_json(url: &str, headers: &[(&str, String)]) -> Result<Value, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
    let mut req = agent.get(url);
    for (k, v) in headers {
        req = req.header(*k, v.as_str());
    }
    match req.call() {
        Ok(mut resp) => resp
            .body_mut()
            .read_json::<Value>()
            .map_err(|e| format!("bad JSON response: {e}")),
        Err(ureq::Error::StatusCode(401)) | Err(ureq::Error::StatusCode(403)) => {
            Err("auth-failed".into())
        }
        Err(ureq::Error::StatusCode(429)) => Err("rate-limited".into()),
        Err(ureq::Error::StatusCode(code)) => Err(format!("server said {code}")),
        // Give the reader the actionable kind, not a dump of the failing URL.
        // The URL is already in their own config; "connection failed" is the
        // part that tells them what to do next.
        Err(e) => Err(match e {
            ureq::Error::HostNotFound => "dns failed",
            ureq::Error::ConnectionFailed => "connection failed",
            ureq::Error::Timeout(_) => "timed out",
            ureq::Error::Io(_) => "io error",
            ureq::Error::BadUri(_) => "bad url",
            ureq::Error::RedirectFailed | ureq::Error::TooManyRedirects => "redirect failed",
            ureq::Error::InvalidProxyUrl => "bad proxy settings",
            ureq::Error::Protocol(_) => "protocol error",
            _ => "request failed",
        }
        .into()),
    }
}

/// Dot-path lookup with optional [n] array indices, e.g. `model_remains[0].current_interval_remaining_percent`.
pub fn json_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path.split('.') {
        let (key, idx) = match seg.split_once('[') {
            Some((k, rest)) => (k, rest.trim_end_matches(']').parse::<usize>().ok()),
            None => (seg, None),
        };
        if !key.is_empty() {
            cur = cur.get(key)?;
        }
        if let Some(i) = idx {
            cur = cur.get(i)?;
        }
    }
    Some(cur)
}

#[allow(dead_code)]
pub fn as_percent(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_u64().map(|n| n as f64))
}

pub fn read_json_file(path: &std::path::Path) -> Option<Value> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn home() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_else(|| "/".into())
}
