mod config;
mod providers;
mod types;
mod ui;

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use eframe::egui::{self, RichText, Sense, Vec2, ViewportBuilder, ViewportCommand};

use config::Config;
use providers::{
    claude::Claude, codex::Codex, custom::Custom, kimi::Kimi, minimax::MiniMax, Provider,
};
use types::{now_unix, Reading, Snapshot};
use ui::theme::{self, Palette};

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

const COLLAPSED_H: f32 = 36.0;
const HEADER_H: f32 = 26.0;
const EXPANDED_W: f32 = 380.0;
const MAX_EXPANDED_H: f32 = 400.0;
const TWEEN_SECS: f32 = 0.45;
const SPIN_SECS: f32 = 0.55;

/// Animates a provider's headline percentage from its old value to a fresh one.
struct Tween {
    from: f64,
    to: f64,
    start: Instant,
}

impl Tween {
    /// Current value, or None when the animation has finished.
    fn value(&self) -> Option<f64> {
        let t = self.start.elapsed().as_secs_f32() / TWEEN_SECS;
        if t >= 1.0 {
            return None;
        }
        let e = 1.0 - (1.0 - t).powi(3); // ease-out cubic
        Some(self.from + (self.to - self.from) * e as f64)
    }
}

struct App {
    snapshots: HashMap<String, Snapshot>,
    tweens: HashMap<String, Tween>,
    rx: mpsc::Receiver<Vec<Snapshot>>,
    tx_tick: mpsc::Sender<()>,
    cfg: Config,
    pal: Palette,
    expanded: bool,
    cur_size: Vec2,
    last_expanded_h: f32,
    refresh_at: Option<Instant>,
    icons: Icons,
}

impl App {
    fn new(cfg: Config, pal: Palette, icons: Icons) -> Self {
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
        // Debug hook: start expanded (used by tests/screenshot automation).
        let expanded = std::env::var("LIMITCUE_UI_EXPANDED").map(|v| v != "0").unwrap_or(false);
        Self {
            snapshots,
            tweens: HashMap::new(),
            rx: rx_snap,
            tx_tick,
            cfg,
            pal,
            expanded,
            cur_size: Vec2::new(280.0, COLLAPSED_H),
            last_expanded_h: 200.0,
            refresh_at: None,
            icons,
        }
    }

    fn drain(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(batch) = self.rx.try_recv() {
            for s in batch {
                if matches!(s.reading, Reading::Ok { .. }) {
                    if let Some(new) = s.min_remaining() {
                        let shown = self.tweens.get(&s.provider_id).and_then(|t| t.value());
                        let from = shown
                            .or_else(|| self.snapshots.get(&s.provider_id).and_then(|old| old.min_remaining()));
                        if let Some(from) = from {
                            if (from - new).abs() > f64::EPSILON {
                                self.tweens.insert(
                                    s.provider_id.clone(),
                                    Tween { from, to: new, start: Instant::now() },
                                );
                            }
                        }
                    }
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

    /// Headline percent to draw: mid-tween value if animating, else the snapshot's.
    fn render_pct(&self, s: &Snapshot) -> Option<f64> {
        self.tweens.get(&s.provider_id).and_then(|t| t.value()).or_else(|| s.min_remaining())
    }

    fn is_stale(&self, s: &Snapshot, now: u64) -> bool {
        matches!(s.reading, Reading::Ok { .. })
            && now.saturating_sub(s.fetched_at) > self.cfg.poll_interval_secs * 3
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

    /// Collapsed pill width that fits its content exactly.
    fn collapsed_width(&self, ctx: &egui::Context, snaps: &[Snapshot]) -> f32 {
        let max_vis = snaps.len().min(self.cfg.max_visible_collapsed);
        let mut w = 11.0 * 2.0 // frame margins
            + 18.0 + 2.0 // grip + gap
            + 26.0 * 3.0 + 8.0 * 2.0; // three icon buttons + spacing
        for s in snaps.iter().take(max_vis) {
            w += 10.0 + ui::chip_width(ui::chip_text_w(ctx, s, self.render_pct(s), &self.pal, 1.0));
        }
        if snaps.len() > max_vis {
            w += 10.0 + 32.0; // +N chip
        }
        if snaps.is_empty() {
            w = w.max(280.0);
        }
        w
    }

    fn refresh(&mut self, ctx: &egui::Context) {
        let _ = self.tx_tick.send(());
        self.refresh_at = Some(Instant::now());
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        self.tweens.retain(|_, t| t.value().is_some());

        let pal = self.pal;
        let now = now_unix();
        let snaps = self.visible();

        // Expanded height grows with content, capped by MAX_EXPANDED_H (then scrolls).
        let ideal_h = HEADER_H + 16.0
            + snaps
                .iter()
                .map(|s| {
                    let rows_h = match &s.reading {
                        Reading::Ok { windows, .. } => windows.len().max(1) as f32 * 19.0,
                        _ => 22.0,
                    };
                    34.0 + rows_h
                })
                .sum::<f32>()
            + snaps.len() as f32 * 6.0;
        let expanded_size = Vec2::new(EXPANDED_W, (ideal_h + 10.0).min(MAX_EXPANDED_H));
        if self.expanded {
            self.last_expanded_h = expanded_size.y;
        }

        let target = if self.expanded {
            expanded_size
        } else {
            Vec2::new(self.collapsed_width(ctx, &snaps), COLLAPSED_H)
        };
        // Debug hook: jump to the final size instead of animating (screenshot automation).
        if std::env::var("LIMITCUE_UI_SNAP").map(|v| v != "0").unwrap_or(false) {
            self.cur_size = target;
        }

        let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.1);
        let t = 1.0 - (-18.0 * dt).exp();
        let prev = self.cur_size;
        self.cur_size = Vec2::new(prev.x + (target.x - prev.x) * t, prev.y + (target.y - prev.y) * t);
        let mut animating = (self.cur_size - prev).length() > 0.08;
        if animating {
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.cur_size));
        }

        if !self.tweens.is_empty() {
            animating = true;
        }
        if let Some(t0) = self.refresh_at {
            if t0.elapsed().as_secs_f32() >= SPIN_SECS {
                self.refresh_at = None;
            } else {
                animating = true;
            }
        }

        // Expanded: 1 s tick so countdowns and ages stay live. Collapsed: cheap 30 s.
        if self.expanded {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        } else if !animating {
            ctx.request_repaint_after(std::time::Duration::from_secs(30));
        }
        if animating {
            ctx.request_repaint();
        }

        let span = (self.last_expanded_h - COLLAPSED_H).max(1.0);
        let f = (((self.cur_size.y - COLLAPSED_H) / span).clamp(0.0, 1.0)).min(1.0);
        let f = 1.0 - (1.0 - f) * (1.0 - f); // ease-out

        let frame = egui::Frame::none()
            .fill(pal.bg)
            .stroke(egui::Stroke::new(1.0_f32, pal.border))
            .rounding(egui::Rounding::same(17.0))
            .inner_margin(egui::Margin::symmetric(11.0, if self.expanded { 8.0 } else { 5.0 }));

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            // ============ header row ============
            let max_vis = if self.expanded {
                snaps.len()
            } else {
                snaps.len().min(self.cfg.max_visible_collapsed)
            };
            ui.horizontal(|ui| {
                // grip
                let (grect, _gr) = ui.allocate_exact_size(Vec2::new(18.0, HEADER_H), Sense::hover());
                ui.painter().image(
                    self.icons.grip.id(),
                    egui::Rect::from_center_size(grect.center(), Vec2::splat(15.0)),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    pal.muted.linear_multiply(0.75 + 0.25 * f),
                );
                let grip = ui.interact(grect.expand(4.0), ui.id().with("grip"), Sense::drag());
                if grip.drag_started() {
                    ctx.send_viewport_cmd(ViewportCommand::StartDrag);
                }
                ui.add_space(2.0);

                // provider chips
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 0.0);
                for s in snaps.iter().take(max_vis) {
                    let pct = self.render_pct(s);
                    let stale = self.is_stale(s, now);
                    let text_w = ui::chip_text_w(ctx, s, pct, &pal, 1.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui::chip_width(text_w), HEADER_H), Sense::hover());
                    let resp = ui.interact(rect, ui.id().with(("chip", &s.provider_id)), Sense::hover());
                    ui::draw_chip(ui, rect, s, pct, &pal, 1.0, stale, resp.hovered());
                    resp.on_hover_ui(|ui| ui::chip_tooltip(ui, s, &pal, stale));
                }
                if !self.expanded && snaps.len() > max_vis
                    && ui::widgets::overflow_chip(ui, snaps.len() - max_vis, &pal).clicked() {
                        self.expanded = true;
                    }
                if snaps.is_empty() {
                    ui.label(
                        RichText::new("no providers — edit config.toml").color(pal.faint).size(12.0),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let spin = self
                        .refresh_at
                        .map(|t0| t0.elapsed().as_secs_f32() / SPIN_SECS * std::f32::consts::TAU * 1.5);
                    if ui::widgets::icon_button(ui, &self.icons.min, "minimize (or press Esc)", f, &pal, None).clicked() {
                        self.expanded = false;
                    }
                    if ui::widgets::icon_button(ui, &self.icons.refresh, "refresh now", f, &pal, spin).clicked() {
                        self.refresh(ctx);
                    }
                    if ui::widgets::icon_button(ui, &self.icons.close, "quit", f, &pal, None).clicked() {
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
                self.refresh(ctx);
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) && self.expanded {
                self.expanded = false;
            }

            // ============ detail area (scrolls when many providers) ============
            if f > 0.02 {
                ui.add_space((1.0 - f) * 10.0); // content slides up as it appears
                ui.visuals_mut().override_text_color = Some(pal.text.linear_multiply(f));
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height((self.cur_size.y - HEADER_H - 34.0).max(60.0))
                    .auto_shrink([false, true])
                    .drag_to_scroll(true)
                    .show(ui, |ui| {
                        for s in &snaps {
                            let pct = self.render_pct(s);
                            let stale = self.is_stale(s, now);
                            ui::provider_card(ui, s, pct, &pal, f, stale);
                            ui.add_space(6.0);
                        }
                    });
            }
        });
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
    if !cfg.theme.is_empty() && !theme::THEMES.contains(&cfg.theme.as_str()) {
        eprintln!(
            "limitcue: unknown theme '{}' (known: {}), using midnight",
            cfg.theme,
            theme::THEMES.join(", ")
        );
    }
    let pal = theme::palette(&cfg.theme);
    if std::env::args().any(|a| a == "--once") {
        run_once(&cfg);
        return Ok(());
    }
    // Match the initial window size to the starting state (debug/screenshot aid:
    // LIMITCUE_UI_EXPANDED=1 opens already expanded at full size).
    let start_expanded = std::env::var("LIMITCUE_UI_EXPANDED").map(|v| v != "0").unwrap_or(false);
    let init_size = if start_expanded {
        Vec2::new(EXPANDED_W, MAX_EXPANDED_H)
    } else {
        Vec2::new(260.0, 30.0)
    };
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_inner_size(init_size)
            .with_position(egui::pos2(60.0, 40.0)),
        ..Default::default()
    };
    eframe::run_native(
        "limitcue",
        options,
        Box::new(move |cc| {
            theme::apply_style(&cc.egui_ctx, &pal);
            let icons = load_icons(&cc.egui_ctx);
            Ok(Box::new(App::new(cfg, pal, icons)))
        }),
    )
}
