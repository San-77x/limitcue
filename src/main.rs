mod config;
mod providers;
mod types;

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use eframe::egui::{self, Color32, RichText, Sense, Vec2, ViewportBuilder, ViewportCommand};

use config::Config;
use providers::{
    claude::Claude, codex::Codex, custom::Custom, kimi::Kimi, minimax::MiniMax, Provider,
};
use types::{fmt_countdown, now_unix, Fidelity, Reading, Snapshot};

fn state_path() -> std::path::PathBuf {
    dirs::data_dir().unwrap_or_default().join("limitcue").join("state.json")
}

fn save_state(snaps: &HashMap<String, Snapshot>) {
    if let Some(dir) = state_path().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let ok: HashMap<&String, &Snapshot> = snaps
        .iter()
        .filter(|(_, s)| matches!(s.reading, Reading::Ok { .. }))
        .map(|(k, v)| (k, v))
        .collect();
    if let Ok(j) = serde_json::to_string(&ok) {
        let _ = std::fs::write(state_path(), j);
    }
}

fn load_state() -> HashMap<String, Snapshot> {
    serde_json::from_str(&std::fs::read_to_string(state_path()).unwrap_or_default()).unwrap_or_default()
}

fn build_providers(cfg: &Config) -> Vec<Box<dyn Provider>> {
    let mut v: Vec<Box<dyn Provider>> = vec![Box::new(Claude), Box::new(Codex)];
    for p in &cfg.provider {
        if p.id == "minimax" {
            v.push(Box::new(MiniMax::new(p.clone())));
        } else if p.id == "kimi" {
            v.push(Box::new(Kimi::new(Some(p.clone()))));
        } else {
            v.push(Box::new(Custom::new(p.clone())));
        }
    }
    if !cfg.provider.iter().any(|p| p.id == "kimi") {
        v.push(Box::new(Kimi::new(None)));
    }
    v.retain(|p| !cfg.disabled.contains(&p.id()));
    v
}

fn color_for(pct: Option<f64>, ok: bool) -> Color32 {
    if !ok {
        return Color32::from_gray(120);
    }
    match pct {
        None => Color32::from_gray(160),
        Some(p) if p > 50.0 => Color32::from_rgb(60, 200, 110),
        Some(p) if p > 15.0 => Color32::from_rgb(240, 180, 60),
        Some(_) => Color32::from_rgb(235, 80, 80),
    }
}

struct App {
    snapshots: HashMap<String, Snapshot>,
    rx: mpsc::Receiver<Vec<Snapshot>>,
    tx_tick: mpsc::Sender<()>,
    cfg: Config,
    open_popup: bool,
}

impl App {
    fn new(cfg: Config) -> Self {
        let (tx_snap, rx_snap) = mpsc::channel::<Vec<Snapshot>>();
        let (tx_tick, rx_tick) = mpsc::channel::<()>();
        let providers = build_providers(&cfg);
        let poll_secs = cfg.poll_interval_secs;
        let snapshots = load_state();
        thread::spawn(move || {
            let mut backoff = poll_secs;
            let mut failed = false;
            loop {
                let mut out = Vec::new();
                for p in &providers {
                    if !p.is_present() {
                        continue;
                    }
                    let s = p.snapshot();
                    if matches!(s.reading, Reading::Error(_) | Reading::NeedsAuth(_)) {
                        failed = true;
                    }
                    out.push(s);
                }
                if tx_snap.send(out).is_err() {
                    break;
                }
                backoff = if failed { (backoff * 2).min(900) } else { poll_secs };
                failed = false;
                let _ = rx_tick.recv_timeout(std::time::Duration::from_secs(backoff));
            }
        });
        Self { snapshots, rx: rx_snap, tx_tick, cfg, open_popup: false }
    }

    fn drain(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(batch) = self.rx.try_recv() {
            for s in batch {
                if matches!(s.reading, Reading::Ok { .. }) {
                    self.snapshots.insert(s.provider_id.clone(), s);
                } else if let Some(old) = self.snapshots.get_mut(&s.provider_id) {
                    old.reading = s.reading;
                } else {
                    self.snapshots.insert(s.provider_id.clone(), s);
                }
                changed = true;
            }
        }
        if changed {
            let oks = self.snapshots.clone();
            save_state(&oks);
            ctx.request_repaint();
        }
    }

    fn visible(&self) -> Vec<Snapshot> {
        let mut v: Vec<_> = self
            .snapshots
            .values()
            .filter(|s| match &s.reading {
                Reading::NotConfigured => !self.cfg.hide_unconfigured,
                _ => true,
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        v
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            // drag anywhere on the pill to move it; StartDrag hands the grab to the
            // compositor, which works on both Wayland (KWin) and X11
            let resp = ui.interact(ui.max_rect(), ui.id().with("drag"), Sense::click_and_drag());
            if resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if resp.double_clicked() {
                self.open_popup = true;
            }
            let snaps = self.visible();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                for s in &snaps {
                    let pct = s.min_remaining();
                    let ok = matches!(s.reading, Reading::Ok { .. });
                    let label = match (&s.reading, pct) {
                        (Reading::Ok { .. }, Some(p)) => format!("{} {:.0}%", tag(&s.provider_id), p),
                        (Reading::Ok { .. }, None) => format!("{}", tag(&s.provider_id)),
                        (Reading::NeedsAuth(_), _) => format!("{} !", tag(&s.provider_id)),
                        (Reading::Error(_), _) => format!("{} ×", tag(&s.provider_id)),
                        _ => tag(&s.provider_id),
                    };
                    let (rect, _r) = ui.allocate_exact_size(Vec2::new(label.len() as f32 * 7.2 + 8.0, 18.0), Sense::hover());
                    ui.painter().circle_filled(egui::pos2(rect.left() + 6.0, rect.center().y), 3.5, color_for(pct, ok));
                    let label_resp = ui.put(
                        rect,
                        egui::Label::new(RichText::new(label).color(Color32::from_gray(225)).size(12.5)).selectable(false),
                    );
                    label_resp.on_hover_ui(|ui| {
                        ui.strong(s.display_name.clone());
                        match &s.reading {
                            Reading::Ok { windows, .. } => {
                                for w in windows {
                                    let pctt = w.remaining_percent.map(|p| format!("{p:.0}% left")).unwrap_or_default();
                                    let cnt = match (w.remaining_count, w.total_count) {
                                        (Some(a), Some(b)) if b > 0 => format!(" {a}/{b}"),
                                        _ => String::new(),
                                    };
                                    let reset = w
                                        .resets_at
                                        .map(|t| fmt_countdown(t.saturating_sub(now_unix())))
                                        .map(|c| format!(" · resets in {c}"))
                                        .unwrap_or_default();
                                    ui.label(format!("{}: {pctt}{cnt}{reset}", w.label));
                                }
                                if s.fidelity == Fidelity::Manual {
                                    ui.colored_label(Color32::from_gray(140), "user-configured source");
                                }
                            }
                            Reading::NeedsAuth(m) => { ui.colored_label(Color32::YELLOW, format!("needs auth: {m}")); }
                            Reading::Error(m) => { ui.colored_label(Color32::LIGHT_RED, m.clone()); }
                            Reading::NotConfigured => { ui.label("not configured"); }
                        }
                        let age = now_unix().saturating_sub(s.fetched_at);
                        ui.colored_label(Color32::from_gray(120), format!("updated {} ago", fmt_countdown(age)));
                    });
                }
                if snaps.is_empty() {
                    ui.label(RichText::new("no providers").color(Color32::from_gray(130)));
                }
            });
            if ui.button("⋯").clicked() {
                self.open_popup = true;
            }
        });

        if self.open_popup {
            let mut open = true;
            egui::Window::new("LimitCue").open(&mut open).show(ctx, |ui| {
                for s in self.visible() {
                    ui.label(format!("{} — {}", s.display_name, s.provider_id));
                }
                ui.separator();
                if ui.button("Refresh now").clicked() {
                    let _ = self.tx_tick.send(());
                }
                if ui.button("Quit").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            self.open_popup = open;
        }
    }
}

fn tag(id: &str) -> String {
    match id {
        "claude" => "CLAUDE".into(),
        "codex" => "CODEX".into(),
        "minimax" => "M3".into(),
        other => other.to_uppercase(),
    }
}

fn run_once(cfg: &Config) {
    for p in build_providers(cfg) {
        if !p.is_present() {
            println!("{:<8} not configured", p.id());
            continue;
        }
        let s = p.snapshot();
        match &s.reading {
            Reading::Ok { windows, .. } => {
                let parts: Vec<String> = windows
                    .iter()
                    .map(|w| match w.remaining_percent {
                        Some(p) => format!("{} {p:.0}%", w.label),
                        None => w.label.clone(),
                    })
                    .collect();
                println!("{:<8} {}", s.provider_id, parts.join(" | "));
            }
            other => println!("{:<8} {:?}", s.provider_id, other),
        }
    }
}

fn main() -> eframe::Result<()> {
    Config::write_example_if_missing();
    let cfg = Config::load();
    if std::env::args().any(|a| a == "--once") {
        run_once(&cfg);
        return Ok(());
    }
    let app = App::new(cfg.clone());

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_inner_size(Vec2::new(260.0, 30.0))
            .with_position(egui::pos2(60.0, 40.0)),
        ..Default::default()
    };
    eframe::run_native("limitcue", options, Box::new(move |_cc| Ok(Box::new(app))))
}
