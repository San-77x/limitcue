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
use zbus::blocking::connection::Builder;

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
struct DockIface {
    state: SharedDock,
}

#[zbus::interface(name = "io.limitcue.Dock")]
impl DockIface {
    /// Called by the KWin script after the user finishes a drag.
    fn store_position(&mut self, x: i32, y: i32, edge: u8) {
        let st = DockState { edge: Edge::from_u8(edge), x, y };
        save_dock(&st);
        *self.state.lock().unwrap() = st;
    }
}

/// Serve `io.limitcue.Dock` on the session bus.
/// Fails silently (returned) if no bus is available (headless/CI) —
/// docking then just doesn't persist; everything else works.
pub fn start_service(state: SharedDock) -> Result<(), zbus::Error> {
    // A second instance would collide on the well-known name; that's fine —
    // the older instance serves the same data.
    let conn = Builder::session()?
        .name("io.limitcue")?
        .serve_at("/io/limitcue/dock", DockIface { state })?
        .build()?;
    // The connection owns the object server; keep it alive for the process
    // lifetime (single-purpose app).
    std::mem::forget(conn);
    Ok(())
}

/// Ask KWin (via its Scripting D-Bus interface) to apply the persisted dock
/// position to our window. Writes a one-shot script with the geometry baked
/// in, then loads + starts it; KWin runs it against the window that already
/// exists. Best-effort: failures just mean no restore this launch.
pub fn request_restore(state: &DockState) {
    let dir = cache_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("apply.js");
    let edge = state.edge.to_u8();
    let x = state.x;
    let y = state.y;
    let js = format!(
        r#"// generated one-shot by limitcue: restore dock position
const W = "limitcue";
for (const w of workspace.windowList()) {{
    if ((w.resourceClass + "").indexOf(W) < 0) continue;
    w.keepAbove = true;
    let a = w.output ? w.output.geometry : workspace.virtualScreenGeometry;
    let g = w.frameGeometry;
    let nx = g.x, ny = g.y;
    if ({edge} === 1) ny = a.y;
    else if ({edge} === 2) ny = a.y + a.height - g.height;
    else if ({edge} === 3) nx = a.x;
    else if ({edge} === 4) nx = a.x + a.width - g.width;
    else {{
        nx = Math.min(Math.max({x}, a.x), a.x + a.width - g.width);
        ny = Math.min(Math.max({y}, a.y), a.y + a.height - g.height);
    }}
    w.frameGeometry = {{ x: nx, y: ny, width: g.width, height: g.height }};
    print("LC-RESTORE applied edge={edge} " + nx + "," + ny);
    break;
}}
"#
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
        eprintln!("limitcue: dock restore not applied ({e})");
    }
}

pub fn shared() -> SharedDock {
    Arc::new(Mutex::new(load_dock()))
}
