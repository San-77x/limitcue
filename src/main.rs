mod config;
mod dock;
mod providers;
mod types;
mod ui;

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use eframe::egui::{self, Color32, Rect, RichText, Sense, Vec2, ViewportBuilder, ViewportCommand};

use config::Config;
use dock::Edge;
use providers::{
    billing::Billing, claude::Claude, codex::Codex, custom::Custom, kimi::Kimi, minimax::MiniMax,
    Provider,
};
use types::{now_unix, Reading, Snapshot};
use ui::theme::{self, Palette};

const ICON_PNGS: [(&str, &[u8]); 5] = [
    ("grip", include_bytes!("../assets/icons/grip-vertical.png")),
    ("min", include_bytes!("../assets/icons/minus.png")),
    ("refresh", include_bytes!("../assets/icons/refresh-cw.png")),
    ("close", include_bytes!("../assets/icons/x.png")),
    ("gear", include_bytes!("../assets/icons/settings.png")),
];

/// White brand logos (64px RGBA PNGs, see assets/logos/README.md). Unknown
/// providers simply aren't in the map and fall back to the monogram letter.
const LOGO_PNGS: [(&str, &[u8]); 6] = [
    ("claude", include_bytes!("../assets/logos/claude.png")),
    ("codex", include_bytes!("../assets/logos/codex.png")),
    ("gemini", include_bytes!("../assets/logos/gemini.png")),
    ("kimi", include_bytes!("../assets/logos/kimi.png")),
    ("minimax", include_bytes!("../assets/logos/minimax.png")),
    ("agentrouter", include_bytes!("../assets/logos/agentrouter.png")),
];

#[derive(Clone)]
struct Icons {
    grip: egui::TextureHandle,
    min: egui::TextureHandle,
    refresh: egui::TextureHandle,
    close: egui::TextureHandle,
    gear: egui::TextureHandle,
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
    Icons {
        grip: get("grip"),
        min: get("min"),
        refresh: get("refresh"),
        close: get("close"),
        gear: get("gear"),
    }
}

fn load_logos(ctx: &egui::Context) -> HashMap<String, egui::TextureHandle> {
    LOGO_PNGS
        .iter()
        .map(|(name, bytes)| {
            (
                name.to_string(),
                ctx.load_texture(format!("logo-{name}"), decode_png(bytes), egui::TextureOptions::LINEAR),
            )
        })
        .collect()
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
        if p.enabled == Some(false) {
            continue;
        }
        if p.billing {
            v.push(Box::new(Billing::new(p.clone())));
        } else if p.id == "minimax" {
            v.push(Box::new(MiniMax::new(p.clone())));
        } else if p.id == "kimi" {
            v.push(Box::new(Kimi::new(Some(p.clone()))));
        } else {
            v.push(Box::new(Custom::new(p.clone())));
        }
    }
    if !cfg.provider.iter().any(|p| p.id == "kimi" && p.enabled != Some(false)) {
        v.push(Box::new(Kimi::new(None)));
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

// Header-row geometry, shared by the collapsed pill (whose window width is
// derived from it) and the expanded header (whose chip budget is derived
// from it), so the two can't drift apart when a button is added or resized.
const H_MARGIN: f32 = 10.0; // horizontal frame inner margin
const GRIP_W: f32 = 18.0;
const GRIP_GAP: f32 = 2.0;
const HEADER_BUTTONS_W: f32 = 26.0 * 4.0 + 8.0 * 3.0; // min/gear/refresh/close + spacing
const HEADER_ROW_SPACING: f32 = 10.0; // chip/chip and chips/buttons gap
const COLLAPSED_H: f32 = 38.0;
const HEADER_H: f32 = 26.0;
const EXPANDED_W: f32 = 380.0;
const MAX_EXPANDED_H: f32 = 400.0;
const SETTINGS_W: f32 = 380.0;
const SETTINGS_H: f32 = 420.0;
const TWEEN_SECS: f32 = 0.45;
const SPIN_SECS: f32 = 0.55;

// Side-dock rail (left/right): a black notch hugging the screen edge —
// one ring cell per provider, an orb button below, and a tail-card usage
// popup beside the hovered cell. The window is STRIP + FLARE wide so the
// concave bezel fillets fit inside it while the visible spine stays slim.
const RAIL_STRIP_W: f32 = 40.0; // visible black notch spine
const RAIL_ROW_H: f32 = 54.0; // one ring cell (ring + percent)
const RAIL_ROW_GAP: f32 = 10.0;
const RAIL_COL_GAP: f32 = 8.0; // notch ↔ card gap when the card is open
const RAIL_CARD_W: f32 = 260.0; // usage card width
const RAIL_RING_R: f32 = 14.0; // ring radius in a cell
const RAIL_CORNER: f32 = 11.0; // convex corner radius of the notch body
const RAIL_FLARE: f32 = 10.0; // concave fillet where the body meets the edge
const RAIL_ORB: f32 = 20.0; // settings orb diameter
/// Ring stroke width in a rail cell, scaled for the small radius.
const RAIL_RING_STROKE: f32 = 1.6;
/// Card tail width (base at the notch edge, tip on the card).
const RAIL_TAIL_W: f32 = 12.0;
/// How long the card stays after the pointer leaves everything (grace for
/// crossing the gap between notch and card).
const RAIL_HOVER_OUT_SECS: f32 = 0.35;

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
    tx_tick: mpsc::Sender<Ctl>,
    cfg: Config,
    pal: Palette,
    expanded: bool,
    cur_size: Vec2,
    last_expanded_h: f32,
    refresh_at: Option<Instant>,
    icons: Icons,
    logos: HashMap<String, egui::TextureHandle>,
    dock: dock::SharedDock,
    restore_sent: u32,
    last_edge: Edge,
    cfg_next: Config,
    settings_open: bool,
    new_provider_id: String,
    /// Provider whose inline usage card is open on the rail (left/right dock).
    rail_open: Option<String>,
    /// Provider whose card is fading out (kept for height budgeting).
    rail_last: Option<String>,
    /// Last instant the pointer was over the rail/card — drives the card's
    /// hover-out grace period.
    rail_hover_at: Option<Instant>,
    /// Rail-card fade (0..1): springs toward 1 while hovered, toward 0 after.
    rail_card_f: f32,
    /// The notch is the primary surface. LIMITCUE_FLOAT=1 restores the pill.
    notch_mode: bool,
}

impl App {
    fn new(cfg: Config, pal: Palette, icons: Icons, logos: HashMap<String, egui::TextureHandle>, dock: dock::SharedDock) -> Self {
        let (tx_snap, rx_snap) = mpsc::channel::<Vec<Snapshot>>();
        let (tx_tick, rx_tick) = mpsc::channel::<Ctl>();
        let snapshots = load_state();
        spawn_poller(cfg.clone(), tx_snap, rx_tick);
        // Debug hooks: start expanded / with settings open / with a rail card
        // open (tests, screenshots).
        let expanded = std::env::var("LIMITCUE_UI_EXPANDED").map(|v| v != "0").unwrap_or(false);
        let settings_open = std::env::var("LIMITCUE_UI_SETTINGS").map(|v| v != "0").unwrap_or(false);
        let rail_open = std::env::var("LIMITCUE_UI_RAIL").ok().filter(|v| !v.is_empty());
        let notch_mode = std::env::var("LIMITCUE_FLOAT").map(|v| v == "0").unwrap_or(true);
        let cfg_next = cfg.clone();
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
            logos,
            dock,
            restore_sent: 0,
            last_edge: if notch_mode { Edge::Left } else { Edge::Free },
            cfg_next,
            settings_open,
            new_provider_id: String::new(),
            rail_open,
            rail_last: None,
            rail_hover_at: None,
            rail_card_f: 0.0,
            notch_mode,
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

    /// Draw the settings screen. Saves `self.cfg_next` back to config.toml on
    /// apply and pushes it to the poll thread (provider list, order, interval).
    fn settings_ui(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        use egui::{Align, Layout};
        let mut dirty = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Settings").size(15.0));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let w = ui::widgets::icon_button(ui, &self.icons.close, "close settings", 1.0, pal, None);
                if w.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.settings_open = false;
                }
            });
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.add_space(2.0);

                // ---- general ----
                ui.heading(RichText::new("General").size(12.0));
                let mut poll = self.cfg_next.poll_interval_secs;
                ui.add(egui::Slider::new(&mut poll, 30..=900).text("poll interval (s)"));
                if poll != self.cfg_next.poll_interval_secs {
                    self.cfg_next.poll_interval_secs = poll;
                }
                let mut theme_buf = self.cfg_next.theme.clone();
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(if theme_buf.is_empty() { "midnight (default)" } else { &theme_buf })
                    .show_ui(ui, |ui| {
                        for t in theme::THEMES {
                            ui.selectable_value(&mut theme_buf, t.to_string(), *t);
                        }
                    });
                if theme_buf != self.cfg_next.theme {
                    self.cfg_next.theme = theme_buf;
                }
                ui.add_space(6.0);

                // ---- providers ----
                ui.heading(RichText::new("Providers").size(12.0));
                let n = self.cfg_next.provider.len();
                for i in 0..n {
                    let (enabled, id, name, can_up, can_down, can_del) = {
                        let p = &self.cfg_next.provider[i];
                        let en = p.enabled.unwrap_or(true);
                        let id = p.id.clone();
                        let name = if p.name.is_empty() { id.clone() } else { p.name.clone() };
                        (en, id, name, i > 0, i + 1 < n, true)
                    };
                    ui.horizontal(|ui| {
                        let mut en = enabled;
                        if ui.checkbox(&mut en, "").changed() {
                            self.cfg_next.provider[i].enabled = Some(en);
                            dirty = true;
                        }
                        let label = if name != id { format!("{name}  ({id})") } else { name };
                        ui.label(RichText::new(label).size(12.0));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if can_del && ui.small_button("🗑").clicked() {
                                self.cfg_next.provider.remove(i);
                                dirty = true;
                            }
                            if can_down && ui.small_button("▾").clicked() {
                                self.cfg_next.provider.swap(i, i + 1);
                                dirty = true;
                            }
                            if can_up && ui.small_button("▴").clicked() {
                                self.cfg_next.provider.swap(i - 1, i);
                                dirty = true;
                            }
                        });
                    });
                }
                // built-ins: toggle only (code can't be reordered/removed)
                for b in ["claude", "codex"] {
                    let mut dis = self.cfg_next.disabled.contains(&b.to_string());
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut dis, "").changed() {
                            if dis {
                                self.cfg_next.disabled.push(b.to_string());
                            } else {
                                self.cfg_next.disabled.retain(|d| d != b);
                            }
                            dirty = true;
                        }
                        let label = match b {
                            "claude" => "Claude (built-in)",
                            _ => "Codex (built-in)",
                        };
                        ui.label(RichText::new(label).size(12.0));
                    });
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("New provider id:").size(11.5));
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.new_provider_id)
                            .desired_width(120.0)
                            .hint_text("e.g. openrouter"),
                    );
                    let dup = self.cfg_next.provider.iter().any(|p| p.id == self.new_provider_id)
                        || matches!(self.new_provider_id.as_str(), "claude" | "codex")
                        || self.new_provider_id.is_empty();
                    let add = ui.add_enabled(!dup, egui::Button::new("Add")).clicked();
                    if add {
                        self.cfg_next.provider.push(config::ProviderConfig {
                            id: self.new_provider_id.trim().to_string(),
                            name: String::new(),
                            url: None,
                            auth_header: None,
                            key_env: None,
                            api_key: None,
                            base_url: None,
                            windows: vec![],
                            enabled: Some(true),
                            billing: false,
                            priority: None,
                        });
                        self.new_provider_id.clear();
                        dirty = true;
                    }
                    if dup && !self.new_provider_id.is_empty() {
                        resp.on_hover_text("id already exists or is built-in");
                    }
                });
                ui.colored_label(
                    pal.faint,
                    "Keys live in config.toml (never stored by this window) — click Edit to open it.",
                )
                .on_hover_text("New/edited providers need their api_key or key_env set in config.toml.");
                if ui.small_button("Edit config.toml").clicked() {
                    let _ = std::process::Command::new("xdg-open").arg(config::Config::path()).spawn();
                }
                ui.add_space(6.0);

                // ---- display ----
                ui.heading(RichText::new("Display").size(12.0));
                let mut mv = self.cfg_next.max_visible_collapsed as i32;
                ui.add(egui::Slider::new(&mut mv, 1..=8).text("visible when collapsed"));
                self.cfg_next.max_visible_collapsed = mv.max(1) as usize;
                let mut hu = self.cfg_next.hide_unconfigured;
                if ui.checkbox(&mut hu, "hide unconfigured providers").changed() {
                    self.cfg_next.hide_unconfigured = hu;
                }
                ui.add_space(10.0);
            });

        if dirty {
            ui.ctx().request_repaint();
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                self.cfg = self.cfg_next.clone();
                config::Config::save(&self.cfg);
                self.pal = theme::palette(&self.cfg.theme);
                self.restart_poller();
                self.settings_open = false;
            }
            if ui.button("Cancel").clicked() {
                self.cfg_next = self.cfg.clone();
                self.settings_open = false;
            }
        });
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

    /// How many of `snaps` fit into `avail` px of chip row (chips consume
    /// `spacing + chip_width` each, the first one without leading spacing).
    /// Always shows at least one chip; hidden ones fold into the `+N` chip.
    /// Widths are measured at the widest stable label (`100%`) so the count
    /// can't grow when a real, narrower percent renders later.
    fn fitting_chips(
        ctx: &egui::Context,
        snaps: &[Snapshot],
        pal: &Palette,
        avail: f32,
        max_any: usize,
    ) -> usize {
        let mut used = 0.0;
        let mut n = 0;
        for (i, s) in snaps.iter().enumerate() {
            if i >= max_any {
                break;
            }
            let text_w = ui::chip_text_w(ctx, s, Some(100.0), pal, 1.0);
            let w = ui::chip_width(text_w) + if i == 0 { 0.0 } else { HEADER_ROW_SPACING };
            if i > 0 && used + w > avail {
                break;
            }
            used += w;
            n += 1;
        }
        n.max(1).min(snaps.len().max(1))
    }

    /// Collapsed pill width that fits its content exactly.
    fn collapsed_width(&self, ctx: &egui::Context, snaps: &[Snapshot]) -> f32 {
        let max_vis = snaps.len().min(self.cfg.max_visible_collapsed);
        // The collapsed state is a real edge pill, not a shrunken window.
        // Controls are not laid out until expanded, so the shell hugs the
        // provider chips instead of reserving invisible button space.
        let mut w = H_MARGIN * 2.0 // frame margins
            + GRIP_W + GRIP_GAP; // grip + gap
        for s in snaps.iter().take(max_vis) {
            w += HEADER_ROW_SPACING + ui::chip_width(ui::chip_text_w(ctx, s, self.render_pct(s), &self.pal, 1.0));
        }
        if snaps.len() > max_vis {
            w += HEADER_ROW_SPACING + 32.0; // +N chip
        }
        if snaps.is_empty() {
            w = w.max(280.0);
        }
        w
    }

    /// Restart the poll thread after a config change (provider set, order,
    /// interval). The old thread exits on its own when its channel closes.
    fn restart_poller(&mut self) {
        let (tx_snap, rx_snap) = mpsc::channel::<Vec<Snapshot>>();
        let (tx_tick, rx_tick) = mpsc::channel::<Ctl>();
        self.rx = rx_snap;
        self.tx_tick = tx_tick;
        spawn_poller(self.cfg.clone(), tx_snap, rx_tick);
    }

    fn refresh(&mut self, ctx: &egui::Context) {
        let _ = self.tx_tick.send(Ctl::Refresh);
        self.refresh_at = Some(Instant::now());
        ctx.request_repaint();
    }
}

/// Rail-card spring rate for the fade in/out (per-second exponential).
fn t_rail_spring(dt: f32) -> f32 {
    1.0 - (-9.0 * dt).exp()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain(ctx);
        self.tweens.retain(|_, t| t.value().is_some());
        let stored_edge = self.dock.lock().unwrap().edge;
        let edge = if self.notch_mode && stored_edge == Edge::Free { Edge::Left } else { stored_edge };
        if edge != self.last_edge {
            self.last_edge = edge;
            ctx.request_repaint();
        }

        // Ask KWin to apply the persisted dock position once, after the
        // window exists. Retries during the first seconds: KWin only sees
        // the window after it maps, and the app settles its own size during
        // the first frames (pill width animation).
        if self.restore_sent < 20 {
            self.restore_sent += 1;
            let st = *self.dock.lock().unwrap();
            if (st.edge != Edge::Free || st.x != 0 || st.y != 0) && self.restore_sent.is_multiple_of(5) {
                dock::request_restore(&st);
            }
        }

        let pal = self.pal;
        let now = now_unix();
        let snaps = self.visible();
        // Left/right dock: the pill becomes a vertical rail (rings + %).
        // In notch mode this is the only primary surface: there is no
        // dashboard-style expand state. Details are revealed by hovering a
        // provider socket and live in the adjacent contextual card.
        let rail = self.notch_mode || matches!(edge, Edge::Left | Edge::Right);
        if self.notch_mode {
            self.expanded = false;
        }

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

        let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.1);

        // Rail: notch body only when nothing is hovered; body + tail-card
        // while a provider is hovered (card fades/springs, so its height is
        // budgeted at full once the pointer is on it). The rail shows one
        // cell per provider — max_visible_collapsed only limits the
        // horizontal pill.
        let rail_rows = snaps.len().max(1) as f32;
        let rail_h = rail_rows * RAIL_ROW_H + (rail_rows - 1.0) * RAIL_ROW_GAP + RAIL_ORB + 10.0;
        let card_want = self.rail_open.is_some();
        let card_f_target = if card_want { 1.0 } else { 0.0 };
        self.rail_card_f += (card_f_target - self.rail_card_f) * (t_rail_spring(dt));
        if self.rail_card_f < 0.01 {
            self.rail_card_f = 0.0;
        }
        if card_want {
            self.rail_card_f = self.rail_card_f.max(0.02);
        }
        let rail_size = if self.rail_card_f > 0.0 {
            let id = self.rail_open.clone().or_else(|| {
                // fading out: keep sizing for the card that just closed
                self.rail_last.clone()
            });
            let card_h = ui::rail_card_height(id.as_deref().and_then(|i| snaps.iter().find(|s| s.provider_id == *i)));
            Vec2::new(
                RAIL_STRIP_W + RAIL_COL_GAP + RAIL_CARD_W,
                (rail_h + RAIL_FLARE).max(card_h + 4.0),
            )
        } else {
            // Spine plus a transparent zone so the bezel fillets fit inside
            // the window instead of being clipped at rest.
            Vec2::new(RAIL_STRIP_W + RAIL_FLARE, rail_h)
        };

        let target = if rail {
            rail_size
        } else if self.expanded {
            expanded_size
        } else {
            Vec2::new(self.collapsed_width(ctx, &snaps), COLLAPSED_H)
        };
        // Debug hook: jump to the final size instead of animating (screenshot automation).
        // Sends the size unconditionally — snapping makes cur==target, which would
        // otherwise never trip the `animating` branch below.
        let snap = std::env::var("LIMITCUE_UI_SNAP").map(|v| v != "0").unwrap_or(false);
        if snap {
            self.cur_size = target;
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.cur_size));
        }

        let t = 1.0 - (-18.0 * dt).exp();
        let prev = self.cur_size;
        self.cur_size = Vec2::new(prev.x + (target.x - prev.x) * t, prev.y + (target.y - prev.y) * t);
        let mut animating = (self.cur_size - prev).length() > 0.08 && !snap;
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

        // Edge docking: square the two corners on the attached edge; when
        // docked at the bottom, the layout flips so the detail card grows
        // *up* (header/pill stays nearest the screen edge).
        let rounding = if rail {
            // the notch body paints its own corners; keep the frame square
            egui::Rounding::ZERO
        } else {
            match edge {
                Edge::Top => egui::Rounding { nw: 0.0, ne: 0.0, sw: 17.0, se: 17.0 },
                Edge::Bottom => egui::Rounding { nw: 17.0, ne: 17.0, sw: 0.0, se: 0.0 },
                Edge::Left => egui::Rounding { nw: 0.0, sw: 0.0, ne: 17.0, se: 17.0 },
                Edge::Right => egui::Rounding { nw: 17.0, sw: 17.0, ne: 0.0, se: 0.0 },
                Edge::Free => egui::Rounding::same(17.0),
            }
        };
        let flip = edge == Edge::Bottom;

        let frame = if rail {
            // Transparent host: the notch paints its own black body and fillets.
            egui::Frame::none()
        } else {
            egui::Frame::none()
                .fill(pal.bg)
                .stroke(egui::Stroke::new(1.0_f32, pal.border))
                .rounding(rounding)
                .inner_margin(egui::Margin::symmetric(11.0, if self.expanded { 8.0 } else { 5.0 }))
        };

        // Settings replaces the whole layout (pill grows to a panel).
        if self.settings_open {
            self.cur_size = Vec2::new(SETTINGS_W, SETTINGS_H);
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.cur_size));
            let frame = egui::Frame::none()
                .fill(pal.bg)
                .stroke(egui::Stroke::new(1.0_f32, pal.border))
                .rounding(egui::Rounding::same(17.0))
                .inner_margin(egui::Margin::same(14.0));
            egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                self.settings_ui(ui, &pal);
            });
            return;
        }

        // Render header + detail; for bottom dock, reverse order so the pill
        // row ends up at the bottom (detail grows upward, away from the edge).
        let header = |ui: &mut egui::Ui, this: &mut App| {
            if rail {
                this.render_rail(ui, ctx, &snaps);
            } else {
                this.render_header(ui, ctx, &snaps, f, now);
            }
        };
        let detail = |ui: &mut egui::Ui, this: &mut App| {
            // Rail mode has no detail area — everything lives in the notch
            // and its tail-card.
            if !rail && f > 0.02 {
                this.render_detail(ui, &snaps, f, now);
            }
        };

        egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
            if flip {
                detail(ui, self);
                header(ui, self);
            } else {
                header(ui, self);
                detail(ui, self);
            }
        });
    }
}

impl App {
    fn render_header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, snaps: &[Snapshot], f: f32, now: u64) {
        let pal = self.pal;
        {
            // The header row is grip+buttons (RTL) plus chips (LTR) sharing one
            // fixed width. Budget the chips so they can never run under the
            // buttons; anything that doesn't fit folds into the `+N` chip.
            let chips_avail = if self.expanded {
                EXPANDED_W
                    - H_MARGIN * 2.0
                    - GRIP_W
                    - GRIP_GAP
                    - HEADER_BUTTONS_W
                    - HEADER_ROW_SPACING
            } else {
                f32::INFINITY
            };
            let hard_cap = if self.expanded {
                snaps.len()
            } else {
                self.cfg.max_visible_collapsed
            };
            let max_vis = if self.expanded && chips_avail.is_finite() {
                Self::fitting_chips(ctx, snaps, &pal, chips_avail, hard_cap)
            } else {
                snaps.len().min(hard_cap)
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
                    ui::draw_chip(ui, rect, s, pct, &pal, 1.0, stale, resp.hovered(), &self.logos);
                    resp.on_hover_ui(|ui| ui::chip_tooltip(ui, s, &pal, stale));
                }
                if snaps.len() > max_vis
                    && ui::widgets::overflow_chip(ui, snaps.len() - max_vis, &pal).clicked()
                {
                    self.expanded = true;
                }
                if snaps.is_empty() {
                    ui.label(
                        RichText::new("no providers — edit config.toml").color(pal.faint).size(12.0),
                    );
                }

                if self.expanded {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let spin = self
                            .refresh_at
                            .map(|t0| t0.elapsed().as_secs_f32() / SPIN_SECS * std::f32::consts::TAU * 1.5);
                        if ui::widgets::icon_button(ui, &self.icons.min, "minimize (or press Esc)", f, &pal, None).clicked() {
                            self.expanded = false;
                        }
                        if ui::widgets::icon_button(ui, &self.icons.gear, "settings", f, &pal, None).clicked() {
                            self.settings_open = true;
                        }
                        if ui::widgets::icon_button(ui, &self.icons.refresh, "refresh now", f, &pal, spin).clicked() {
                            self.refresh(ctx);
                        }
                        if ui::widgets::icon_button(ui, &self.icons.close, "quit", f, &pal, None).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                }
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
        }
    }

    /// Vertical rail shown when docked left/right: a pure-black notch body
    /// with concave fillets where it meets the screen edge, one ring cell per
    /// provider, and a settings orb below. Hovering a cell (or the gap+card
    /// while the card is open) slides the tail-card usage popup out beside
    /// that cell; it lingers through a short grace period after the pointer
    /// leaves.
    fn render_rail(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, snaps: &[Snapshot]) {
        let pal = self.pal;
        let edge = self.last_edge;
        // While the edge is unknown the window may still be the tiny init
        // pill — don't render the notch into a rect it can't fit in.
        if edge == Edge::Free {
            return;
        }
        let now = now_unix();
        let on_left = edge == Edge::Left;
        let ui_rect = ui.max_rect();
        let pointer = ctx.input(|i| i.pointer.hover_pos());

        // ---- hover bookkeeping: does the card stay open? -----------------
        let over_card = self.rail_card_f > 0.0 && pointer.is_some_and(|pt| {
            // Card x-range plus the gap toward the strip, so crossing the
            // gap (or sitting anywhere on the card) doesn't count as "out".
            let (x0, x1) = if on_left {
                (RAIL_STRIP_W - 2.0, ui_rect.right())
            } else {
                (-2.0, RAIL_COL_GAP + 2.0)
            };
            pt.x >= x0 && pt.x <= x1 && pt.y >= ui_rect.top() && pt.y <= ui_rect.bottom()
        });

        // ---- the notch body ----------------------------------------------
        // The strip hugs the docked edge: window-left for a left dock,
        // window-right for a right dock (the card then fills the remainder).
        let body_x = if on_left { ui_rect.left() } else { ui_rect.right() - RAIL_STRIP_W };
        let body = Rect::from_min_size(egui::pos2(body_x, ui_rect.top()), Vec2::new(RAIL_STRIP_W, ui_rect.height()));
        let (c0, c1) = if on_left {
            (egui::pos2(body.right(), body.top()), egui::pos2(body.right(), body.bottom()))
        } else {
            (egui::pos2(body.left(), body.top()), egui::pos2(body.left(), body.bottom()))
        };
        let r = RAIL_CORNER.min(body.height() / 2.0);
        // body with two square edge-corners and two 15px rounded corners
        let body_rounding = if on_left {
            egui::Rounding { nw: 0.0, sw: 0.0, ne: r, se: r }
        } else {
            egui::Rounding { nw: r, sw: r, ne: 0.0, se: 0.0 }
        };
        ui.painter().rect_filled(body, body_rounding, pal.rail_bg);
        // concave fillets at the two square corners (into the bezel)
        let (along, away) = if on_left { (1.0, 1.0) } else { (-1.0, -1.0) };
        ui::widgets::edge_flare(ui, c0, along, away, RAIL_FLARE, pal.rail_bg, 1.0);
        ui::widgets::edge_flare(ui, c1, along, -away, RAIL_FLARE, pal.rail_bg, 1.0);

        // ---- whole-notch drag surface --------------------------------------
        // Any drag on the body (left or middle button) hands off to the
        // compositor's native window move; the KWin integration re-snaps to
        // the nearer side when the move ends. No visible grip needed.
        let drag = ui.interact(body, ui.id().with("rail-drag"), Sense::drag());
        if drag.drag_started_by(egui::PointerButton::Primary)
            || drag.drag_started_by(egui::PointerButton::Middle)
        {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        // ---- ring cells ---------------------------------------------------
        let cell_w = RAIL_STRIP_W;
        let mut hovered_id: Option<String> = None;
        let mut hover_row_cy: f32 = body.center().y;
        let mut next_y = body.top();
        for s in snaps {
            let row_rect = Rect::from_min_size(egui::pos2(body.left(), next_y), Vec2::new(cell_w, RAIL_ROW_H));
            next_y = row_rect.bottom() + RAIL_ROW_GAP;
            let pct = self.render_pct(s);
            let stale = self.is_stale(s, now);
            let ok = matches!(s.reading, Reading::Ok { .. });
            let ring_col = if stale {
                ui::theme::mix(ui::widgets::pct_color(pct, ok, &pal), pal.stale, 0.6)
            } else {
                ui::widgets::pct_color(pct, ok, &pal)
            };
            let resp = ui.allocate_rect(row_rect, Sense::hover());
            let hovered = resp.hovered();
            if hovered {
                hovered_id = Some(s.provider_id.clone());
                hover_row_cy = row_rect.center().y;
            }
            // ring + white logo (dark disc), centered in the cell
            let ring_center = egui::pos2(row_rect.center().x, row_rect.center().y - 8.0);
            ui::widgets::logo_ring_stroke_on(
                ui,
                ring_center,
                RAIL_RING_R,
                self.logos.get(&s.provider_id),
                &ui::theme::monogram(&s.provider_id),
                ui::theme::brand(&s.provider_id, &pal),
                (pct.unwrap_or(0.0) / 100.0) as f32,
                ring_col,
                &pal,
                1.0,
                pal.rail_disc,
                RAIL_RING_STROKE,
            );
            // percent under the ring (used %, white; auth/err labels colored)
            let label = match (&s.reading, pct) {
                (Reading::Ok { .. }, Some(v)) => format!("{:.0}%", 100.0 - v),
                (Reading::Ok { .. }, None) => "…".into(),
                (Reading::NeedsAuth(_), _) => "auth".into(),
                (Reading::Error(_), _) => "err".into(),
                _ => "?".into(),
            };
            let label_col = match &s.reading {
                Reading::Ok { .. } => Color32::WHITE,
                Reading::NeedsAuth(_) => pal.warn,
                Reading::Error(_) => pal.bad,
                _ => pal.muted,
            };
            ui.painter().text(
                egui::pos2(row_rect.center().x, row_rect.bottom() - 10.0),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(11.5),
                label_col,
            );
        }

        // ---- settings orb --------------------------------------------------
        let orb_rect = Rect::from_center_size(
            egui::pos2(body.center().x, next_y + RAIL_ORB / 2.0),
            Vec2::splat(RAIL_ORB),
        );
        // Click-drag on the orb also moves the window (it's part of the
        // notch surface); a plain click still opens settings.
        let orb = ui.allocate_rect(orb_rect, Sense::click_and_drag());
        let orb_hover = orb.hovered();
        ui.painter().circle_filled(
            orb_rect.center(),
            RAIL_ORB / 2.0,
            if orb_hover { pal.rail_disc } else { pal.rail_deep },
        );
        // arc at rest, gear on hover (the spec's orb behavior)
        if orb_hover {
            let gear_center = orb_rect.center();
            ui.painter().image(
                self.icons.gear.id(),
                Rect::from_center_size(gear_center, Vec2::splat(15.0)),
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            ui::widgets::ring(ui, orb_rect.center(), 6.0, 1.8, 0.75, pal.muted, &pal, 0.9);
        }
        if orb.clicked() {
            self.settings_open = true;
        }
        if orb.drag_started_by(egui::PointerButton::Primary)
            || orb.drag_started_by(egui::PointerButton::Middle)
        {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        orb.on_hover_text("settings · drag the notch to move it");

        // ---- hover state machine ------------------------------------------
        // (Debug runs seeded with LIMITCUE_UI_RAIL pin the card open so
        // screenshots don't depend on pointer placement.)
        let over_body = pointer.is_some_and(|pt| body.contains(pt));
        if over_body || over_card {
            self.rail_hover_at = Some(Instant::now());
        }
        let in_grace = self
            .rail_hover_at
            .map(|t0| t0.elapsed().as_secs_f32() < RAIL_HOVER_OUT_SECS)
            .unwrap_or(false);
        // Debug runs seeded with LIMITCUE_UI_RAIL keep that card open without
        // a pointer on it (screenshot automation); any hover releases the pin.
        let seeded = std::env::var("LIMITCUE_UI_RAIL").ok().filter(|v| !v.is_empty());
        let pinned = self.rail_hover_at.is_none()
            && self.rail_open.is_some()
            && seeded.as_deref() == self.rail_open.as_deref();
        if let Some(id) = hovered_id.clone() {
            self.rail_open = Some(id);
        } else if !pinned && !in_grace && !over_card {
            if let Some(prev) = self.rail_open.take() {
                self.rail_last = Some(prev);
            }
        }

        // ---- the tail-card popup ------------------------------------------
        if self.rail_card_f > 0.0 {
            let Some(id) = self.rail_open.clone().or_else(|| self.rail_last.clone()) else {
                return;
            };
            let Some(s) = snaps.iter().find(|s| s.provider_id == id) else {
                self.rail_open = None;
                return;
            };
            let pct = self.render_pct(s);
            let stale = self.is_stale(s, now);
            let a = self.rail_card_f.clamp(0.0, 1.0);
            // The usage card is a small floating bubble, not a second rail.
            // Keep it vertically attached to the hovered socket so the
            // pointer can cross the tail without the card jumping.
            let card_h = ui::rail_card_height(Some(s));
            // Center on the hovered row, but the card can be taller than the
            // strip — clamp with min<=max guaranteed (max() before min()).
            let card_y = (hover_row_cy - card_h / 2.0)
                .max(ui_rect.top() + 8.0)
                .min((ui_rect.bottom() - card_h - 8.0).max(ui_rect.top() + 8.0));
            let card_rect = Rect::from_min_size(
                egui::pos2(
                    if on_left {
                        RAIL_STRIP_W + RAIL_COL_GAP
                    } else {
                        0.0
                    },
                    card_y,
                ),
                Vec2::new(RAIL_CARD_W, card_h),
            );
            // The card paints into its own rect, but the tail must cross the
            // gap between card and strip — draw through the ui painter so the
            // tip isn't clipped at the card boundary.
            let card_p = ui.painter();
            let fill = pal.rail_deep.linear_multiply(a);
            card_p.rect_filled(card_rect, 16.0_f32, fill);
            // tail triangle from the card edge toward the hovered cell
            let tail_base_x = if on_left { card_rect.left() } else { card_rect.right() };
            let tip_x = if on_left {
                card_rect.left() - RAIL_TAIL_W
            } else {
                card_rect.right() + RAIL_TAIL_W
            };
            // Pinned (debug-seeded) cards: point the tail at that provider's
            // row so screenshots don't depend on pointer placement.
            let row_cy = if pinned {
                snaps
                    .iter()
                    .position(|s| s.provider_id == id)
                    .map(|i| body.top() + (i as f32 + 0.5) * RAIL_ROW_H + i as f32 * RAIL_ROW_GAP)
                    .unwrap_or(hover_row_cy)
            } else {
                hover_row_cy
            };
            let tw = 9.0; // half the tail's vertical extent
            // clamp order matters when the card is shorter than the margins
            let (lo, hi) = (card_rect.top() + 24.0, card_rect.bottom() - 24.0);
            let cy = if lo <= hi {
                row_cy.clamp(lo, hi)
            } else {
                card_rect.center().y
            };
            card_p.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(tail_base_x, cy - tw),
                    egui::pos2(tail_base_x, cy + tw),
                    egui::pos2(tip_x, cy),
                ],
                fill,
                egui::Stroke::NONE,
            ));
            // content (scaled presence via alpha)
            let content_ui = &mut ui.new_child(
                egui::UiBuilder::new().max_rect(card_rect).layout(egui::Layout::top_down(egui::Align::Min)),
            );
            ui::rail_card(
                content_ui,
                s,
                pct,
                &pal,
                a,
                stale,
                &self.logos,
                now,
                RAIL_CARD_W,
            );
        }

        if ui.input(|i| i.key_pressed(egui::Key::R)) {
            self.refresh(ctx);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.rail_open = None;
        }
    }

    /// ============ detail area (scrolls when many providers) ============
    fn render_detail(&mut self, ui: &mut egui::Ui, snaps: &[Snapshot], f: f32, now: u64) {
        let pal = self.pal;
        ui.add_space((1.0 - f) * 10.0); // content slides in as it appears
        ui.visuals_mut().override_text_color = Some(pal.text.linear_multiply(f));
        // Small wordmark above the list (lives here, not in the chip row —
        // the header budget doesn't account for it).
        ui.label(
            RichText::new("LIMITCUE")
                .monospace()
                .size(9.5)
                .strong()
                .color(pal.accent.linear_multiply(0.85 * f)),
        );
        ui.label(
            RichText::new("/ USAGE")
                .monospace()
                .size(8.5)
                .color(pal.faint.linear_multiply(f)),
        );
        ui.separator();
        egui::ScrollArea::vertical()
            .max_height((self.cur_size.y - HEADER_H - 34.0).max(60.0))
            .auto_shrink([false, true])
            .drag_to_scroll(true)
            .show(ui, |ui| {
                for s in snaps {
                    let pct = self.render_pct(s);
                    let stale = self.is_stale(s, now);
                    ui::provider_card(ui, s, pct, &pal, f, stale, &self.logos);
                    ui.add_space(6.0);
                }
            });
    }
}

/// Messages the UI can send to the poll thread.
#[derive(Clone, Copy)]
enum Ctl {
    Refresh,
}

/// Background poll loop. Exits when the UI drops `rx_tick` (config change).
fn spawn_poller(
    cfg: Config,
    tx_snap: mpsc::Sender<Vec<Snapshot>>,
    rx_tick: mpsc::Receiver<Ctl>,
) {
    let providers = build_providers(&cfg);
    let poll_secs = cfg.poll_interval_secs;
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
            match rx_tick.recv_timeout(std::time::Duration::from_secs(backoff)) {
                Ok(Ctl::Refresh) => continue,
                Err(_) => break, // channel replaced (config change) or app quit
            }
        }
    });
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
    // D-Bus service must exist before the window so the KWin dock script can
    // restore the position at window-add time. Bus-less environments just
    // lose persistence, nothing else.
    let dock = dock::shared();
    if let Err(e) = dock::start_service(dock.clone()) {
        eprintln!("limitcue: no D-Bus session bus ({e}) — dock position won't persist");
    }
    // Match the initial window size to the starting state (debug/screenshot aid:
    // LIMITCUE_UI_EXPANDED=1 opens already expanded at full size).
    let start_expanded = std::env::var("LIMITCUE_UI_EXPANDED").map(|v| v != "0").unwrap_or(false);
    let notch_mode = std::env::var("LIMITCUE_FLOAT").map(|v| v == "0").unwrap_or(true);
    let init_size = if notch_mode {
        // Resting notch: spine + the transparent zone the bezel fillets
        // live in; height matches the rail's own layout math.
        let rail_h = 4.0 * RAIL_ROW_H + 3.0 * RAIL_ROW_GAP + RAIL_ORB + 10.0;
        Vec2::new(RAIL_STRIP_W + RAIL_FLARE, rail_h)
    } else if start_expanded {
        Vec2::new(EXPANDED_W, MAX_EXPANDED_H)
    } else {
        Vec2::new(260.0, 30.0)
    };
    // Debug launch position (screenshot automation): LIMITCUE_UI_POS=X,Y.
    let init_pos = std::env::var("LIMITCUE_UI_POS")
        .ok()
        .and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some(egui::pos2(x.parse().ok()?, y.parse().ok()?))
        })
        .unwrap_or_else(|| if notch_mode { egui::pos2(0.0, 120.0) } else { egui::pos2(60.0, 40.0) });
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_resizable(false)
            .with_inner_size(init_size)
            .with_position(init_pos),
        ..Default::default()
    };
    eframe::run_native(
        "limitcue",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            theme::apply_style(&cc.egui_ctx, &pal);
            let icons = load_icons(&cc.egui_ctx);
            let logos = load_logos(&cc.egui_ctx);
            Ok(Box::new(App::new(cfg, pal, icons, logos, dock)))
        }),
    )
}
