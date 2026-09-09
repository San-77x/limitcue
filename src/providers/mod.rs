pub mod billing;
pub mod catalog;
pub mod claude;
pub mod codex;
pub mod custom;
pub mod kimi;
pub mod minimax;

use std::time::Duration;

use serde_json::Value;

use crate::config::ProviderConfig;
use crate::types::{Fidelity, Snapshot};

pub trait Provider: Send {
    fn id(&self) -> String;
    fn snapshot(&self) -> Snapshot;
    /// Cheap check: do credentials/config for this provider exist on disk?
    fn is_present(&self) -> bool;
    fn fidelity(&self) -> Fidelity;
}

/// The adapter a `[[provider]]` entry describes. One place decides this, so
/// the poll loop and the settings sheet's Test button can never disagree
/// about what a given config actually does.
pub fn adapter_for(cfg: &ProviderConfig) -> Box<dyn Provider> {
    if cfg.billing {
        Box::new(billing::Billing::new(cfg.clone()))
    } else if cfg.id == "minimax" {
        Box::new(minimax::MiniMax::new(cfg.clone()))
    } else if cfg.id == "kimi" {
        Box::new(kimi::Kimi::new(Some(cfg.clone())))
    } else {
        Box::new(custom::Custom::new(cfg.clone()))
    }
}

/// Take one reading from a config the user is still editing, so the settings
/// sheet can say whether it works before they save it.
pub fn probe(cfg: &ProviderConfig) -> Snapshot {
    adapter_for(cfg).snapshot()
}

/// GET a URL with headers; returns parsed JSON. Errors carry a readable message.
pub fn http_get_json(url: &str, headers: &[(&str, String)]) -> Result<Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(10))
        .build();
    let mut req = agent.get(url);
    for (k, v) in headers {
        req = req.set(k, v);
    }
    match req.call() {
        Ok(resp) => resp
            .into_json()
            .map_err(|e| format!("bad JSON response: {e}")),
        Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => {
            Err("auth-failed".into())
        }
        Err(ureq::Error::Status(429, _)) => Err("rate-limited".into()),
        Err(e) => Err(format!("request failed: {e}")),
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
