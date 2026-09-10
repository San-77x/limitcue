//! Which providers are being used *right now*.
//!
//! "Why did my percentage just move" is a question the gauge cannot answer on
//! its own. The vendor CLIs each keep a session log, and a log that was
//! written to a minute ago means an agent is running against that plan. That
//! is enough to mark the provider as live without reading a single line of
//! what was said.
//!
//! Only modification times are looked at. The contents of a session log are
//! none of this app's business, and it never opens one.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

use crate::config::Config;
use crate::providers::expand_home;
use crate::types::now_unix;

/// A session touched more recently than this counts as running.
pub const LIVE_WINDOW_SECS: u64 = 90;
/// How often to re-scan. Cheap, but not free — these are directory walks.
const SCAN_EVERY: Duration = Duration::from_secs(5);
/// Guard rails for the walk: a projects directory can be large, and no answer
/// is worth stalling on it.
const MAX_DEPTH: usize = 4;
const MAX_ENTRIES: usize = 4000;

#[derive(Default)]
pub struct Activity {
    /// Provider id → unix time its session log was last written.
    last: Mutex<HashMap<String, u64>>,
}

pub type SharedActivity = Arc<Activity>;

impl Activity {
    /// Seconds since this provider was last written to, if ever.
    pub fn idle_for(&self, id: &str) -> Option<u64> {
        let last = *self.last.lock().ok()?.get(id)?;
        Some(now_unix().saturating_sub(last))
    }

    pub fn is_live(&self, id: &str) -> bool {
        self.idle_for(id).is_some_and(|s| s <= LIVE_WINDOW_SECS)
    }
}

/// Where each provider's CLI keeps its session logs, given the config. Only
/// the compiled-in CLI adapters have one; a key-based provider has no local
/// trace of being used, and inventing one would be a lie.
fn session_dirs(cfg: &Config) -> Vec<(String, PathBuf)> {
    let home = crate::providers::home();
    let mut out = Vec::new();
    let mut add = |id: &str, dir: PathBuf, leaf: &str| out.push((id.to_string(), dir.join(leaf)));

    if !cfg.disabled.iter().any(|d| d == "claude") {
        add("claude", home.join(".claude"), "projects");
    }
    if !cfg.disabled.iter().any(|d| d == "codex") {
        add("codex", home.join(".codex"), "sessions");
    }
    for p in &cfg.provider {
        if p.enabled == Some(false) {
            continue;
        }
        let dir = p.credentials_dir.clone().map(expand_home);
        match (p.adapter.as_deref(), dir) {
            (Some("claude"), Some(d)) => add(&p.id, d, "projects"),
            (Some("codex"), Some(d)) => add(&p.id, d, "sessions"),
            _ => {}
        }
    }
    out
}

/// Newest modification time anywhere under `root`, as unix seconds.
fn newest_mtime(root: &Path) -> Option<u64> {
    let mut newest = 0u64;
    let mut budget = MAX_ENTRIES;
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH || budget == 0 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push((entry.path(), depth + 1));
                continue;
            }
            if let Ok(t) = meta.modified().and_then(|m| {
                m.duration_since(UNIX_EPOCH).map_err(std::io::Error::other)
            }) {
                newest = newest.max(t.as_secs());
            }
        }
    }
    (newest > 0).then_some(newest)
}

/// Watch the session directories in the background. Returns the handle the UI
/// reads; the thread lives for the process.
pub fn start(cfg: Config) -> SharedActivity {
    let shared: SharedActivity = Default::default();
    let out = shared.clone();
    std::thread::spawn(move || {
        let dirs = session_dirs(&cfg);
        if dirs.is_empty() {
            return;
        }
        loop {
            for (id, dir) in &dirs {
                if let Some(t) = newest_mtime(dir) {
                    if let Ok(mut g) = shared.last.lock() {
                        g.insert(id.clone(), t);
                    }
                }
            }
            std::thread::sleep(SCAN_EVERY);
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recent_write_reads_as_live() {
        let a = Activity::default();
        a.last.lock().unwrap().insert("claude".into(), now_unix());
        assert!(a.is_live("claude"));
    }

    #[test]
    fn an_old_write_does_not() {
        let a = Activity::default();
        a.last
            .lock()
            .unwrap()
            .insert("claude".into(), now_unix() - LIVE_WINDOW_SECS - 10);
        assert!(!a.is_live("claude"));
        assert!(a.idle_for("claude").unwrap() > LIVE_WINDOW_SECS);
    }

    #[test]
    fn a_provider_never_seen_is_neither() {
        let a = Activity::default();
        assert!(!a.is_live("nobody"));
        assert!(a.idle_for("nobody").is_none());
    }

    #[test]
    fn the_walk_finds_the_newest_file_below_the_root() {
        let dir = std::env::temp_dir().join(format!("limitcue-act-{}", std::process::id()));
        let nested = dir.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("session.jsonl"), b"x").unwrap();
        let t = newest_mtime(&dir).expect("found the nested file");
        assert!(now_unix().saturating_sub(t) < 60);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_directory_is_quiet() {
        assert!(newest_mtime(Path::new("/nonexistent/limitcue")).is_none());
    }
}
