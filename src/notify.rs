//! Desktop alerts when a quota runs low, and when it comes back.
//!
//! The point of a quota gauge is to stop you finding out the hard way, but
//! that only works if you are looking at it. These fire from the poll thread
//! rather than the UI, so they arrive whether or not the notch is on screen.
//!
//! Each window latches independently: an alert fires once when it drops
//! through the threshold and re-arms only after a real refill, so a quota
//! hovering on the line cannot machine-gun the notification tray.

use std::collections::HashMap;

use crate::config::Config;
use crate::types::{Reading, Snapshot};

/// How far a window has to climb back above the threshold before it counts as
/// refilled. Without this, one hovering on the line re-alerts forever.
const REARM_MARGIN: f64 = 15.0;

pub struct Notifier {
    conn: Option<zbus::blocking::Connection>,
    /// Windows currently in the "already warned" state, by `provider|window`.
    latched: HashMap<String, ()>,
    /// Suppresses the burst of alerts a fresh start would otherwise produce
    /// for quotas that were already low before the app opened.
    primed: bool,
}

impl Notifier {
    pub fn new() -> Self {
        Self {
            conn: zbus::blocking::Connection::session().ok(),
            latched: HashMap::new(),
            primed: false,
        }
    }

    /// Look at a fresh batch of readings and fire whatever it warrants.
    pub fn review(&mut self, snaps: &[Snapshot], cfg: &Config) {
        if !cfg.notify {
            // Still track state, so switching alerts on mid-session does not
            // immediately announce everything that was already low.
            self.record(snaps, cfg);
            return;
        }
        let threshold = cfg.notify_threshold.clamp(1.0, 99.0);
        for s in snaps {
            let Reading::Ok { windows, .. } = &s.reading else { continue };
            for w in windows {
                let Some(pct) = w.remaining_percent else { continue };
                let key = format!("{}|{}", s.provider_id, w.label);
                let was = self.latched.contains_key(&key);
                if pct <= threshold && !was {
                    self.latched.insert(key, ());
                    if self.primed {
                        self.low(&s.display_name, &w.label, pct, threshold);
                        run_hook(&cfg.cmd_on_low, &s.provider_id, &w.label, pct);
                    }
                } else if pct >= threshold + REARM_MARGIN && was {
                    self.latched.remove(&key);
                    if self.primed && cfg.notify_on_reset {
                        self.refilled(&s.display_name, &w.label, pct);
                        run_hook(&cfg.cmd_on_reset, &s.provider_id, &w.label, pct);
                    }
                }
            }
        }
        self.primed = true;
    }

    /// Seed the latches without announcing anything.
    fn record(&mut self, snaps: &[Snapshot], cfg: &Config) {
        let threshold = cfg.notify_threshold.clamp(1.0, 99.0);
        for s in snaps {
            let Reading::Ok { windows, .. } = &s.reading else { continue };
            for w in windows {
                let Some(pct) = w.remaining_percent else { continue };
                let key = format!("{}|{}", s.provider_id, w.label);
                if pct <= threshold {
                    self.latched.insert(key, ());
                } else if pct >= threshold + REARM_MARGIN {
                    self.latched.remove(&key);
                }
            }
        }
        self.primed = true;
    }

    /// Fire one alert on demand, so "will I actually see these?" is
    /// answerable without waiting for a quota to run out. Returns false when
    /// there is no session bus to send it on.
    pub fn test(&self) -> bool {
        self.send(
            "LimitCue alerts are working",
            "This is what a low-quota warning will look like.",
            false,
        )
    }

    fn low(&self, provider: &str, window: &str, pct: f64, threshold: f64) {
        let urgent = pct <= (threshold / 3.0).max(2.0);
        self.send(
            &format!("{provider} — {pct:.0}% left"),
            &format!("The {window} window is running out."),
            urgent,
        );
    }

    fn refilled(&self, provider: &str, window: &str, pct: f64) {
        self.send(
            &format!("{provider} — back to {pct:.0}%"),
            &format!("The {window} window refilled."),
            false,
        );
    }

    /// Returns whether the desktop actually accepted it, so the settings
    /// sheet can report a missing notification daemon rather than claiming
    /// success into the void.
    fn send(&self, summary: &str, body: &str, urgent: bool) -> bool {
        let Some(conn) = &self.conn else { return false };
        let proxy = zbus::blocking::Proxy::new(
            conn,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        );
        let Ok(proxy) = proxy else { return false };
        let mut hints: HashMap<&str, zbus::zvariant::Value> = HashMap::new();
        hints.insert("urgency", zbus::zvariant::Value::U8(if urgent { 2 } else { 1 }));
        // Collapse repeats from the same provider into one tray entry.
        hints.insert("x-canonical-private-synchronous", zbus::zvariant::Value::from("limitcue"));
        proxy
            .call::<_, _, u32>(
            "Notify",
            &(
                "LimitCue",
                0u32,
                "utilities-system-monitor",
                summary,
                body,
                Vec::<&str>::new(),
                hints,
                if urgent { 0i32 } else { 8000i32 },
            ),
            )
            .is_ok()
    }
}

/// Run a user hook, if they configured one. It gets the details in the
/// environment rather than interpolated into the command, so a provider name
/// can never become part of the shell line.
fn run_hook(cmd: &str, provider: &str, window: &str, pct: f64) {
    if cmd.trim().is_empty() {
        return;
    }
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .env("LIMITCUE_PROVIDER", provider)
        .env("LIMITCUE_WINDOW", window)
        .env("LIMITCUE_PERCENT", format!("{pct:.0}"))
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Fidelity, Window};

    fn cfg() -> Config {
        Config { notify: true, notify_threshold: 15.0, ..Default::default() }
    }

    fn snap(pct: f64) -> Vec<Snapshot> {
        vec![Snapshot {
            provider_id: "p".into(),
            display_name: "P".into(),
            fidelity: Fidelity::Official,
            reading: Reading::Ok {
                windows: vec![Window {
                    label: "5h".into(),
                    remaining_percent: Some(pct),
                    remaining_count: None,
                    total_count: None,
                    resets_at: None,
                }],
                detail: None,
            },
            fetched_at: 0,
            fetch_error: None,
        }]
    }

    /// A notifier with no bus: `send` is a no-op, so the latch logic is what
    /// the tests actually exercise.
    fn offline() -> Notifier {
        Notifier { conn: None, latched: HashMap::new(), primed: true }
    }

    #[test]
    fn latches_once_on_the_way_down() {
        let mut n = offline();
        n.review(&snap(10.0), &cfg());
        assert!(n.latched.contains_key("p|5h"));
        n.review(&snap(9.0), &cfg());
        assert!(n.latched.contains_key("p|5h"), "still latched, not re-fired");
    }

    #[test]
    fn hovering_on_the_threshold_does_not_re_arm() {
        let mut n = offline();
        n.review(&snap(14.0), &cfg());
        n.review(&snap(16.0), &cfg()); // above, but within the margin
        assert!(n.latched.contains_key("p|5h"));
    }

    #[test]
    fn a_real_refill_re_arms() {
        let mut n = offline();
        n.review(&snap(5.0), &cfg());
        n.review(&snap(90.0), &cfg());
        assert!(!n.latched.contains_key("p|5h"));
    }

    #[test]
    fn a_first_reading_never_announces() {
        let mut n = Notifier { conn: None, latched: HashMap::new(), primed: false };
        n.review(&snap(2.0), &cfg());
        assert!(n.latched.contains_key("p|5h"), "recorded");
        assert!(n.primed, "and armed for next time");
    }
}
