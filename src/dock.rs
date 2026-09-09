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

/// The dock position as last reported by the compositor.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct DockState {
    pub edge: Edge,
    pub x: i32,
    pub y: i32,
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
        let st = DockState { edge: Edge::from_u8(edge.clamp(0, 4) as u8), x, y };
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
             "StorePositionForPid", Math.round(nx), Math.round(ny), edge, PID);
    print("LC-PLACE pid=" + w.pid + " edge=" + edge + " " + nx + "," + ny + " h=" + nh);
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
