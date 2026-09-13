//! Optional system-tray fallback.
//!
//! For compositors that refuse always-on-top, or when a tray is simply
//! preferred, LimitCue can publish a StatusNotifierItem: a ring gauge coloured
//! by the tightest window, a tooltip listing every provider, and a menu to
//! show the notch, refresh, or quit.
//!
//! Off unless `tray = true`. The tray is started on the first frame, once an
//! `egui::Context` exists, so a menu click can wake the UI immediately.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui;
use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, ToolTip};

use crate::types::Snapshot;

/// Actions a tray click hands back to the app.
#[derive(Default)]
pub struct Actions {
    show: AtomicBool,
    refresh: AtomicBool,
    quit: AtomicBool,
}

/// What the app should do with a tray click, if anything.
#[derive(Default)]
pub struct Wanted {
    pub show: bool,
    pub refresh: bool,
    pub quit: bool,
}

struct Indicator {
    ctx: egui::Context,
    actions: Arc<Actions>,
    /// Provider name and its lowest remaining percentage, tightest first.
    lines: Vec<(String, Option<f64>)>,
}

impl Indicator {
    fn set(&mut self, snaps: &[Snapshot]) {
        let mut lines: Vec<(String, Option<f64>)> = snaps
            .iter()
            .map(|s| (s.display_name.clone(), s.min_remaining()))
            .collect();
        lines.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        self.lines = lines;
    }

    /// The colour of the ring: the tightest window's severity.
    fn color(&self) -> (u8, u8, u8) {
        match self.lines.iter().filter_map(|(_, p)| *p).next() {
            Some(p) if p <= 15.0 => (255, 105, 95),
            Some(p) if p <= 50.0 => (255, 174, 69),
            Some(_) => (116, 228, 163),
            None => (139, 149, 150),
        }
    }

    fn headline(&self) -> String {
        match self.lines.iter().filter_map(|(_, p)| *p).next() {
            Some(p) => format!("LimitCue — {p:.0}% left"),
            None => "LimitCue".into(),
        }
    }

    fn detail(&self) -> String {
        if self.lines.is_empty() {
            return "No providers configured".into();
        }
        self.lines
            .iter()
            .map(|(name, pct)| match pct {
                Some(p) => format!("{name}  {p:.0}%"),
                None => format!("{name}  —"),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl ksni::Tray for Indicator {
    fn id(&self) -> String {
        "limitcue".into()
    }

    fn title(&self) -> String {
        "LimitCue".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::ApplicationStatus
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![ring_icon(self.color())]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: String::new(),
            icon_pixmap: vec![ring_icon(self.color())],
            title: self.headline(),
            description: self.detail(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.actions.show.store(true, Ordering::Relaxed);
        self.ctx.request_repaint();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show LimitCue".into(),
                activate: Box::new(|t: &mut Indicator| {
                    t.actions.show.store(true, Ordering::Relaxed);
                    t.ctx.request_repaint();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Refresh now".into(),
                activate: Box::new(|t: &mut Indicator| {
                    t.actions.refresh.store(true, Ordering::Relaxed);
                    t.ctx.request_repaint();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit LimitCue".into(),
                activate: Box::new(|t: &mut Indicator| {
                    t.actions.quit.store(true, Ordering::Relaxed);
                    t.ctx.request_repaint();
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// The running tray, or nothing if there is no StatusNotifierItem host.
pub struct TrayHandle {
    handle: Option<ksni::blocking::Handle<Indicator>>,
    actions: Arc<Actions>,
}

impl TrayHandle {
    /// Publish the current readings to the tooltip and icon.
    pub fn publish(&self, snaps: &[Snapshot]) {
        if let Some(h) = &self.handle {
            let _ = h.update(|t: &mut Indicator| t.set(snaps));
        }
    }

    /// Take any pending action, clearing it.
    pub fn take(&self) -> Wanted {
        Wanted {
            show: self.actions.show.swap(false, Ordering::Relaxed),
            refresh: self.actions.refresh.swap(false, Ordering::Relaxed),
            quit: self.actions.quit.swap(false, Ordering::Relaxed),
        }
    }

    pub fn shutdown(&self) {
        if let Some(h) = &self.handle {
            h.shutdown().wait();
        }
    }
}

/// Start the tray, bound to `ctx` so a click can wake the UI. A desktop with
/// no StatusNotifierItem host is not an error: the handle simply carries no
/// actions.
pub fn start(ctx: egui::Context) -> TrayHandle {
    let actions = Arc::new(Actions::default());
    let indicator = Indicator {
        ctx,
        actions: actions.clone(),
        lines: Vec::new(),
    };
    match indicator.spawn() {
        Ok(handle) => TrayHandle {
            handle: Some(handle),
            actions,
        },
        Err(e) => {
            eprintln!("limitcue: no tray available ({e})");
            TrayHandle {
                handle: None,
                actions,
            }
        }
    }
}

/// A 32 px ring in ARGB32 (network byte order), the colour of the worst
/// remaining window.
fn ring_icon(color: (u8, u8, u8)) -> Icon {
    const SIZE: i32 = 32;
    let (r, g, b) = color;
    let (cx, cy) = (15.5_f32, 15.5_f32);
    let (r_out, r_in) = (14.0_f32, 9.0_f32);
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            let a = if (r_in..=r_out).contains(&d) { 255 } else { 0 };
            data.extend_from_slice(&[a, r, g, b]);
        }
    }
    Icon {
        width: SIZE,
        height: SIZE,
        data,
    }
}
