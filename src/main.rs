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
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(26.0), if alpha > 0.9 { Sense::click() } else { Sense::hover() });
    if alpha > 0.02 {
        if resp.hovered() && alpha > 0.9 {
            ui.painter().rect_filled(rect.shrink(2.0), 6.0, Color32::from_gray(52));
        }
        let base = if resp.hovered() && alpha > 0.9 { Color32::WHITE } else { Color32::from_gray(215) };
        ui.painter().image(
            tex.id(),
            egui::Rect::from_center_size(rect.center(), Vec2::splat(17.0)),
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

const COLLAPSED: Vec2 = Vec2::new(300.0, 36.0);
const HEADER_H: f32 = 26.0;
const MAX_EXPANDED_H: f32 = 400.0;

mod pal {
    use eframe::egui::Color32;
    pub const BG: Color32 = Color32::from_rgb(22, 24, 29);
    pub const BORDER: Color32 = Color32::from_rgb(43, 47, 56);
    pub const CARD: Color32 = Color32::from_rgb(30, 33, 40);
    pub const TEXT: Color32 = Color32::from_rgb(232, 234, 237);
    pub const MUTED: Color32 = Color32::from_rgb(136, 143, 156);
    pub const FAINT: Color32 = Color32::from_rgb(96, 102, 114);
    pub const GREEN: Color32 = Color32::from_rgb(52, 211, 153);
    pub const AMBER: Color32 = Color32::from_rgb(251, 191, 36);
    pub const RED: Color32 = Color32::from_rgb(248, 113, 113);
    pub const TRACK: Color32 = Color32::from_rgb(46, 50, 60);
}

fn pct_color(pct: Option<f64>, ok: bool) -> eframe::egui::Color32 {
    if !ok {
        return pal::FAINT;
    }
    match pct {
        None => pal::MUTED,
        Some(p) if p > 50.0 => pal::GREEN,
        Some(p) if p > 15.0 => pal::AMBER,
        Some(_) => pal::RED,
    }
}

/// Ring gauge: track circle + progress arc (polyline segments) from 12 o'clock.
fn ring(ui: &egui::Ui, center: egui::Pos2, r: f32, pct: Option<f64>, ok: bool, alpha: f32) {
    let painter = ui.painter();
    painter.circle_stroke(center, r, egui::Stroke::new(3.0_f32, pal::TRACK.linear_multiply(alpha)));
    let color = pct_color(pct, ok);
    let Some(p) = pct.filter(|_| ok) else {
        return;
    };
    let frac = (p / 100.0).clamp(0.0, 1.0) as f32;
    if frac <= 0.0 {
        return;
    }
    if frac > 0.995 {
        painter.circle_stroke(center, r, egui::Stroke::new(3.0_f32, color.linear_multiply(alpha)));
        return;
    }
    let a0 = -std::f32::consts::FRAC_PI_2;
    let a1 = a0 + std::f32::consts::TAU * frac;
    let steps = (24.0 * frac).max(2.0) as usize;
    let pts: Vec<egui::Pos2> = (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * (i as f32 / steps as f32);
            egui::pos2(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect();
    painter.add(egui::Shape::line(pts, egui::Stroke::new(3.0_f32, color.linear_multiply(alpha))));
}

/// Slim rounded progress bar; returns none, paints in given rect.
fn bar(ui: &egui::Ui, rect: egui::Rect, frac: f32, color: Color32, alpha: f32) {
    ui.painter().rect_filled(rect, rect.height() / 2.0, pal::TRACK.linear_multiply(alpha));
    let w = (rect.width() * frac.clamp(0.0, 1.0)).max(rect.height());
    ui.painter()
        .rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(w, rect.height())), rect.height() / 2.0, color.linear_multiply(alpha));
}

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
        let ideal_h = HEADER_H + 46.0 + total_windows as f32 * 20.0 + snaps.len() as f32 * 26.0;
        let expanded_size = Vec2::new(380.0, ideal_h.min(MAX_EXPANDED_H));
        if self.expanded {
            self.last_expanded_h = expanded_size.y;
        }
        let target = if self.expanded { expanded_size } else { COLLAPSED };

        let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.1);
        let t = 1.0 - (-18.0 * dt).exp();
        let prev = self.cur_size;
        self.cur_size = Vec2::new(prev.x + (target.x - prev.x) * t, prev.y + (target.y - prev.y) * t);
        if (self.cur_size - prev).length() > 0.08 {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.cur_size));
            ctx.request_repaint();
        }

        let span = (self.last_expanded_h - COLLAPSED.y).max(1.0);
        let f = (((self.cur_size.y - COLLAPSED.y) / span).clamp(0.0, 1.0)).powi(1).min(1.0);
        let f = 1.0 - (1.0 - f) * (1.0 - f); // ease-out

        let frame = egui::Frame::none()
            .fill(pal::BG)
            .stroke(egui::Stroke::new(1.0, pal::BORDER))
            .rounding(egui::Rounding::same(17.0))
            .inner_margin(egui::Margin::symmetric(11.0, if self.expanded { 8.0 } else { 5.0 }));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            // ============ header row ============
            let header_h = HEADER_H;
            ui.horizontal(|ui| {
                // grip
                let (grect, _gr) = ui.allocate_exact_size(Vec2::new(18.0, HEADER_H), Sense::hover());
                ui.painter().image(
                    self.icons.grip.id(),
                    egui::Rect::from_center_size(grect.center(), Vec2::splat(15.0)),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::from_gray(165).linear_multiply(0.75 + 0.25 * f),
                );
                let grip = ui.interact(grect.expand(4.0), ui.id().with("grip"), Sense::drag());
                if grip.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                ui.add_space(2.0);

                // providers: up to 4 rings, overflow as +N chip (collapsed only)
                let max_vis = if self.expanded { snaps.len() } else { snaps.len().min(4) };
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
                for s in snaps.iter().take(max_vis) {
                    let pct = s.min_remaining();
                    let ok = matches!(s.reading, Reading::Ok { .. });
                    let label = match (&s.reading, pct) {
                        (Reading::Ok { .. }, Some(p)) => format!("{:.0}%", p),
                        (Reading::Ok { .. }, None) => "…".into(),
                        (Reading::NeedsAuth(_), _) => "auth".into(),
                        (Reading::Error(_), _) => "err".into(),
                        _ => "?".into(),
                    };
                    let width = 20.0 + label.len() as f32 * 7.4 + 6.0;
                    let (rect, _r) = ui.allocate_exact_size(Vec2::new(width, header_h), Sense::hover());
                    let c = egui::pos2(rect.left() + 10.0, rect.center().y);
                    ring(ui, c, 6.5, pct, ok, 1.0);
                    ui.put(
                        egui::Rect::from_min_size(egui::pos2(rect.left() + 21.0, rect.center().y - 8.0), Vec2::new(width - 21.0, 16.0)),
                        egui::Label::new(
                            RichText::new(format!("{} {}", tag(&s.provider_id), label))
                                .color(pal::TEXT)
                                .size(12.5),
                        )
                        .selectable(false),
                    );
                    let resp = ui.interact(rect, ui.id().with(("hover", &s.provider_id)), Sense::hover());
                    resp.on_hover_ui(|ui| {
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
                            Reading::Error(m) => { ui.colored_label(pal::RED, m.clone()); }
                            Reading::NotConfigured => { ui.label("not configured"); }
                        }
                        let age = now_unix().saturating_sub(s.fetched_at);
                        ui.colored_label(pal::FAINT, format!("updated {} ago", fmt_countdown(age)));
                    });
                }
                if !self.expanded && snaps.len() > 4 {
                    ui.label(RichText::new(format!("+{}", snaps.len() - 4)).color(pal::MUTED).size(12.0));
                }
                if snaps.is_empty() {
                    ui.label(RichText::new("no providers — open config to add").color(pal::FAINT).size(12.0));
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

            // ============ detail area (scrolls when many providers) ============
            if f > 0.02 {
                ui.visuals_mut().override_text_color = Some(pal::TEXT.linear_multiply(f));
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height((self.cur_size.y - HEADER_H - 34.0).max(60.0))
                    .auto_shrink([false, true])
                    .drag_to_scroll(true)
                    .show(ui, |ui| {
                        for s in &snaps {
                            let pct = s.min_remaining();
                            let ok = matches!(s.reading, Reading::Ok { .. });
                            let card = egui::Frame::none()
                                .fill(pal::CARD)
                                .rounding(egui::Rounding::same(10.0))
                                .inner_margin(egui::Margin::symmetric(10.0, 6.0));
                            card.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rr, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                                    ring(ui, rr.center(), 6.5, pct, ok, f);
                                    ui.strong(s.display_name.clone());
                                    if s.fidelity == Fidelity::Manual {
                                        ui.label(RichText::new("manual").color(pal::FAINT).size(10.5));
                                    }
                                    match &s.reading {
                                        Reading::Ok { .. } => {}
                                        Reading::NeedsAuth(m) => {
                                            ui.colored_label(Color32::YELLOW, format!("· {m}"));
                                        }
                                        Reading::Error(m) => {
                                            ui.colored_label(pal::RED, format!("· {m}"));
                                        }
                                        Reading::NotConfigured => {
                                            ui.colored_label(pal::FAINT, "· not configured");
                                        }
                                    }
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        let age = now_unix().saturating_sub(s.fetched_at);
                                        ui.colored_label(pal::FAINT, format!("{} ago", fmt_countdown(age)));
                                    });
                                });
                                if let Reading::Ok { windows, .. } = &s.reading {
                                    for w in windows {
                                        ui.add_space(2.0);
                                        let (r2, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), Sense::hover());
                                        let lw = 108.0_f32.min(r2.width() * 0.45);
                                        ui.put(
                                            egui::Rect::from_min_size(r2.min, Vec2::new(lw, 14.0)),
                                            egui::Label::new(RichText::new(&w.label).color(pal::MUTED).size(11.5)).selectable(false),
                                        );
                                        let frac = (w.remaining_percent.unwrap_or(0.0) / 100.0) as f32;
                                        let bx = egui::Rect::from_min_size(
                                            egui::pos2(r2.left() + lw + 6.0, r2.center().y - 2.5),
                                            Vec2::new((r2.width() - lw - 116.0).max(40.0), 5.0),
                                        );
                                        bar(ui, bx, frac, pct_color(w.remaining_percent, true), f);
                                        let mut right = String::new();
                                        if let Some(p) = w.remaining_percent {
                                            right.push_str(&format!("{p:.0}%"));
                                        }
                                        if let (Some(a), Some(b)) = (w.remaining_count, w.total_count) {
                                            if b > 0 {
                                                right.push_str(&format!(" {a}/{b}"));
                                            }
                                        }
                                        if let Some(t) = w.resets_at {
                                            right.push_str(&format!(" · in {}", fmt_countdown(t.saturating_sub(now_unix()))));
                                        }
                                        ui.put(
                                            egui::Rect::from_min_size(egui::pos2(r2.right() - 106.0, r2.min.y), Vec2::new(106.0, 14.0)),
                                            egui::Label::new(RichText::new(right).color(pal::TEXT).size(11.5)).selectable(false),
                                        );
                                    }
                                }
                            });
                            ui.add_space(6.0);
                        }
                    });
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
