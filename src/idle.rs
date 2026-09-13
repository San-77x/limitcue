//! Idle and lock detection, for hiding the notch while you are away.
//!
//! Reads `IdleHint` and `LockedHint` from the current session on systemd-logind
//! over the system bus. Best effort by design: on a machine without logind
//! (a container, a non-systemd init, a desktop that never sets the hint) the
//! query fails, the monitor reports "active", and the notch simply never
//! hides. Nothing here is required for the app to work.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// True while the session is idle or locked.
pub type SharedIdle = Arc<AtomicBool>;

/// How often the hint is re-read. Activity has to bring the notch back
/// promptly, and this is one cheap property read on the system bus.
const POLL: Duration = Duration::from_secs(5);

/// Start the monitor; the returned flag is updated in the background.
pub fn start() -> SharedIdle {
    let idle = Arc::new(AtomicBool::new(false));
    let flag = idle.clone();
    std::thread::spawn(move || loop {
        flag.store(query(), Ordering::Relaxed);
        std::thread::sleep(POLL);
    });
    idle
}

/// The session is idle or locked, according to logind. `false` when logind
/// cannot be reached — never hide on a guess.
fn query() -> bool {
    let Ok(conn) = zbus::blocking::Connection::system() else {
        return false;
    };
    for path in [
        "/org/freedesktop/login1/session/auto",
        "/org/freedesktop/login1/session/self",
    ] {
        let Ok(proxy) = zbus::blocking::Proxy::new(
            &conn,
            "org.freedesktop.login1",
            path,
            "org.freedesktop.login1.Session",
        ) else {
            continue;
        };
        let idle: bool = proxy.get_property("IdleHint").unwrap_or(false);
        let locked: bool = proxy.get_property("LockedHint").unwrap_or(false);
        return idle || locked;
    }
    false
}
