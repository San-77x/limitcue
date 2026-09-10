//! Dock state persistence + the KWin integration bridge.
//!
//! On Wayland the app can neither move nor even learn its own window
//! position — only the compositor knows it. So the KWin script
//! (misc/kwin/limitcue-integrate) does the moving:
//!
//! - keep-above + edge snapping happen entirely in the script;
//! - when the user finishes a drag, the script reports the result here over
//!   D-Bus (`StorePosition`) and we persist it to `dock.json`;
//! - at startup the app can't ask KWin to move the window directly
//!   (scripts can't receive dynamic arguments), so it writes a one-shot
//!   `apply.js` with the restored geometry baked in and triggers it through
//!   KWin's Scripting D-Bus interface. The script looks for the window and
//!   positions it.
//!
//! `dock.json` is numbers-only (edge + x + y), same privacy posture as
//! `state.json`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

/// Screen edge the pill is docked to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Edge {
    /// Free-floating (restored x/y apply).
    #[default]
    Free,
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    pub fn from_u8(v: u8) -> Edge {
        match v {
            1 => Edge::Top,
            2 => Edge::Bottom,
            3 => Edge::Left,
            4 => Edge::Right,
            _ => Edge::Free,
        }
    }

    /// Deprecated: the notch is side-only now and the restore script derives
    /// the edge itself, so nothing serializes edges anymore.
    #[allow(dead_code)]
    pub fn to_u8(self) -> u8 {
        match self {
            Edge::Free => 0,
            Edge::Top => 1,
            Edge::Bottom => 2,
            Edge::Left => 3,
            Edge::Right => 4,
        }
    }
}

/// The dock position as last reported by the compositor, and the output it
/// landed on.
///
/// The output rect matters for two things the app cannot work out for itself:
/// which side of *that* monitor the notch is against, and how much vertical
/// room the card has on a screen whose origin is not 0,0. A Wayland client is
/// told neither.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DockState {
    pub edge: Edge,
    pub x: i32,
    pub y: i32,
    /// Geometry of the output the window is on. All zero means "not reported
    /// yet", in which case callers fall back to what egui knows.
    #[serde(default)]
    pub out_x: i32,
    #[serde(default)]
    pub out_y: i32,
    #[serde(default)]
    pub out_w: i32,
    #[serde(default)]
    pub out_h: i32,
}

impl DockState {
    /// Which side of its own output the notch is against, worked out from the
    /// position rather than taken on trust from `edge`.
    ///
    /// The stored enum is only as fresh as the last report, and a drag that
    /// does not produce one leaves it contradicting the actual position —
    /// which is how a left-docked notch ended up opening its card leftwards,
    /// off the screen. A position and an output rect cannot disagree.
    pub fn side(&self, window_w: i32) -> Option<Edge> {
        if self.out_w <= 0 {
            return None;
        }
        let from_left = self.x - self.out_x;
        let from_right = (self.out_x + self.out_w) - (self.x + window_w);
        Some(if from_left <= from_right { Edge::Left } else { Edge::Right })
    }

    /// The vertical span of the output, for clamping the notch's band. Falls
    /// back to a screen starting at zero when nothing has been reported.
    pub fn output_span(&self, fallback_h: f32) -> (f32, f32) {
        if self.out_h > 0 {
            (self.out_y as f32, self.out_h as f32)
        } else {
            (0.0, fallback_h)
        }
    }
}

pub type SharedDock = Arc<Mutex<DockState>>;

pub fn dock_path() -> PathBuf {
    dirs::data_dir().unwrap_or_default().join("limitcue").join("dock.json")
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_default().join("limitcue")
}

pub fn load_dock() -> DockState {
    serde_json::from_str(&std::fs::read_to_string(dock_path()).unwrap_or_default()).unwrap_or_default()
}

fn save_dock(st: &DockState) {
    if let Some(dir) = dock_path().parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(j) = serde_json::to_string(st) {
        let _ = std::fs::write(dock_path(), j);
    }
}

/// D-Bus interface served at /io/limitcue/dock (io.limitcue.Dock).
/// The KWin script calls StorePosition after each drag.
pub struct DockIface {
    state: SharedDock,
}

impl DockIface {
    pub fn new(state: SharedDock) -> Self {
        Self { state }
    }
}

#[zbus::interface(name = "io.limitcue.Dock")]
impl DockIface {
    /// Called by the persistent KWin script after the user finishes a drag.
    /// (`callDBus` can only marshal plain ints — no u8s, no structs.)
    fn store_position(&mut self, x: i32, y: i32, edge: i32) {
        // Keep whatever output we last learned about: the persistent script
        // does not send one, and forgetting it here would cost the app its
        // multi-monitor bearings after every drag.
        let mut st = *self.state.lock().unwrap();
        st.edge = Edge::from_u8(edge.clamp(0, 4) as u8);
        st.x = x;
        st.y = y;
        save_dock(&st);
        *self.state.lock().unwrap() = st;
    }

    /// Same, but tagged with the window's pid. Only one instance can own the
    /// well-known bus name, so with two running — routine while developing —
    /// every report lands in the same process regardless of whose window moved.
    /// The app now paints the notch relative to its own window position, so a
    /// stray report would visibly shift somebody's notch. Reports that are not
    /// about this process are dropped.
    fn store_position_for_pid(&mut self, x: i32, y: i32, edge: i32, pid: u32) {
        if pid != std::process::id() {
            return;
        }
        self.store_position(x, y, edge);
    }

    /// Position plus the geometry of the output it landed on. Only the app's
    /// own generated script calls this, so it can carry more than the
    /// installed persistent script knows how to send.
    #[allow(clippy::too_many_arguments)]
    fn store_geometry(
        &mut self,
        x: i32,
        y: i32,
        edge: i32,
        pid: u32,
        out_x: i32,
        out_y: i32,
        out_w: i32,
        out_h: i32,
    ) {
        if pid != std::process::id() {
            return;
        }
        let st = DockState {
            edge: Edge::from_u8(edge.clamp(0, 4) as u8),
            x,
            y,
            out_x,
            out_y,
            out_w,
            out_h,
        };
        save_dock(&st);
        *self.state.lock().unwrap() = st;
    }
}

/// Ask KWin (via its Scripting D-Bus interface) to place our window.
///
/// `y` and `height` are baked into a generated one-shot script; `x` is left to
/// the script, which snaps to whichever side the window's midpoint is nearer
/// and reports the settled edge back over D-Bus so the app persists it.
///
/// This is how the notch gets *headroom*. The window's top edge is pinned once
/// mapped — winit cannot position a Wayland window, and the persistent dock
/// script only re-clamps `x` for a side dock — so a hover card can never be
/// drawn above the window's top. Instead the app asks for a window taller than
/// the notch, positioned so the notch still lands where the user put it, and
/// paints the notch at an offset inside it. The space above the notch is then
/// the card's to use.
///
/// Best-effort: on a compositor without the script, nothing moves and the card
/// falls back to the space below the notch.
pub fn request_geometry(y: i32, height: i32) {
    let dir = cache_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("apply.js");
    let js = format!(
        r#"// generated one-shot by limitcue: place the notch window
// The notch docks to the left/right sides only: snap to whichever side the
// window's midpoint is nearer, take the app's requested y and height, clamp
// the result into the output, and report the side edge back so the app
// persists it.
//
// The window is found by pid first: matching on resourceClass alone would
// grab whichever limitcue instance KWin happened to list first, so a second
// instance (or a developer's test run) moved someone else's notch.
const W = "limitcue";
const PID = {pid};
const WANT_Y = {y};
const WANT_H = {height};
let target = null;
for (const w of workspace.windowList()) {{
    if (w.pid === PID) {{ target = w; break; }}
}}
if (!target) {{
    for (const w of workspace.windowList()) {{
        if ((w.resourceClass + "").indexOf(W) >= 0) {{ target = w; break; }}
    }}
}}
if (target) {{
    let w = target;
    w.keepAbove = true;
    let a = w.output ? w.output.geometry : workspace.virtualScreenGeometry;
    let g = w.frameGeometry;
    let mid = g.x + g.width / 2;
    let edge = (mid < a.x + a.width / 2) ? 3 : 4;
    let nx = (edge === 3) ? a.x : a.x + a.width - g.width;
    let nh = WANT_H > 0 ? WANT_H : g.height;
    let ny = WANT_Y >= 0 ? WANT_Y : g.y;
    ny = Math.min(Math.max(ny, a.y), a.y + a.height - nh);
    w.frameGeometry = {{ x: nx, y: ny, width: g.width, height: nh }};
    callDBus("io.limitcue", "/io/limitcue/dock", "io.limitcue.Dock",
             "StoreGeometry", Math.round(nx), Math.round(ny), edge, PID,
             Math.round(a.x), Math.round(a.y), Math.round(a.width), Math.round(a.height));
    print("LC-PLACE pid=" + w.pid + " edge=" + edge + " " + nx + "," + ny + " h=" + nh +
          " on " + a.x + "," + a.y + " " + a.width + "x" + a.height);
}}
"#,
        pid = std::process::id(),
    );
    if std::fs::write(&path, js).is_err() {
        return;
    }
    let ok = (|| -> Result<(), Box<dyn std::error::Error>> {
        let conn = zbus::blocking::Connection::session()?;
        let scripting: zbus::blocking::Proxy<'_> = zbus::blocking::Proxy::new(
            &conn,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        )?;
        // unload stale copy under the same plugin name, then load + start
        scripting.call::<_, _, bool>("unloadScript", &("limitcue-restore"))?;
        scripting.call::<_, _, i32>(
            "loadScript",
            &(path.to_string_lossy().to_string(), "limitcue-restore"),
        )?;
        scripting.call::<_, _, ()>("start", &())?;
        Ok(())
    })();
    if let Err(e) = ok {
        eprintln!("limitcue: window placement not applied ({e})");
    }
}

pub fn shared() -> SharedDock {
    Arc::new(Mutex::new(load_dock()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(x: i32, out_x: i32, out_w: i32) -> DockState {
        DockState { x, out_x, out_w, out_h: 1080, ..Default::default() }
    }

    #[test]
    fn a_notch_against_the_left_bezel_reads_as_left() {
        assert_eq!(on(0, 0, 1920).side(48), Some(Edge::Left));
    }

    #[test]
    fn a_notch_against_the_right_bezel_reads_as_right() {
        assert_eq!(on(1872, 0, 1920).side(48), Some(Edge::Right));
    }

    #[test]
    fn the_position_wins_over_a_contradicting_edge() {
        // This is the reported bug: dragged to the left, but the stored enum
        // still said right, so the card opened leftwards off the screen.
        let st = DockState { edge: Edge::Right, ..on(0, 0, 1920) };
        assert_eq!(st.side(48), Some(Edge::Left));
    }

    #[test]
    fn a_second_monitor_is_measured_against_its_own_bezels() {
        // A monitor starting at x=1920: x=1920 is its *left* edge, not the
        // desktop's right-hand side.
        assert_eq!(on(1920, 1920, 2560).side(48), Some(Edge::Left));
        assert_eq!(on(1920 + 2560 - 48, 1920, 2560).side(48), Some(Edge::Right));
    }

    #[test]
    fn without_a_reported_output_it_declines_to_guess() {
        assert_eq!(DockState { x: 0, ..Default::default() }.side(48), None);
    }

    #[test]
    fn the_band_is_measured_against_the_monitor_it_is_on() {
        // A monitor stacked below another starts at y=1080.
        let st = DockState { out_y: 1080, out_h: 1440, out_w: 2560, ..Default::default() };
        assert_eq!(st.output_span(900.0), (1080.0, 1440.0));
        // ...and with nothing reported, fall back to a screen at the origin.
        assert_eq!(DockState::default().output_span(900.0), (0.0, 900.0));
    }
}
