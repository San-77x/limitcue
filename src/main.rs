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
use providers::{claude::Claude, codex::Codex, kimi::Kimi, Provider};
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
        v.push(providers::adapter_for(p));
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
const SETTINGS_W: f32 = 420.0;
const VIEWPORT_H_MARGIN: f32 = 48.0;
const TWEEN_SECS: f32 = 0.45;
const SPIN_SECS: f32 = 0.55;

// Side-dock rail (left/right): a black notch hugging the screen edge —
// one gauge cell per provider, an orb button below, and a tail-card usage
// popup beside the hovered cell. The window is exactly the spine wide; the
// square corners sit on the screen edge, the card-side corners are rounded.
const RAIL_STRIP_W: f32 = 48.0; // compact visible notch spine
const RAIL_ROW_H: f32 = 56.0; // cell height when the percentage is visible
const RAIL_ROW_H_COMPACT: f32 = 40.0; // gauge-only cell height
const RAIL_ROW_GAP: f32 = 4.0;
const RAIL_COL_GAP: f32 = 10.0; // notch ↔ card gap when the card is open
const RAIL_CARD_W: f32 = 264.0; // usage card width
const RAIL_RING_R: f32 = 14.0; // compact gauge ring radius
const RAIL_CORNER: f32 = 14.0; // convex corner radius of the notch body
const RAIL_ORB: f32 = 20.0; // compact settings orb diameter
const RAIL_PAD_TOP: f32 = 10.0; // breathing room above the first cell
/// Gauge ring stroke width in a rail cell.
const RAIL_RING_STROKE: f32 = 2.6;
/// Status badge radius, and where it sits on the gauge rim: up and to the
/// right, the conventional badge corner. Rows overlap their neighbours' slack
/// rather than their rings, so it has room there even in a compact cell.
const RAIL_BADGE_R: f32 = 5.5;
const RAIL_BADGE_ANGLE: f32 = std::f32::consts::PI * 40.0 / 180.0;

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

/// Which face the Providers tab is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsView {
    List,
    Picker,
    Editor(usize),
}

/// Result of the editor's Test button.
enum Probe {
    Idle,
    Running,
    Done { ok: bool, text: String },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Providers,
    /// Kept under the old name for the `LIMITCUE_UI_SETTINGS` debug hook;
    /// the tab is labelled "Appearance".
    Personalization,
}

/// Settings sheet geometry.
const SETTINGS_PAD: f32 = 16.0;
/// Height reserved for the pinned action bar (rule + gap + buttons).
const SETTINGS_ACTIONS_H: f32 = 10.0 + 1.0 + 10.0 + 28.0;

/// Do two configs describe the same UI state? Drives the "unsaved changes"
/// hint — `Config` is not `PartialEq` and gaining that would mean deriving it
/// across every provider field, so compare the serialized form.
fn cfg_eq(a: &Config, b: &Config) -> bool {
    toml::to_string(a).ok() == toml::to_string(b).ok()
}

/// Remaining width, never negative — egui panics on a negative child size.
fn avail(ui: &egui::Ui) -> f32 {
    ui.available_width().max(1.0)
}

/// Small tracked caps label that titles a group of settings rows.
fn section(ui: &mut egui::Ui, title: &str, pal: &Palette) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(avail(ui), 16.0), Sense::hover());
    let col = theme::mix(pal.faint, pal.muted, 0.55);
    let g = ui.painter().layout_job(theme::caps_job(title, 9.5, col));
    ui.painter().galley(egui::pos2(r.left() + 2.0, r.top()), g, col);
    ui.add_space(7.0);
}

/// Rounded well that holds a group of rows.
fn card<R>(ui: &mut egui::Ui, pal: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::none()
        .fill(pal.control)
        .stroke(egui::Stroke::new(1.0_f32, pal.border))
        .rounding(egui::Rounding::same(11.0))
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing = Vec2::new(8.0, 0.0);
            add(ui)
        })
        .inner
}

/// Full-width rule separating two rows inside a [`card`].
fn hairline(ui: &mut egui::Ui, pal: &Palette) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(avail(ui), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0_f32, pal.border);
}

/// A settings row: title over an explanatory line on the left, the control
/// on the right. The description is what makes a setting self-explanatory,
/// so it is part of the row rather than a tooltip.
fn setting_row<R>(
    ui: &mut egui::Ui,
    title: &str,
    desc: &str,
    pal: &Palette,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.add_space(9.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(avail(ui), 15.0), Sense::hover());
    ui.painter().text(
        egui::pos2(r.left(), r.center().y),
        egui::Align2::LEFT_CENTER,
        title,
        theme::medium(12.5),
        pal.text,
    );
    ui.add_space(3.0);
    let (d, _) = ui.allocate_exact_size(Vec2::new(avail(ui), 13.0), Sense::hover());
    ui.painter().galley(
        egui::pos2(d.left(), d.top()),
        ui.painter().layout(desc.to_owned(), theme::sans(10.0), pal.faint, d.width()),
        pal.faint,
    );
    ui.add_space(8.0);
    let out = control(ui);
    ui.add_space(9.0);
    out
}

/// [`setting_row`] with the control right-aligned on the title line. Use it
/// when the control is compact (a button, a switch); the full-width variant
/// is for controls that want the whole row, like a slider.
fn control_row<R>(
    ui: &mut egui::Ui,
    title: &str,
    desc: &str,
    reserve: f32,
    pal: &Palette,
    control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.add_space(9.0);
    let mut out = None;
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(
            Vec2::new((ui.available_width() - reserve).max(1.0), 30.0),
            Sense::hover(),
        );
        ui.painter().text(
            egui::pos2(r.left(), r.top() + 1.0),
            egui::Align2::LEFT_TOP,
            title,
            theme::medium(12.5),
            pal.text,
        );
        ui.painter().galley(
            egui::pos2(r.left(), r.top() + 16.0),
            ui.painter().layout(desc.to_owned(), theme::sans(10.0), pal.faint, r.width()),
            pal.faint,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            out = Some(control(ui));
        });
    });
    ui.add_space(9.0);
    out.expect("control_row body runs exactly once")
}

/// [`setting_row`] whose control is a switch, laid out on the title line so
/// the affordance sits where the eye already is.
fn toggle_row(ui: &mut egui::Ui, title: &str, desc: &str, value: &mut bool, pal: &Palette) -> bool {
    let mut changed = false;
    ui.add_space(9.0);
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(
            Vec2::new((ui.available_width() - 42.0).max(1.0), 30.0),
            Sense::hover(),
        );
        ui.painter().text(
            egui::pos2(r.left(), r.top() + 1.0),
            egui::Align2::LEFT_TOP,
            title,
            theme::medium(12.5),
            pal.text,
        );
        ui.painter().galley(
            egui::pos2(r.left(), r.top() + 16.0),
            ui.painter().layout(desc.to_owned(), theme::sans(10.0), pal.faint, r.width()),
            pal.faint,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            changed = ui::widgets::switch(ui, value, pal).changed();
        });
    });
    ui.add_space(9.0);
    changed
}

/// The provider's white mark on a dark disc, dimmed when it is switched off.
fn provider_mark(
    ui: &mut egui::Ui,
    id: &str,
    logos: &HashMap<String, egui::TextureHandle>,
    pal: &Palette,
    enabled: bool,
) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
    let a = if enabled { 1.0 } else { 0.4 };
    ui.painter().circle_filled(r.center(), 13.0, pal.rail_disc);
    match logos.get(id) {
        Some(tex) => {
            ui.painter().image(
                tex.id(),
                Rect::from_center_size(r.center(), Vec2::splat(15.0)),
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE.linear_multiply(a),
            );
        }
        None => {
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                theme::monogram(id),
                theme::semibold(12.0),
                Color32::WHITE.linear_multiply(a),
            );
        }
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
    /// Screen y the notch should be painted at — the position the user chose.
    /// The host window is taller than the notch and sits *above* this, so the
    /// window's own y (what the dock script reports) is `notch_y - headroom`.
    notch_y: f32,
    /// How far down inside the window the notch is painted. This is the room
    /// a card gets to open upward.
    headroom: f32,
    /// Last (y, height) handed to the compositor, so it is asked only on change.
    placed: Option<(i32, i32)>,
    /// Last window y the dock script reported, to spot a genuine move.
    reported_y: f32,
    last_edge: Edge,
    cfg_next: Config,
    settings_open: bool,
    settings_tab: SettingsTab,
    settings_view: SettingsView,
    probe: Probe,
    probe_rx: Option<mpsc::Receiver<(bool, String)>>,
    /// Whether the editor is currently showing a key in the clear.
    key_revealed: bool,
    /// Measured height of the settings body, used to size the sheet.
    settings_body_h: f32,
    /// Provider whose inline usage card is open on the rail (left/right dock).
    rail_open: Option<String>,
    /// Provider whose card is fading out (kept for height budgeting).
    rail_last: Option<String>,

    /// Rail-card fade (0..1): springs toward 1 while hovered, toward 0 after.
    rail_card_f: f32,
    /// Per-provider focus weights keep rapid A → B → C switches smooth.
    rail_focus_weights: HashMap<String, f32>,
    /// Shared ambient dim weight, eased independently from provider focus.
    rail_dim_t: f32,
    /// The notch is the primary surface. LIMITCUE_FLOAT=1 restores the pill.
    notch_mode: bool,
    started: Instant,
    shot_requested: bool,
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
        let settings_var = std::env::var("LIMITCUE_UI_SETTINGS").unwrap_or_default();
        let settings_open = !settings_var.is_empty() && settings_var != "0";
        let settings_tab = match settings_var.as_str() {
            "providers" | "picker" | "editor" => SettingsTab::Providers,
            "appearance" => SettingsTab::Personalization,
            _ => SettingsTab::General,
        };
        // Screenshot hooks for the two sub-views of the Providers tab.
        let settings_view = match settings_var.as_str() {
            "picker" => SettingsView::Picker,
            "editor" => SettingsView::Editor(0),
            _ => SettingsView::List,
        };
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
            notch_y: dock.lock().map(|d| d.y as f32).unwrap_or(0.0),
            headroom: 0.0,
            placed: None,
            reported_y: f32::NAN,
            dock,
            restore_sent: 0,
            last_edge: if notch_mode { Edge::Left } else { Edge::Free },
            cfg_next,
            settings_open,
            settings_tab,
            settings_view,
            probe: Probe::Idle,
            probe_rx: None,
            key_revealed: false,
            settings_body_h: 0.0,
            rail_open,
            rail_last: None,
            rail_card_f: 0.0,
            rail_focus_weights: HashMap::new(),
            rail_dim_t: 0.0,
            notch_mode,
            started: Instant::now(),
            shot_requested: false,
        }
    }

    fn drain(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.probe_rx {
            if let Ok((ok, text)) = rx.try_recv() {
                self.probe = Probe::Done { ok, text };
                self.probe_rx = None;
                ctx.request_repaint();
            }
        }
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

    /// Draw the settings sheet. Saves `self.cfg_next` back to config.toml on
    /// Save and pushes it to the poll thread (provider list, order, interval).
    ///
    /// The sheet is three fixed bands — title, tabs, action bar — around one
    /// scrolling body, so the buttons are always reachable no matter how many
    /// providers are configured. Every control is hand-painted: egui's stock
    /// widgets speak a different visual language, and its arrow/trash glyphs
    /// are not in Inter, so they used to render as tofu boxes.
    fn settings_ui(&mut self, ui: &mut egui::Ui, pal: &Palette) {
        use ui::widgets as w;
        let mut dirty = false;
        // The window is still notch-sized on the frame the sheet opens; every
        // allocation below has to survive that before the resize lands.
        let full = ui.available_width().max(1.0);
        // The text field is the one stock egui widget left in the sheet;
        // restyle it here rather than in apply_style(), which runs once at
        // startup and so cannot follow the live theme preview.
        {
            let v = &mut ui.style_mut().visuals;
            v.extreme_bg_color = pal.bg;
            v.selection.bg_fill = pal.accent.gamma_multiply(0.30);
            v.selection.stroke = egui::Stroke::new(1.0_f32, pal.accent);
            v.text_cursor.stroke = egui::Stroke::new(1.4_f32, pal.accent);
            v.widgets.inactive.rounding = egui::Rounding::same(8.0);
            v.widgets.hovered.rounding = egui::Rounding::same(8.0);
            v.widgets.active.rounding = egui::Rounding::same(8.0);
            v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, pal.border);
            v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, pal.muted);
            v.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, pal.accent);
        }

        // ---- title band --------------------------------------------------
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new((full - 26.0).max(1.0), 24.0), Sense::hover());
            ui.painter().text(
                egui::pos2(r.left(), r.center().y),
                egui::Align2::LEFT_CENTER,
                "Settings",
                theme::semibold(16.0),
                pal.text,
            );
            let name = ui.painter().layout_job(theme::caps_job("limitcue", 9.0, pal.faint));
            ui.painter().galley(
                egui::pos2(r.right() - name.rect.width(), r.center().y - name.rect.height() / 2.0),
                name,
                pal.faint,
            );
            if w::glyph_button(ui, w::Mark::Cross, true, "close", pal).clicked() {
                self.settings_open = false;
            }
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.settings_open = false;
        }
        ui.add_space(12.0);

        // ---- tabs ----------------------------------------------------------
        let tabs = ["General", "Providers", "Appearance"];
        let cur = match self.settings_tab {
            SettingsTab::General => 0,
            SettingsTab::Providers => 1,
            SettingsTab::Personalization => 2,
        };
        let picked = w::segmented(ui, &tabs, cur, pal);
        if picked != cur {
            self.settings_view = SettingsView::List;
            self.probe = Probe::Idle;
            self.settings_tab = match picked {
                0 => SettingsTab::General,
                1 => SettingsTab::Providers,
                _ => SettingsTab::Personalization,
            };
        }
        ui.add_space(14.0);

        // ---- scrolling body -------------------------------------------------
        let body_h = (ui.available_height() - SETTINGS_ACTIONS_H).max(80.0);
        let out = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(body_h)
            .show(ui, |ui| match self.settings_tab {
                SettingsTab::General => dirty |= self.settings_general(ui, pal),
                SettingsTab::Providers => dirty |= self.settings_providers(ui, pal),
                SettingsTab::Personalization => dirty |= self.settings_appearance(ui, pal),
            });
        // Feed the measured content height back into next frame's window size.
        // Content width is fixed, so this settles in one frame rather than
        // oscillating.
        let measured = out.content_size.y;
        if (measured - self.settings_body_h).abs() > 0.5 {
            self.settings_body_h = measured;
            ui.ctx().request_repaint();
        }

        // ---- action bar -----------------------------------------------------
        ui.add_space(10.0);
        let (rule_r, _) = ui.allocate_exact_size(Vec2::new(full, 1.0), Sense::hover());
        ui.painter().rect_filled(rule_r, 0.0_f32, pal.border);
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let changed = !cfg_eq(&self.cfg, &self.cfg_next);
            let hint = if changed { "Unsaved changes" } else { "All changes saved" };
            let (r, _) = ui.allocate_exact_size(Vec2::new((full - 150.0).max(1.0), 28.0), Sense::hover());
            ui.painter().text(
                egui::pos2(r.left(), r.center().y),
                egui::Align2::LEFT_CENTER,
                hint,
                theme::sans(10.5),
                if changed { pal.warn } else { pal.faint },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if w::pill_button(ui, "Save", true, true, pal).clicked() {
                    self.cfg = self.cfg_next.clone();
                    config::Config::save(&self.cfg);
                    self.pal = theme::palette(&self.cfg.theme);
                    // Re-apply the global style: tooltips and the scrollbar
                    // are styled once at startup and would otherwise keep the
                    // old theme's colors until the next launch.
                    theme::apply_style(ui.ctx(), &self.pal);
                    self.restart_poller();
                    self.settings_open = false;
                }
                ui.add_space(8.0);
                if w::pill_button(ui, "Cancel", false, true, pal).clicked() {
                    self.cfg_next = self.cfg.clone();
                    self.settings_open = false;
                }
            });
        });

        if dirty {
            ui.ctx().request_repaint();
        }
    }

    /// General: how often to poll, and what the notch shows at rest.
    fn settings_general(&mut self, ui: &mut egui::Ui, pal: &Palette) -> bool {
        use ui::widgets as w;
        let mut dirty = false;
        section(ui, "Updates", pal);
        card(ui, pal, |ui| {
            setting_row(ui, "Poll interval", "How often each provider is asked for fresh numbers.", pal, |ui| {
                let mut v = self.cfg_next.poll_interval_secs as i64;
                if w::slider(ui, &mut v, 30..=900, 30, "s", pal) {
                    self.cfg_next.poll_interval_secs = v as u64;
                    dirty = true;
                }
            });
        });

        ui.add_space(16.0);
        section(ui, "Notch", pal);
        card(ui, pal, |ui| {
            dirty |= toggle_row(
                ui,
                "Show percentages",
                "Print the used share under each gauge instead of the ring alone.",
                &mut self.cfg_next.show_rail_percent,
                pal,
            );
            hairline(ui, pal);
            dirty |= toggle_row(
                ui,
                "Quiet at rest",
                "Fade the notch until the pointer is over it.",
                &mut self.cfg_next.quiet_mode,
                pal,
            );
            hairline(ui, pal);
            dirty |= toggle_row(
                ui,
                "Hide unconfigured",
                "Keep providers you have not set up out of the notch.",
                &mut self.cfg_next.hide_unconfigured,
                pal,
            );
        });

        ui.add_space(16.0);
        section(ui, "Config file", pal);
        card(ui, pal, |ui| {
            control_row(
                ui,
                "config.toml",
                "API keys live here and are never held by this window.",
                72.0,
                pal,
                |ui| {
                    if w::pill_button(ui, "Open", false, true, pal).clicked() {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(config::Config::path())
                            .spawn();
                    }
                },
            );
        });
        ui.add_space(8.0);
        dirty
    }

    /// Providers: the tracked list, the built-in adapters, and the flow for
    /// adding a new one. Three views share the tab — the list, a picker of
    /// everything the catalogue knows, and a per-provider editor.
    fn settings_providers(&mut self, ui: &mut egui::Ui, pal: &Palette) -> bool {
        match self.settings_view {
            SettingsView::Picker => self.settings_picker(ui, pal),
            SettingsView::Editor(i) if i < self.cfg_next.provider.len() => {
                self.settings_editor(ui, pal, i)
            }
            _ => {
                self.settings_view = SettingsView::List;
                self.settings_provider_list(ui, pal)
            }
        }
    }

    fn settings_provider_list(&mut self, ui: &mut egui::Ui, pal: &Palette) -> bool {
        use ui::widgets as w;
        let mut dirty = false;
        section(ui, "Tracked providers", pal);
        card(ui, pal, |ui| {
            let n = self.cfg_next.provider.len();
            let mut action: Option<(usize, i32)> = None; // -1 up / 1 down / 0 delete / 2 edit
            for i in 0..n {
                if i > 0 {
                    hairline(ui, pal);
                }
                let (id, name, mut enabled) = {
                    let p = &self.cfg_next.provider[i];
                    (
                        p.id.clone(),
                        if p.name.is_empty() { p.id.clone() } else { p.name.clone() },
                        p.enabled.unwrap_or(true),
                    )
                };
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    provider_mark(ui, &id, &self.logos, pal, enabled);
                    ui.add_space(9.0);
                    let right = 22.0 * 4.0 + 34.0 + 18.0;
                    let (r, _) = ui.allocate_exact_size(
                        Vec2::new((ui.available_width() - right).max(1.0), 34.0),
                        Sense::hover(),
                    );
                    // The row is the way in to the editor, so the whole title
                    // block is clickable, not just the pencil.
                    if ui
                        .interact(r, ui.id().with(("prow", i)), Sense::click())
                        .on_hover_text("edit this provider")
                        .clicked()
                    {
                        action = Some((i, 2));
                    }
                    let title_col = if enabled { pal.text } else { pal.faint };
                    let g = w::elide(ui, &name, theme::medium(12.5), title_col, r.width());
                    ui.painter().galley(egui::pos2(r.left(), r.top() + 3.0), g, title_col);
                    let sub = self.provider_summary(i);
                    let g2 = w::elide(ui, &sub, theme::sans(10.0), pal.faint, r.width());
                    ui.painter().galley(egui::pos2(r.left(), r.top() + 18.0), g2, pal.faint);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if w::switch(ui, &mut enabled, pal).changed() {
                            self.cfg_next.provider[i].enabled = Some(enabled);
                            dirty = true;
                        }
                        ui.add_space(6.0);
                        if w::glyph_button(ui, w::Mark::Cross, true, "remove provider", pal).clicked() {
                            action = Some((i, 0));
                        }
                        if w::glyph_button(ui, w::Mark::Down, i + 1 < n, "move down", pal).clicked() {
                            action = Some((i, 1));
                        }
                        if w::glyph_button(ui, w::Mark::Up, i > 0, "move up", pal).clicked() {
                            action = Some((i, -1));
                        }
                        if w::glyph_button(ui, w::Mark::Pencil, true, "edit", pal).clicked() {
                            action = Some((i, 2));
                        }
                    });
                });
                ui.add_space(4.0);
            }
            if n == 0 {
                let (r, _) = ui.allocate_exact_size(Vec2::new(avail(ui), 34.0), Sense::hover());
                ui.painter().text(
                    egui::pos2(r.left(), r.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Nothing added yet — the built-ins below may already cover you.",
                    theme::sans(11.0),
                    pal.faint,
                );
            }
            // Applied after the loop so the list is not mutated mid-iteration.
            match action {
                Some((i, 0)) => {
                    self.cfg_next.provider.remove(i);
                    dirty = true;
                }
                Some((i, 1)) => {
                    self.cfg_next.provider.swap(i, i + 1);
                    dirty = true;
                }
                Some((i, -1)) => {
                    self.cfg_next.provider.swap(i - 1, i);
                    dirty = true;
                }
                Some((i, 2)) => {
                    self.settings_view = SettingsView::Editor(i);
                    self.probe = Probe::Idle;
                }
                _ => {}
            }
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if w::pill_button(ui, "Add a provider", true, true, pal).clicked() {
                self.settings_view = SettingsView::Picker;
            }
        });

        ui.add_space(16.0);
        section(ui, "Built in", pal);
        card(ui, pal, |ui| {
            for (idx, p) in providers::catalog::built_ins().enumerate() {
                if idx > 0 {
                    hairline(ui, pal);
                }
                let mut on = !self.cfg_next.disabled.iter().any(|d| d == p.id);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    provider_mark(ui, p.id, &self.logos, pal, on);
                    ui.add_space(9.0);
                    let (r, _) = ui.allocate_exact_size(
                        Vec2::new((ui.available_width() - 52.0).max(1.0), 34.0),
                        Sense::hover(),
                    );
                    let col = if on { pal.text } else { pal.faint };
                    ui.painter().text(
                        egui::pos2(r.left(), r.top() + 3.0),
                        egui::Align2::LEFT_TOP,
                        p.name,
                        theme::medium(12.5),
                        col,
                    );
                    // Say which file the adapter borrowed. Transparency about
                    // whose credentials are being read is the whole trust story.
                    let source = match p.source {
                        providers::catalog::Source::Cli(path) => path,
                        _ => "",
                    };
                    let g = w::elide(ui, source, theme::sans(10.0), pal.faint, r.width());
                    ui.painter().galley(egui::pos2(r.left(), r.top() + 18.0), g, pal.faint);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if w::switch(ui, &mut on, pal).changed() {
                            if on {
                                self.cfg_next.disabled.retain(|d| d != p.id);
                            } else {
                                self.cfg_next.disabled.push(p.id.to_string());
                            }
                            dirty = true;
                        }
                    });
                });
                ui.add_space(4.0);
            }
        });
        ui.add_space(8.0);
        dirty
    }

    /// One line describing how a configured provider is set up, shown under
    /// its name so the list says what is wired without opening anything.
    fn provider_summary(&self, i: usize) -> String {
        let p = &self.cfg_next.provider[i];
        let where_ = if p.billing {
            p.base_url.clone().unwrap_or_else(|| "no base URL yet".into())
        } else if let Some(u) = &p.url {
            u.clone()
        } else if let Some(b) = &p.base_url {
            b.clone()
        } else {
            "not pointed anywhere yet".into()
        };
        let key = match (&p.api_key, &p.key_env) {
            (Some(k), _) if !k.trim().is_empty() => " · key set".to_string(),
            (_, Some(e)) if !e.trim().is_empty() => format!(" · ${e}"),
            _ => " · no key".to_string(),
        };
        format!("{}{}", where_.trim_start_matches("https://"), key)
    }

    /// The picker: everything the catalogue knows how to track.
    fn settings_picker(&mut self, ui: &mut egui::Ui, pal: &Palette) -> bool {
        use ui::widgets as w;
        ui.horizontal(|ui| {
            if w::pill_button(ui, "‹  Back", false, true, pal).clicked() {
                self.settings_view = SettingsView::List;
            }
        });
        ui.add_space(12.0);
        section(ui, "Add a provider", pal);
        let mut chosen: Option<&'static providers::catalog::Preset> = None;
        card(ui, pal, |ui| {
            let list = providers::catalog::addable(&self.cfg_next.provider);
            for (idx, p) in list.iter().enumerate() {
                if idx > 0 {
                    hairline(ui, pal);
                }
                ui.add_space(5.0);
                let row_h = 40.0;
                ui.horizontal(|ui| {
                    provider_mark(ui, p.id, &self.logos, pal, true);
                    ui.add_space(9.0);
                    let (r, _) = ui.allocate_exact_size(
                        Vec2::new(avail(ui), row_h),
                        Sense::click(),
                    );
                    if ui.interact(r, ui.id().with(("pick", p.id)), Sense::click()).clicked() {
                        chosen = Some(p);
                    }
                    let tag = match p.fidelity {
                        types::Fidelity::Official => ("official", pal.ok),
                        types::Fidelity::Derived => ("derived", pal.warn),
                        types::Fidelity::Manual => ("manual", pal.muted),
                    };
                    let g = ui.painter().layout_job(theme::caps_job(tag.0, 8.5, tag.1));
                    let tag_w = g.rect.width();
                    ui.painter().galley(egui::pos2(r.right() - tag_w, r.top() + 3.0), g, tag.1);
                    let name = w::elide(
                        ui,
                        p.name,
                        theme::medium(12.5),
                        pal.text,
                        (r.width() - tag_w - 10.0).max(20.0),
                    );
                    ui.painter().galley(egui::pos2(r.left(), r.top() + 2.0), name, pal.text);
                    ui.painter().galley(
                        egui::pos2(r.left(), r.top() + 17.0),
                        ui.painter().layout(p.blurb.to_owned(), theme::sans(10.0), pal.faint, r.width()),
                        pal.faint,
                    );
                });
                ui.add_space(5.0);
            }
        });
        ui.add_space(8.0);
        if let Some(p) = chosen {
            // "gateway" and "custom" are templates, so they can be added more
            // than once; give each a free id.
            let mut cfg = p.to_config();
            if self.cfg_next.provider.iter().any(|e| e.id == cfg.id) {
                let mut n = 2;
                while self.cfg_next.provider.iter().any(|e| e.id == format!("{}-{n}", p.id)) {
                    n += 1;
                }
                cfg.id = format!("{}-{n}", p.id);
            }
            self.cfg_next.provider.push(cfg);
            self.settings_view = SettingsView::Editor(self.cfg_next.provider.len() - 1);
            self.probe = Probe::Idle;
            return true;
        }
        false
    }

    /// The editor for one provider. Which fields appear follows the entry's
    /// source: a billing gateway needs a base URL, a JSON endpoint needs paths.
    fn settings_editor(&mut self, ui: &mut egui::Ui, pal: &Palette, i: usize) -> bool {
        use ui::widgets as w;
        let mut dirty = false;
        let preset_id = self.cfg_next.provider[i].id.split('-').next().unwrap_or("").to_string();
        let preset = providers::catalog::preset(&preset_id);
        let source = preset.map(|p| p.source).unwrap_or(providers::catalog::Source::Json);
        let is_billing = self.cfg_next.provider[i].billing;

        ui.horizontal(|ui| {
            if w::pill_button(ui, "‹  Back", false, true, pal).clicked() {
                self.settings_view = SettingsView::List;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let busy = matches!(self.probe, Probe::Running);
                if w::pill_button(ui, if busy { "Testing…" } else { "Test" }, false, !busy, pal)
                    .clicked()
                {
                    self.start_probe(i);
                }
            });
        });
        ui.add_space(12.0);

        section(ui, "Identity", pal);
        card(ui, pal, |ui| {
            ui.add_space(6.0);
            let mut name = self.cfg_next.provider[i].name.clone();
            if w::field(ui, "Display name", "MiniMax", &mut name, pal) {
                self.cfg_next.provider[i].name = name;
                dirty = true;
            }
            let mut id = self.cfg_next.provider[i].id.clone();
            if w::field(ui, "Id", "lowercase, no spaces", &mut id, pal) {
                self.cfg_next.provider[i].id = id.trim().to_lowercase().replace(' ', "-");
                dirty = true;
            }
        });

        ui.add_space(16.0);
        section(ui, "Endpoint", pal);
        card(ui, pal, |ui| {
            ui.add_space(6.0);
            if is_billing || source == providers::catalog::Source::Keyed {
                let mut base = self.cfg_next.provider[i].base_url.clone().unwrap_or_default();
                if w::field(ui, "Base URL", "https://gateway.example.com/v1", &mut base, pal) {
                    self.cfg_next.provider[i].base_url = (!base.trim().is_empty()).then_some(base);
                    dirty = true;
                }
            } else {
                let mut url = self.cfg_next.provider[i].url.clone().unwrap_or_default();
                if w::field(ui, "Usage URL", "https://api.example.com/v1/usage", &mut url, pal) {
                    self.cfg_next.provider[i].url = (!url.trim().is_empty()).then_some(url);
                    dirty = true;
                }
                let mut auth = self.cfg_next.provider[i].auth_header.clone().unwrap_or_default();
                if w::field(ui, "Auth header", "Authorization: Bearer {key}", &mut auth, pal) {
                    self.cfg_next.provider[i].auth_header = (!auth.trim().is_empty()).then_some(auth);
                    dirty = true;
                }
            }
        });

        ui.add_space(16.0);
        section(ui, "Key", pal);
        card(ui, pal, |ui| {
            ui.add_space(6.0);
            let mut key = self.cfg_next.provider[i].api_key.clone().unwrap_or_default();
            let mut shown = self.key_revealed;
            if w::secret_field(ui, "API key", "paste it here", &mut key, &mut shown, pal) {
                self.cfg_next.provider[i].api_key = (!key.trim().is_empty()).then_some(key);
                dirty = true;
            }
            self.key_revealed = shown;
            let mut env = self.cfg_next.provider[i].key_env.clone().unwrap_or_default();
            if w::field(ui, "…or read from env var", "MINIMAX_API_KEY", &mut env, pal) {
                self.cfg_next.provider[i].key_env = (!env.trim().is_empty()).then_some(env);
                dirty = true;
            }
            let note = match &self.cfg_next.provider[i].key_hint {
                Some(h) if !h.is_empty() => format!(
                    "Get it from {h}. It is written to config.toml, which stays user-only (0600)."
                ),
                _ => "The key is written to config.toml, which stays user-only (0600).".to_string(),
            };
            let (r, _) = ui.allocate_exact_size(Vec2::new(avail(ui), 30.0), Sense::hover());
            ui.painter().galley(
                egui::pos2(r.left() + 2.0, r.top()),
                ui.painter().layout(note, theme::sans(10.0), pal.faint, r.width() - 4.0),
                pal.faint,
            );
        });

        if !is_billing && source != providers::catalog::Source::Keyed {
            ui.add_space(16.0);
            dirty |= self.settings_windows(ui, pal, i);
        }

        // ---- test result ---------------------------------------------------
        ui.add_space(16.0);
        if let Probe::Done { ok, ref text } = self.probe {
            let col = if ok { pal.ok } else { pal.bad };
            section(ui, if ok { "It works" } else { "No reading" }, pal);
            card(ui, pal, |ui| {
                ui.add_space(6.0);
                let (r, _) = ui.allocate_exact_size(Vec2::new(avail(ui), 34.0), Sense::hover());
                ui.painter().galley(
                    egui::pos2(r.left(), r.top()),
                    ui.painter().layout(text.clone(), theme::sans(11.0), col, r.width()),
                    col,
                );
                ui.add_space(6.0);
            });
        }
        ui.add_space(8.0);
        dirty
    }

    /// The window mapping editor: which fields of the response hold the
    /// numbers. Each window picks between a ready-made percentage and a
    /// count-of-total, which is the only choice that changes what to fill in.
    fn settings_windows(&mut self, ui: &mut egui::Ui, pal: &Palette, i: usize) -> bool {
        use ui::widgets as w;
        let mut dirty = false;
        section(ui, "Quota windows", pal);
        card(ui, pal, |ui| {
            let n = self.cfg_next.provider[i].windows.len();
            let mut remove: Option<usize> = None;
            for j in 0..n {
                if j > 0 {
                    hairline(ui, pal);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(
                        Vec2::new((ui.available_width() - 28.0).max(1.0), 16.0),
                        Sense::hover(),
                    );
                    let col = theme::mix(pal.faint, pal.muted, 0.55);
                    let g = ui
                        .painter()
                        .layout_job(theme::caps_job(&format!("window {}", j + 1), 9.0, col));
                    ui.painter().galley(egui::pos2(r.left() + 2.0, r.top()), g, col);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if w::glyph_button(ui, w::Mark::Cross, true, "remove window", pal).clicked() {
                            remove = Some(j);
                        }
                    });
                });
                ui.add_space(6.0);
                let mut label = self.cfg_next.provider[i].windows[j].label.clone();
                if w::field(ui, "Label", "5h · weekly · credits", &mut label, pal) {
                    self.cfg_next.provider[i].windows[j].label = label;
                    dirty = true;
                }
                // Percentage or count-of-total. Anything else the mapping
                // supports is a variation on one of these two.
                let counted = self.cfg_next.provider[i].windows[j].remaining_count_path.is_some()
                    || self.cfg_next.provider[i].windows[j].total_const.is_some();
                let pick = w::segmented(ui, &["Percentage", "N of total"], usize::from(counted), pal);
                if pick != usize::from(counted) {
                    let win = &mut self.cfg_next.provider[i].windows[j];
                    if pick == 0 {
                        win.remaining_count_path = None;
                        win.total_count_path = None;
                        win.total_const = None;
                    } else {
                        win.remaining_path = None;
                    }
                    dirty = true;
                }
                ui.add_space(10.0);
                if pick == 0 {
                    let mut p = self.cfg_next.provider[i].windows[j].remaining_path.clone().unwrap_or_default();
                    if w::field(ui, "Percent remaining", "usage.percent_left", &mut p, pal) {
                        self.cfg_next.provider[i].windows[j].remaining_path =
                            (!p.trim().is_empty()).then_some(p);
                        dirty = true;
                    }
                } else {
                    let mut r = self.cfg_next.provider[i].windows[j]
                        .remaining_count_path
                        .clone()
                        .unwrap_or_default();
                    if w::field(ui, "Remaining", "data.limit_remaining", &mut r, pal) {
                        self.cfg_next.provider[i].windows[j].remaining_count_path =
                            (!r.trim().is_empty()).then_some(r);
                        dirty = true;
                    }
                    // One field for the ceiling: a number is a ceiling you
                    // know, anything else is a path to one the API reports.
                    let win = &self.cfg_next.provider[i].windows[j];
                    let mut total = win
                        .total_const
                        .map(|v| format!("{v}"))
                        .or_else(|| win.total_count_path.clone())
                        .unwrap_or_default();
                    if w::field(ui, "Total (a path, or a number you know)", "data.limit  ·  50", &mut total, pal) {
                        let win = &mut self.cfg_next.provider[i].windows[j];
                        let t = total.trim();
                        match (t.is_empty(), t.parse::<f64>()) {
                            (true, _) => {
                                win.total_const = None;
                                win.total_count_path = None;
                            }
                            (_, Ok(v)) => {
                                win.total_const = Some(v);
                                win.total_count_path = None;
                            }
                            _ => {
                                win.total_const = None;
                                win.total_count_path = Some(total.clone());
                            }
                        }
                        dirty = true;
                    }
                }
                let mut rs = self.cfg_next.provider[i].windows[j].resets_at_path.clone().unwrap_or_default();
                if w::field(ui, "Resets at (optional)", "usage.reset_at", &mut rs, pal) {
                    self.cfg_next.provider[i].windows[j].resets_at_path =
                        (!rs.trim().is_empty()).then_some(rs);
                    dirty = true;
                }
            }
            if n == 0 {
                let (r, _) = ui.allocate_exact_size(Vec2::new(avail(ui), 30.0), Sense::hover());
                ui.painter().galley(
                    egui::pos2(r.left(), r.top()),
                    ui.painter().layout(
                        "Add one window per quota the endpoint reports.".to_owned(),
                        theme::sans(11.0),
                        pal.faint,
                        r.width(),
                    ),
                    pal.faint,
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if w::pill_button(ui, "Add window", false, true, pal).clicked() {
                    self.cfg_next.provider[i].windows.push(config::WindowConfig {
                        label: "quota".into(),
                        ..Default::default()
                    });
                    dirty = true;
                }
            });
            ui.add_space(4.0);
            if let Some(j) = remove {
                self.cfg_next.provider[i].windows.remove(j);
                dirty = true;
            }
        });
        dirty
    }

    /// Take one live reading in the background so the sheet can say whether a
    /// half-filled provider actually works, before anything is saved.
    fn start_probe(&mut self, i: usize) {
        let cfg = self.cfg_next.provider[i].clone();
        let (tx, rx) = mpsc::channel();
        self.probe_rx = Some(rx);
        self.probe = Probe::Running;
        thread::spawn(move || {
            let snap = providers::probe(&cfg);
            let msg = match &snap.reading {
                Reading::Ok { windows, .. } => {
                    let parts: Vec<String> = windows
                        .iter()
                        .map(|w| match w.remaining_percent {
                            Some(p) => format!("{} {:.0}% left", w.label, p),
                            None => w.label.clone(),
                        })
                        .collect();
                    (true, format!("Read {} window(s): {}", windows.len(), parts.join(" · ")))
                }
                Reading::NeedsAuth(m) => (false, format!("The key was not accepted — {m}")),
                Reading::Error(m) => (false, format!("No reading — {m}")),
                Reading::NotConfigured => {
                    (false, "Still missing something — check the URL and key above.".to_string())
                }
            };
            let _ = tx.send(msg);
        });
    }

    /// Appearance: theme, and how much of the notch is on show.
    fn settings_appearance(&mut self, ui: &mut egui::Ui, pal: &Palette) -> bool {
        use ui::widgets as w;
        let mut dirty = false;
        section(ui, "Theme", pal);
        card(ui, pal, |ui| {
            ui.add_space(2.0);
            // Swatches, not a dropdown: a theme is a set of colors, so it
            // should be chosen by looking at the colors.
            let current = if self.cfg_next.theme.is_empty() { "midnight" } else { &self.cfg_next.theme };
            let current = current.to_string();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
                for t in theme::THEMES {
                    if w::theme_swatch(ui, t, *t == current, pal).clicked() && *t != current {
                        self.cfg_next.theme = (*t).to_string();
                        dirty = true;
                    }
                }
            });
            ui.add_space(4.0);
        });

        ui.add_space(16.0);
        section(ui, "Surfaces", pal);
        card(ui, pal, |ui| {
            setting_row(
                ui,
                "Notch opacity",
                "How much of the desktop shows through the notch body.",
                pal,
                |ui| {
                    let mut v = (self.cfg_next.notch_opacity * 100.0).round() as i64;
                    if w::slider(ui, &mut v, 15..=100, 5, "%", pal) {
                        self.cfg_next.notch_opacity = v as f32 / 100.0;
                        dirty = true;
                    }
                },
            );
            hairline(ui, pal);
            setting_row(
                ui,
                "Card opacity",
                "Same, for the usage card that opens beside a hovered gauge.",
                pal,
                |ui| {
                    let mut v = (self.cfg_next.card_opacity * 100.0).round() as i64;
                    if w::slider(ui, &mut v, 15..=100, 5, "%", pal) {
                        self.cfg_next.card_opacity = v as f32 / 100.0;
                        dirty = true;
                    }
                },
            );
        });

        ui.add_space(16.0);
        section(ui, "Floating pill", pal);
        card(ui, pal, |ui| {
            setting_row(
                ui,
                "Providers when collapsed",
                "The rest fold into a +N chip. Only affects the undocked pill.",
                pal,
                |ui| {
                    let mut v = self.cfg_next.max_visible_collapsed as i64;
                    if w::slider(ui, &mut v, 1..=8, 1, "", pal) {
                        self.cfg_next.max_visible_collapsed = v.max(1) as usize;
                        dirty = true;
                    }
                },
            );
        });
        ui.add_space(8.0);
        dirty
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

/// Rail-card fade rates (per-second exponential): quick in, snappier out so
/// the card feels strictly hover-bound.
fn t_rail_spring(dt: f32, opening: bool) -> f32 {
    1.0 - (-(if opening { 12.0 } else { 18.0 }) * dt).exp()
}

/// Vertical center of the cell for the provider at row `i`.
fn rail_row_cy(i: usize, body_top: f32, row_h: f32) -> f32 {
    body_top + RAIL_PAD_TOP + (i as f32 + 0.5) * row_h + i as f32 * RAIL_ROW_GAP
}

/// Minimal panel layout for provider `id`, placed beside the notch with no
/// tail. This is the single source of truth for painting and hover hit-testing.
fn rail_card_layout(
    id: &str,
    snaps: &[Snapshot],
    ui_rect: Rect,
    on_left: bool,
    body_top: f32,
    row_h: f32,
) -> Option<ui::RailCardLayout> {
    let s = snaps.iter().find(|s| s.provider_id == id)?;
    let anchor_cy = snaps.iter().position(|p| p.provider_id == id)
        .map(|i| rail_row_cy(i, body_top, row_h))
        .unwrap_or_else(|| ui_rect.center().y);
    let mut layout = ui::rail_card_layout(s, anchor_cy, ui_rect.height(), RAIL_CARD_W, 8.0);
    let x0 = if on_left { RAIL_STRIP_W + RAIL_COL_GAP } else { 0.0 };
    layout.rect = layout.rect.translate(egui::vec2(x0, 0.0));
    Some(layout)
}

/// Debug hook: `LIMITCUE_UI_SHOT=/path.png` captures the window after the
/// layout has settled (~1.5 s) and exits. Unlike eframe's `__screenshot`
/// feature this waits for the notch to size itself and reads the frame
/// before the buffer swap, so transparent windows capture correctly.
fn debug_shot(ctx: &egui::Context, started: Instant, requested: &mut bool) {
    let Ok(path) = std::env::var("LIMITCUE_UI_SHOT") else { return };
    if path.is_empty() {
        return;
    }
    let shot = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let Some(img) = shot {
        let file = std::fs::File::create(&path).expect("screenshot path");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.width() as u32, img.height() as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().expect("png header");
        let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        w.write_image_data(&bytes).expect("png data");
        drop(w);
        std::process::exit(0);
    }
    if !*requested && started.elapsed().as_secs_f32() > 1.5 {
        *requested = true;
        ctx.send_viewport_cmd(ViewportCommand::Screenshot);
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(50));
}

impl eframe::App for App {
    /// Fully transparent clear. (eframe's default is `rgba(12,12,12,180)` —
    /// a 70%-opaque grey that fills whatever the UI doesn't paint, which
    /// shows up as a flat dark slab behind the usage card.)
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        debug_shot(ctx, self.started, &mut self.shot_requested);
        self.drain(ctx);
        self.tweens.retain(|_, t| t.value().is_some());
        let stored_edge = self.dock.lock().unwrap().edge;
        let edge = if self.notch_mode && stored_edge == Edge::Free { Edge::Left } else { stored_edge };
        if edge != self.last_edge {
            self.last_edge = edge;
            ctx.request_repaint();
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

        // Expanded height follows content. Only clamp to the available
        // screen height so a long provider list scrolls instead of clipping.
        let ideal_h = HEADER_H + 42.0
            + snaps.iter().map(|s| {
                let rows_h = match &s.reading {
                    Reading::Ok { windows, .. } => windows.len().max(1) as f32 * 24.0,
                    _ => 56.0,
                };
                38.0 + rows_h
            }).sum::<f32>();
        let viewport_h = ctx.input(|i| i.viewport().inner_rect.map(|r| r.height()))
            .unwrap_or(720.0);
        let expanded_size = Vec2::new(EXPANDED_W, (ideal_h + 10.0).min((viewport_h - VIEWPORT_H_MARGIN).max(220.0)));
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
        let rail_row_h = if self.cfg.show_rail_percent { RAIL_ROW_H } else { RAIL_ROW_H_COMPACT };
        let rail_h = RAIL_PAD_TOP + rail_rows * rail_row_h + (rail_rows - 1.0) * RAIL_ROW_GAP + RAIL_ORB + 10.0;
        let card_want = self.rail_open.is_some();
        let card_f_target = if card_want { 1.0 } else { 0.0 };
        let previous_card_f = self.rail_card_f;
        self.rail_card_f += (card_f_target - self.rail_card_f) * (t_rail_spring(dt, card_want));
        if self.rail_card_f < 0.01 {
            self.rail_card_f = 0.0;
            self.rail_last = None;
        }
        if (self.rail_card_f - previous_card_f).abs() > 0.001 {
            ctx.request_repaint();
        }
        if card_want {
            self.rail_card_f = self.rail_card_f.max(0.02);
        }
        let monitor_h = ctx.input(|i| i.viewport().monitor_size.map(|s| s.y)).unwrap_or(900.0);
        // ---- the notch's vertical band -----------------------------------
        //
        // The window's top edge is pinned once mapped, so a card can never be
        // painted above it. Rather than fight that, the window is made a fixed
        // band that *contains* the notch: tall enough for the widest card any
        // provider can show, positioned so the notch still lands where the
        // user put it, with the notch painted `headroom` down from the top.
        // The space above the notch is then the card's to open into.
        //
        // The band's height does not change on hover — only its width does —
        // so opening a card can no longer disturb the notch's position.
        if self.notch_mode || matches!(edge, Edge::Left | Edge::Right) {
            // The script reports the *window's* top; the notch is `headroom`
            // below it. Debug hook: LIMITCUE_UI_TOP parks the notch at a given
            // screen y without dragging the real window.
            let reported = std::env::var("LIMITCUE_UI_TOP")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or_else(|| self.dock.lock().unwrap().y as f32);
            if !self.reported_y.is_finite() || (reported - self.reported_y).abs() > 0.5 {
                self.reported_y = reported;
                self.notch_y = (reported + self.headroom).clamp(0.0, monitor_h);
            }
        }
        let card_max_h = snaps
            .iter()
            .map(ui::rail_card_height)
            .fold(0.0_f32, f32::max);
        let band_h = rail_h.max(card_max_h + ui::RAIL_CARD_GAP_Y * 2.0);
        let window_y = (self.notch_y).min(monitor_h - band_h).max(0.0);
        self.headroom = (self.notch_y - window_y).max(0.0);
        let rail_size = match self.rail_open.is_some() || self.rail_last.is_some() {
            true => Vec2::new(RAIL_STRIP_W + RAIL_COL_GAP + RAIL_CARD_W, band_h),
            false => Vec2::new(RAIL_STRIP_W, band_h),
        };

        // Hand the band to the compositor: once the window has mapped (KWin
        // cannot see it before that, hence the early retries), and thereafter
        // only when the band or the notch's position actually changes.
        if rail {
            if self.restore_sent < 30 {
                self.restore_sent += 1;
            }
            let want = (window_y.round() as i32, band_h.round() as i32);
            let settling = self.restore_sent < 30 && self.restore_sent.is_multiple_of(6);
            // Screenshot runs must not touch the compositor: the script picks
            // the first limitcue window it finds, which would be whatever
            // instance the user already has open.
            let capturing = std::env::var_os("LIMITCUE_UI_SHOT").is_some();
            // Never re-place while a card is open: the notch would hold still
            // (headroom absorbs the move) but the card under the pointer would
            // not, and losing the hover mid-read is worse than a stale band.
            let busy = self.rail_open.is_some() || self.rail_card_f > 0.0;
            if !capturing && !busy && (settling || self.placed != Some(want)) {
                self.placed = Some(want);
                dock::request_geometry(want.0, want.1);
            }
        }

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
        let prev = self.cur_size;
        let mut animating = false;
        if rail {
            // Do not spring the native rail viewport while hovering. A
            // compositor-docked transparent window moves when its width or
            // height changes; animating that geometry makes the pointer cross
            // the moving hover rect and causes a visible vibration. Animate
            // only the card paint below, keeping the notch stable.
            if (prev - target).length() > 0.08 || snap {
                self.cur_size = target;
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(target));
            }
        } else {
            if snap {
                self.cur_size = target;
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.cur_size));
            }
            let t = 1.0 - (-18.0 * dt).exp();
            self.cur_size = Vec2::new(prev.x + (target.x - prev.x) * t, prev.y + (target.y - prev.y) * t);
            animating = (self.cur_size - prev).length() > 0.08 && !snap;
            if animating {
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.cur_size));
            }
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

        // Settings follows its content, with only a viewport safety cap.
        if self.settings_open {
            // Theme changes preview live — picking a swatch that only takes
            // effect after Save makes the picker feel broken.
            let pal = theme::palette(&self.cfg_next.theme);
            let body = self.settings_body_h.max(140.0);
            let chrome = SETTINGS_PAD * 2.0 + 24.0 + 12.0 + 28.0 + 14.0 + SETTINGS_ACTIONS_H;
            // Cap against the *monitor*, not the window: capping against its
            // own height pinned the sheet at the floor it started from and it
            // could never grow to fit its content.
            let screen_h = ctx
                .input(|i| i.viewport().monitor_size.map(|s| s.y))
                .unwrap_or(900.0);
            let settings_h = (body + chrome + 10.0).min((screen_h - VIEWPORT_H_MARGIN).max(380.0));
            self.cur_size = Vec2::new(SETTINGS_W, settings_h);
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(self.cur_size));
            let frame = egui::Frame::none()
                .fill(pal.bg)
                .stroke(egui::Stroke::new(1.0_f32, pal.border))
                .rounding(egui::Rounding::same(17.0))
                .inner_margin(egui::Margin::same(SETTINGS_PAD));
            egui::CentralPanel::default().frame(frame).show(ctx, |ui| {
                self.settings_ui(ui, &pal);
            });
            return;
        }

        // Render header + detail; for bottom dock, reverse order so the pill
        // row ends up at the bottom (detail grows upward, away from the edge).
        let header = |ui: &mut egui::Ui, this: &mut App| {
            if rail {
                this.render_rail(ui, ctx, &snaps, rail_h, rail_row_h);
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
                    let chip_alpha = if self.cfg.quiet_mode && !resp.hovered() { 0.58 } else { 1.0 };
                    ui::draw_chip(ui, rect, s, pct, &pal, chip_alpha, stale, resp.hovered(), &self.logos);
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
    fn render_rail(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, snaps: &[Snapshot], rail_h: f32, rail_row_h: f32) {
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

        // ---- the notch body ----------------------------------------------
        // The strip hugs the docked edge: window-left for a left dock,
        // window-right for a right dock (the card then fills the remainder).
        let body_x = if on_left { ui_rect.left() } else { ui_rect.right() - RAIL_STRIP_W };
        // The host is a band taller than the notch, positioned so that painting
        // the notch `headroom` below the window's top lands it exactly where
        // the user parked it. Everything above that offset is room for a card
        // to open upward into.
        let body = Rect::from_min_size(
            egui::pos2(body_x, ui_rect.top() + self.headroom),
            Vec2::new(RAIL_STRIP_W, rail_h),
        );
        let r = RAIL_CORNER.min(body.height() / 2.0);
        // body with two square edge-corners and two 15px rounded corners
        let body_rounding = if on_left {
            egui::Rounding { nw: 0.0, sw: 0.0, ne: r, se: r }
        } else {
            egui::Rounding { nw: r, sw: r, ne: 0.0, se: 0.0 }
        };
        // Surface opacity is the user's setting; quiet mode is a separate
        // factor on top of it, so setting the notch to 100 % actually yields
        // an opaque notch rather than the old baked-in 0.92.
        let quiet = if self.cfg.quiet_mode && !pointer.map(|p| body.contains(p)).unwrap_or(false) {
            0.78
        } else {
            1.0
        };
        let body_fill = ui::theme::at_opacity(pal.rail_bg, self.cfg.notch_opacity);
        ui.painter().rect_filled(body, body_rounding, body_fill.linear_multiply(quiet));

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
        let pointer_row = pointer.and_then(|pt| {
            if pt.x < body.left() || pt.x > body.right() {
                return None;
            }
            let y = pt.y - body.top() - RAIL_PAD_TOP;
            if y < 0.0 {
                return None;
            }
            let stride = rail_row_h + RAIL_ROW_GAP;
            let index = (y / stride).floor() as usize;
            let within = y - index as f32 * stride;
            (index < snaps.len() && within <= rail_row_h).then_some(index)
        });
        let any_row_hovered = pointer_row.is_some();
        let focused_id = pointer_row.map(|i| snaps[i].provider_id.as_str());
        let dt_focus = ctx.input(|i| i.stable_dt).clamp(0.001, 0.1);
        let mut focus_animating = false;
        for s in snaps {
            let target = if Some(s.provider_id.as_str()) == focused_id { 1.0 } else { 0.0 };
            let weight = self.rail_focus_weights.entry(s.provider_id.clone()).or_insert(0.0);
            let step = 1.0 - (-dt_focus / 0.32).exp();
            *weight += (target - *weight) * step;
            if (*weight - target).abs() > 0.005 {
                focus_animating = true;
            }
        }
        self.rail_focus_weights.retain(|id, weight| {
            snaps.iter().any(|s| s.provider_id == *id) || *weight > 0.005
        });
        if focus_animating {
            ctx.request_repaint();
        }
        let dim_target = if any_row_hovered { 1.0 } else { 0.0 };
        let dim_step = 1.0 - (-dt_focus / 0.48).exp();
        self.rail_dim_t += (dim_target - self.rail_dim_t) * dim_step;
        if (self.rail_dim_t - dim_target).abs() > 0.005 {
            ctx.request_repaint();
        }
        let dim_ease = self.rail_dim_t * self.rail_dim_t * (3.0 - 2.0 * self.rail_dim_t);
        let ambient_alpha = 1.0 - 0.68 * dim_ease;
        let mut next_y = body.top() + RAIL_PAD_TOP;
        for s in snaps.iter() {
            let row_rect = Rect::from_min_size(egui::pos2(body.left(), next_y), Vec2::new(cell_w, rail_row_h));
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
            }
            // A hovered gauge is the focus: each provider owns an independent
            // weight, so rapid A -> B -> C switches never discard an in-flight
            // fade from the previous provider.
            let focus_mix = self.rail_focus_weights.get(&s.provider_id).copied().unwrap_or(0.0);
            let gauge_alpha = if any_row_hovered {
                ambient_alpha + (1.0 - ambient_alpha) * focus_mix
            } else {
                1.0
            };
            let gauge_stroke = RAIL_RING_STROKE + 0.7 * focus_mix;
            // gauge: track ring + heat arc (share used) around the bare mark
            let used01 = pct.map(|v| 1.0 - (v / 100.0) as f32).unwrap_or(0.0);
            let heat = if ok { ui::theme::heat(used01, &pal) } else { ring_col };
            let heat = if stale { ui::theme::mix(heat, pal.stale, 0.6) } else { heat };
            let ring_center = egui::pos2(row_rect.center().x, row_rect.center().y - 9.0);
            ui::widgets::gauge(
                ui,
                ring_center,
                RAIL_RING_R,
                gauge_stroke,
                self.logos.get(&s.provider_id),
                &ui::theme::monogram(&s.provider_id),
                if ok { used01 } else { 0.0 },
                heat,
                pal.rail_disc,
                gauge_alpha,
            );
            // A provider in trouble is marked on the gauge itself, so the
            // notch still says so when the labels are off — that used to be
            // the one thing the compact cell could not express, and it fell
            // back to printing "err" under a gauge that had no percentage
            // above it.
            let status = match &s.reading {
                Reading::Ok { .. } => None,
                Reading::NeedsAuth(_) => Some(pal.warn),
                Reading::Error(_) => Some(pal.bad),
                Reading::NotConfigured => Some(pal.muted),
            };
            if let Some(status) = status {
                let (sin, cos) = RAIL_BADGE_ANGLE.sin_cos();
                ui::widgets::alert_badge(
                    ui,
                    egui::pos2(
                        ring_center.x + RAIL_RING_R * cos,
                        ring_center.y - RAIL_RING_R * sin,
                    ),
                    RAIL_BADGE_R,
                    status,
                    pal.bg,
                    gauge_alpha,
                );
            }
            // The label line is percentages-only now: with them switched off
            // the badge carries the status on its own.
            if self.cfg.show_rail_percent {
                let (label, label_col) = match (&s.reading, pct) {
                    (Reading::Ok { .. }, Some(v)) => (format!("{:.0}%", 100.0 - v), Color32::WHITE),
                    (Reading::Ok { .. }, None) => ("…".into(), Color32::WHITE),
                    (Reading::NeedsAuth(_), _) => ("auth".into(), pal.warn),
                    (Reading::Error(_), _) => ("err".into(), pal.bad),
                    _ => ("not set".into(), pal.muted),
                };
                ui.painter().text(
                    egui::pos2(row_rect.center().x, row_rect.bottom() - 11.0),
                    egui::Align2::CENTER_CENTER,
                    label,
                    ui::theme::medium(12.0),
                    label_col,
                );
            }
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
            ui::theme::at_opacity(pal.rail_deep, self.cfg.notch_opacity),
        );
        // Neutral three-dot menu mark: this control is navigation/settings,
        // not another quota gauge. The dots brighten together on hover.
        let dot_color = if orb_hover { Color32::WHITE } else { pal.muted };
        let dot_radius = if orb_hover { 2.0 } else { 1.7 };
        for offset in [-6.0_f32, 0.0, 6.0] {
            ui.painter().circle_filled(
                egui::pos2(orb_rect.center().x + offset, orb_rect.center().y),
                dot_radius,
                dot_color,
            );
        }
        if orb.clicked() {
            self.settings_open = true;
        }
        if orb.drag_started_by(egui::PointerButton::Primary)
            || orb.drag_started_by(egui::PointerButton::Middle)
        {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        let _ = orb;

        // ---- hover state machine ------------------------------------------
        // The card is a real hover target, not just painted pixels. This is
        // important for a transparent, resizable viewport: raw pointer
        // coordinates can survive a resize for a frame and make a card look
        // stuck even after the pointer has left it.
        let card_rect_now = self
            .rail_open
            .as_deref()
            .and_then(|id| rail_card_layout(id, snaps, ui_rect, on_left, body.top(), rail_row_h).map(|l| l.rect));
        let card_hovered = card_rect_now.is_some_and(|rect| {
            ui.interact(rect, ui.id().with(("rail-card", self.rail_open.as_deref())), Sense::hover())
                .hovered()
        });
        // Screenshot hook: pin only when there is no pointer at all. Normal
        // desktop interaction never uses this path.
        let seeded = std::env::var("LIMITCUE_UI_RAIL").ok().filter(|v| !v.is_empty());
        // Pin while there is no pointer at all, and unconditionally when a
        // screenshot is being captured (the capture window is long enough for
        // a stray pointer to drift over the notch and dismiss the card).
        let pinned = self.rail_open.is_some()
            && seeded.as_deref() == self.rail_open.as_deref()
            && (pointer.is_none() || std::env::var_os("LIMITCUE_UI_SHOT").is_some());
        // Strict hover-only behavior: a row or the visible card itself must
        // be hovered. There is no broad transparent keep-zone around it.
        if let Some(id) = hovered_id {
            self.rail_open = Some(id);
        } else if !pinned && !card_hovered {
            if let Some(prev) = self.rail_open.take() {
                self.rail_last = Some(prev);
            }
        }

        // ---- the tail-card popup ------------------------------------------
        if self.rail_card_f > 0.0 {
            let Some(id) = self.rail_open.clone().or_else(|| self.rail_last.clone()) else {
                self.rail_card_f = 0.0;
                return;
            };
            let Some(s) = snaps.iter().find(|s| s.provider_id == id) else {
                self.rail_open = None;
                return;
            };
            let stale = self.is_stale(s, now);
            let a = self.rail_card_f.clamp(0.0, 1.0);
            let Some(card_layout) = rail_card_layout(&id, snaps, ui_rect, on_left, body.top(), rail_row_h) else {
                return;
            };
            // Slide the card out from the notch while it fades in; reverse
            // on exit. `rail_card` owns the whole panel — glass, content and
            // shadow — so the rect it is handed is the rect it fills.
            let slide = (1.0 - a) * 10.0;
            let card_rect = card_layout.rect.translate(Vec2::new(
                if on_left { -slide } else { slide },
                0.0,
            ));
            ui::rail_card(
                ui,
                card_rect,
                s,
                &pal,
                a,
                stale,
                &self.logos,
                now,
                card_layout.list_height,
                self.cfg.card_opacity,
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
        // Resting notch: exactly the spine; height matches the rail's own
        // layout math.
        let rail_h = RAIL_PAD_TOP + 4.0 * RAIL_ROW_H_COMPACT + 3.0 * RAIL_ROW_GAP + RAIL_ORB + 10.0;
        Vec2::new(RAIL_STRIP_W, rail_h)
    } else if start_expanded {
        Vec2::new(EXPANDED_W, 420.0)
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
