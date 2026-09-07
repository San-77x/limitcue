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

const ICON_PNGS: [(&str, &[u8]); 4] = [
    ("grip", include_bytes!("../assets/icons/grip-vertical.png")),
    ("min", include_bytes!("../assets/icons/minus.png")),
    ("refresh", include_bytes!("../assets/icons/refresh-cw.png")),
    ("close", include_bytes!("../assets/icons/x.png")),
];

#[derive(Clone)]
struct Icons {
    grip: egui::TextureHandle,
    min: egui::TextureHandle,
    refresh: egui::TextureHandle,
    close: egui::TextureHandle,
}

fn decode_png(bytes: &[u8]) -> egui::ColorImage {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("icon png header");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("icon png frame");
    assert_eq!(info.color_type, png::ColorType::Rgba, "icons must be RGBA");
    egui::ColorImage::from_rgba_unmultiplied([info.width as usize, info.height as usize], &buf[..info.buffer_size()])
}

fn load_icons(ctx: &egui::Context) -> Icons {
    let get = |name: &str| {
        let bytes = ICON_PNGS.iter().find(|(n, _)| *n == name).map(|(_, b)| *b).unwrap();
        ctx.load_texture(name, decode_png(bytes), egui::TextureOptions::LINEAR)
    };
    Icons { grip: get("grip"), min: get("min"), refresh: get("refresh"), close: get("close") }
}

fn icon_button(ui: &mut egui::Ui, tex: &egui::TextureHandle, tip: &str, alpha: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(24.0), if alpha > 0.9 { Sense::click() } else { Sense::hover() });
    if alpha > 0.02 {
        if resp.hovered() && alpha > 0.9 {
            ui.painter().rect_filled(rect.shrink(2.0), 6.0, Color32::from_gray(52));
        }
        let base = if resp.hovered() && alpha > 0.9 { Color32::WHITE } else { Color32::from_gray(190) };
        ui.painter().image(
            tex.id(),
            rect.shrink(4.5),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            base.linear_multiply(alpha),
        );
    }
    if alpha > 0.9 { resp.on_hover_text(tip) } else { resp }
}

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
    expanded: bool,
    cur_size: Vec2,
    last_expanded_h: f32,
    icons: Icons,
}

impl App {
    fn new(cfg: Config, icons: Icons) -> Self {
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
        Self { snapshots, rx: rx_snap, tx_tick, cfg, expanded: false, cur_size: COLLAPSED, last_expanded_h: 180.0, icons }
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

const COLLAPSED: Vec2 = Vec2::new(300.0, 30.0);
const HEADER_H: f32 = 28.0;
const MARGINS: [f32; 4] = [10.0, 26.0, 10.0, 26.0]; // top/bottom expanded/collapsed

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        ctx.request_repaint_after(std::time::Duration::from_secs(30));

        let snaps = self.visible();
        let total_windows: usize = snaps
            .iter()
            .map(|s| match &s.reading {
                Reading::Ok { windows, .. } => windows.len().max(1),
                _ => 1,
            })
            .sum();
        let expanded_size = Vec2::new(
            360.0,
            HEADER_H + MARGINS[0] + MARGINS[1] + total_windows as f32 * 19.0 + snaps.len() as f32 * 32.0,
        );
        if self.expanded {
            self.last_expanded_h = expanded_size.y;
        }
        let target = if self.expanded { expanded_size } else { COLLAPSED };

        // exponential ease toward target; repaint while moving
        let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.1);
        let t = 1.0 - (-18.0 * dt).exp();
        let prev = self.cur_size;
        self.cur_size = Vec2::new(prev.x + (target.x - prev.x) * t, prev.y + (target.y - prev.y) * t);
        if (self.cur_size - prev).length() > 0.08 || self.cur_size != prev {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.cur_size));
            ctx.request_repaint();
        }

        // progress 0..1 (0 = collapsed, 1 = fully expanded)
        let span = (self.last_expanded_h - COLLAPSED.y).max(1.0);
        let f = ((self.cur_size.y - COLLAPSED.y) / span).clamp(0.0, 1.0);
        let f = 1.0 - (1.0 - f) * (1.0 - f);

        // animated inner margin (vertical) so content glides with the growth
        let m_top = MARGINS[3] + (MARGINS[0] - MARGINS[3]) * f;
        let m_bot = MARGINS[3] + (MARGINS[1] - MARGINS[3]) * f;

        let frame = egui::Frame::none()
            .fill(Color32::from_rgb(24, 26, 30))
            .rounding(egui::Rounding::same(15.0))
            .inner_margin(egui::Margin::symmetric(12.0, m_top.max(m_bot)));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (grect, _gr) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                ui.painter().image(
                    self.icons.grip.id(),
                    grect.shrink(1.0),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::from_gray(130),
                );
                let grip = ui.interact(grect.expand(4.0), ui.id().with("grip"), Sense::drag());
                if grip.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }

                // provider dots/labels — stable header, present in both states
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
                for s in &snaps {
                    let pct = s.min_remaining();
                    let ok = matches!(s.reading, Reading::Ok { .. });
                    let label = match (&s.reading, pct) {
                        (Reading::Ok { .. }, Some(p)) => format!("{} {:.0}%", tag(&s.provider_id), p),
                        (Reading::Ok { .. }, None) => tag(&s.provider_id),
                        (Reading::NeedsAuth(_), _) => format!("{} !", tag(&s.provider_id)),
                        (Reading::Error(_), _) => format!("{} ×", tag(&s.provider_id)),
                        _ => tag(&s.provider_id),
                    };
                    let (rect, _r) =
                        ui.allocate_exact_size(Vec2::new(label.len() as f32 * 7.2 + 8.0, 18.0), Sense::hover());
                    ui.painter().circle_filled(
                        egui::pos2(rect.left() + 6.0, rect.center().y),
                        3.5,
                        color_for(pct, ok),
                    );
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
                                    let reset = w
                                        .resets_at
                                        .map(|t| fmt_countdown(t.saturating_sub(now_unix())))
                                        .map(|c| format!(" · resets in {c}"))
                                        .unwrap_or_default();
                                    ui.label(format!("{}: {pctt}{reset}", w.label));
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, &self.icons.min, "minimize (or press Esc)", f).clicked() {
                        self.expanded = false;
                    }
                    if icon_button(ui, &self.icons.refresh, "refresh now", f).clicked() {
                        let _ = self.tx_tick.send(());
                    }
                    if icon_button(ui, &self.icons.close, "quit", f).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });

            // expand-on-click only when collapsed
            if f < 0.05 {
                let body = ui.interact(ui.max_rect().shrink(2.0), ui.id().with("body"), Sense::click());
                if body.clicked() {
                    self.expanded = true;
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::R)) {
                let _ = self.tx_tick.send(());
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) && self.expanded {
                self.expanded = false;
            }

            // faded detail section, revealed as the window grows
            if f > 0.02 {
                ui.visuals_mut().override_text_color = Some(Color32::from_gray(205).linear_multiply(f));
                ui.separator();
                ui.colored_label(Color32::from_gray(110).linear_multiply(f), "R refresh · Esc minimize");
                for s in &snaps {
                    let pct = s.min_remaining();
                    let ok = matches!(s.reading, Reading::Ok { .. });
                    ui.horizontal(|ui| {
                        ui.painter().circle_filled(
                            ui.cursor().center() - Vec2::new(0.0, 2.0),
                            3.5,
                            color_for(pct, ok).linear_multiply(f),
                        );
                        ui.strong(s.display_name.clone());
                        if s.fidelity == Fidelity::Manual {
                            ui.colored_label(Color32::from_gray(120), "(manual)");
                        }
                    });
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
                                ui.label(format!("  {}: {pctt}{cnt}{reset}", w.label));
                            }
                        }
                        Reading::NeedsAuth(m) => {
                            ui.colored_label(Color32::YELLOW, format!("  needs auth: {m}"));
                        }
                        Reading::Error(m) => {
                            ui.colored_label(Color32::LIGHT_RED, format!("  {m}"));
                        }
                        Reading::NotConfigured => {
                            ui.label("  not configured");
                        }
                    }
                    let age = now_unix().saturating_sub(s.fetched_at);
                    ui.colored_label(Color32::from_gray(110), format!("updated {} ago", fmt_countdown(age)));
                    ui.add_space(2.0);
                }
            }
        });
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
    eframe::run_native(
        "limitcue",
        options,
        Box::new(move |cc| {
            let icons = load_icons(&cc.egui_ctx);
            Ok(Box::new(App::new(cfg, icons)))
        }),
    )
}
