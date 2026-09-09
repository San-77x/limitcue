//! The command-line surface.
//!
//! The widget is only one way to read a quota. Everything it knows is also
//! available as text, as JSON, and as a status-bar line, so LimitCue can be a
//! source other tools read from rather than a window you have to look at.

use crate::config::Config;
use crate::providers::build_all;
use crate::types::{fmt_countdown, now_unix, Reading, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// Open the window (the default).
    Gui,
    /// One human-readable table.
    Once,
    /// One JSON document.
    Json,
    /// One status-bar line with pango markup.
    Line,
    /// One waybar `custom/*` JSON object.
    Waybar,
    /// Send one desktop notification, to check alerts reach the tray.
    TestAlert,
    Help,
    Version,
}

pub struct Args {
    pub cmd: Cmd,
    /// Keep printing every poll interval instead of exiting.
    pub watch: bool,
}

pub fn parse<I: Iterator<Item = String>>(argv: I) -> Args {
    let mut cmd = Cmd::Gui;
    let mut watch = false;
    for a in argv.skip(1) {
        match a.as_str() {
            "--once" => cmd = Cmd::Once,
            "--json" => cmd = Cmd::Json,
            "--line" | "--stdout" => cmd = Cmd::Line,
            "--waybar" => cmd = Cmd::Waybar,
            "--test-alert" => cmd = Cmd::TestAlert,
            "--watch" => watch = true,
            "-h" | "--help" => cmd = Cmd::Help,
            "-V" | "--version" => cmd = Cmd::Version,
            _ => {}
        }
    }
    Args { cmd, watch }
}

pub const HELP: &str = "\
limitcue — an always-on-top quota gauge for AI coding plans

USAGE
  limitcue                 open the notch (default)
  limitcue --once          print one reading per provider and exit
  limitcue --json          print one JSON document and exit
  limitcue --line          print one status-bar line (pango markup)
  limitcue --waybar        print one waybar custom-module object

OPTIONS
  --test-alert             send one desktop notification and exit
  --watch                  keep printing every poll interval instead of exiting
  -h, --help               show this
  -V, --version            show the version

CONFIG
  ~/.config/limitcue/config.toml   providers, theme, poll interval
  Providers are easier to add from Settings → Providers → Add.

EXIT
  0 always, so a status bar never shows an error box for a missing provider.
";

/// Read every configured provider once. Shared by all the one-shot commands so
/// they cannot drift apart in which providers they include.
fn read_all(cfg: &Config) -> Vec<Snapshot> {
    build_all(cfg)
        .iter()
        .filter(|p| p.is_present())
        .map(|p| p.snapshot())
        .collect()
}

/// The number a bar should show for a provider: the window closest to empty.
fn worst(s: &Snapshot) -> Option<f64> {
    s.min_remaining()
}

pub fn run(cmd: Cmd, cfg: &Config, watch: bool) {
    if cmd == Cmd::TestAlert {
        if crate::notify::Notifier::new().test() {
            println!("sent one notification to the desktop");
        } else {
            println!("no session bus — notifications are not available here");
        }
        return;
    }
    loop {
        let snaps = read_all(cfg);
        match cmd {
            Cmd::Once => print_table(&snaps),
            Cmd::Json => println!("{}", json_doc(&snaps)),
            Cmd::Line => println!("{}", status_line(&snaps)),
            Cmd::Waybar => println!("{}", waybar(&snaps)),
            _ => {}
        }
        if !watch {
            return;
        }
        use std::io::Write;
        let _ = std::io::stdout().flush();
        std::thread::sleep(std::time::Duration::from_secs(cfg.poll_interval_secs.max(5)));
    }
}

fn print_table(snaps: &[Snapshot]) {
    for s in snaps {
        match &s.reading {
            Reading::Ok { windows, .. } => {
                let parts: Vec<String> = windows
                    .iter()
                    .map(|w| match w.remaining_percent {
                        Some(p) => format!("{} {p:.0}%", w.label),
                        None => w.label.clone(),
                    })
                    .collect();
                println!("{:<12} {}", s.provider_id, parts.join(" | "));
            }
            Reading::NeedsAuth(m) => println!("{:<12} needs auth: {m}", s.provider_id),
            Reading::Error(m) => println!("{:<12} error: {m}", s.provider_id),
            Reading::NotConfigured => println!("{:<12} not configured", s.provider_id),
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A deliberately flat, stable document — not the app's internal state dumped
/// out. Anything reading this should not have to track our enum shapes.
fn json_doc(snaps: &[Snapshot]) -> String {
    let now = now_unix();
    let mut out = format!("{{\"generated_at\":{now},\"providers\":[");
    for (i, s) in snaps.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let (status, message) = match &s.reading {
            Reading::Ok { .. } => ("ok", String::new()),
            Reading::NeedsAuth(m) => ("needs_auth", m.clone()),
            Reading::Error(m) => ("error", m.clone()),
            Reading::NotConfigured => ("not_configured", String::new()),
        };
        out.push_str(&format!(
            "{{\"id\":\"{}\",\"name\":\"{}\",\"fidelity\":\"{}\",\"status\":\"{}\"",
            esc(&s.provider_id),
            esc(&s.display_name),
            match s.fidelity {
                crate::types::Fidelity::Official => "official",
                crate::types::Fidelity::Derived => "derived",
                crate::types::Fidelity::Manual => "manual",
            },
            status
        ));
        if !message.is_empty() {
            out.push_str(&format!(",\"message\":\"{}\"", esc(&message)));
        }
        match worst(s) {
            Some(p) => out.push_str(&format!(",\"remaining_percent\":{p:.1}")),
            None => out.push_str(",\"remaining_percent\":null"),
        }
        out.push_str(&format!(",\"fetched_at\":{},\"windows\":[", s.fetched_at));
        if let Reading::Ok { windows, detail } = &s.reading {
            for (j, w) in windows.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                out.push_str(&format!("{{\"label\":\"{}\"", esc(&w.label)));
                match w.remaining_percent {
                    Some(p) => out.push_str(&format!(",\"remaining_percent\":{p:.1}")),
                    None => out.push_str(",\"remaining_percent\":null"),
                }
                if let (Some(a), Some(b)) = (w.remaining_count, w.total_count) {
                    out.push_str(&format!(",\"remaining_count\":{a},\"total_count\":{b}"));
                }
                if let Some(t) = w.resets_at {
                    out.push_str(&format!(
                        ",\"resets_at\":{t},\"resets_in\":{}",
                        t.saturating_sub(now)
                    ));
                }
                out.push('}');
            }
            out.push(']');
            if let Some(d) = detail {
                out.push_str(&format!(",\"detail\":\"{}\"", esc(d)));
            }
        } else {
            out.push(']');
        }
        out.push('}');
    }
    out.push_str("]}");
    out
}

/// Colour a percentage the same way the gauges do, so a status bar and the
/// notch never disagree about whether a number is fine.
fn bar_color(pct: f64) -> &'static str {
    match pct {
        p if p <= 5.0 => "#ff453a",
        p if p <= 25.0 => "#ff9500",
        p if p <= 55.0 => "#ffd60a",
        _ => "#34d361",
    }
}

fn short(id: &str) -> String {
    match id {
        "claude" => "CL".into(),
        "codex" => "CX".into(),
        "minimax" => "M3".into(),
        "openrouter" => "OR".into(),
        other => other.chars().take(2).collect::<String>().to_uppercase(),
    }
}

fn status_line(snaps: &[Snapshot]) -> String {
    let parts: Vec<String> = snaps
        .iter()
        .map(|s| match (&s.reading, worst(s)) {
            (Reading::Ok { .. }, Some(p)) => format!(
                "{} <span color='{}'>{:.0}%</span>",
                short(&s.provider_id),
                bar_color(p),
                p
            ),
            (Reading::NeedsAuth(_), _) => format!("{} <span color='#f6be51'>auth</span>", short(&s.provider_id)),
            (Reading::Error(_), _) => format!("{} <span color='#f86c68'>err</span>", short(&s.provider_id)),
            _ => format!("{} —", short(&s.provider_id)),
        })
        .collect();
    parts.join("  ")
}

/// waybar's `custom/*` contract: text, tooltip, class, percentage.
fn waybar(snaps: &[Snapshot]) -> String {
    let now = now_unix();
    let low = snaps.iter().filter_map(worst).fold(f64::INFINITY, f64::min);
    let low = if low.is_finite() { low } else { -1.0 };
    let class = match low {
        p if p < 0.0 => "unknown",
        p if p <= 5.0 => "critical",
        p if p <= 25.0 => "warning",
        _ => "ok",
    };
    let mut tip = String::new();
    for s in snaps {
        if !tip.is_empty() {
            tip.push_str("\\n");
        }
        match &s.reading {
            Reading::Ok { windows, .. } => {
                tip.push_str(&esc(&s.display_name));
                for w in windows {
                    let pct = w.remaining_percent.map(|p| format!("{p:.0}%")).unwrap_or_else(|| "—".into());
                    let reset = w
                        .resets_at
                        .map(|t| format!(", resets in {}", fmt_countdown(t.saturating_sub(now))))
                        .unwrap_or_default();
                    tip.push_str(&format!("\\n  {} {pct}{reset}", esc(&w.label)));
                }
            }
            Reading::NeedsAuth(m) => tip.push_str(&format!("{}: needs auth ({})", esc(&s.display_name), esc(m))),
            Reading::Error(m) => tip.push_str(&format!("{}: {}", esc(&s.display_name), esc(m))),
            Reading::NotConfigured => tip.push_str(&format!("{}: not configured", esc(&s.display_name))),
        }
    }
    format!(
        "{{\"text\":\"{}\",\"tooltip\":\"{}\",\"class\":\"{}\",\"percentage\":{}}}",
        esc(&status_line(snaps)),
        tip,
        class,
        if low < 0.0 { 0.0 } else { low }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Fidelity, Window};

    fn snap(id: &str, reading: Reading) -> Snapshot {
        Snapshot {
            provider_id: id.into(),
            display_name: id.into(),
            fidelity: Fidelity::Official,
            reading,
            fetched_at: 1_700_000_000,
        }
    }

    fn ok(pct: f64) -> Reading {
        Reading::Ok {
            windows: vec![Window {
                label: "5h".into(),
                remaining_percent: Some(pct),
                remaining_count: None,
                total_count: None,
                resets_at: None,
            }],
            detail: None,
        }
    }

    #[test]
    fn flags_map_to_commands() {
        let a = |s: &str| parse(["limitcue".to_string(), s.to_string()].into_iter()).cmd;
        assert_eq!(a("--json"), Cmd::Json);
        assert_eq!(a("--waybar"), Cmd::Waybar);
        assert_eq!(a("--line"), Cmd::Line);
        assert_eq!(a("--stdout"), Cmd::Line); // the name the v2 note used
        assert_eq!(parse(["limitcue".to_string()].into_iter()).cmd, Cmd::Gui);
        assert!(parse(["l".to_string(), "--watch".to_string()].into_iter()).watch);
    }

    #[test]
    fn json_is_parseable_and_carries_the_worst_window() {
        let out = json_doc(&[snap("claude", ok(22.0))]);
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["providers"][0]["id"], "claude");
        assert_eq!(v["providers"][0]["remaining_percent"], 22.0);
        assert_eq!(v["providers"][0]["status"], "ok");
    }

    #[test]
    fn json_survives_a_message_with_quotes() {
        let out = json_doc(&[snap("x", Reading::Error("bad \"key\" here".into()))]);
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["providers"][0]["message"], "bad \"key\" here");
    }

    #[test]
    fn waybar_reports_the_lowest_provider_as_the_class() {
        let out = waybar(&[snap("a", ok(80.0)), snap("b", ok(3.0))]);
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["class"], "critical");
        assert_eq!(v["percentage"], 3.0);
    }

    #[test]
    fn waybar_with_nothing_readable_is_still_valid() {
        let out = waybar(&[snap("a", Reading::NotConfigured)]);
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        assert_eq!(v["class"], "unknown");
    }

    #[test]
    fn the_status_line_colours_by_severity() {
        let line = status_line(&[snap("claude", ok(3.0))]);
        assert!(line.contains("#ff453a"), "{line}");
        assert!(line.starts_with("CL "), "{line}");
    }
}
