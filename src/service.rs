//! The local read interfaces: D-Bus and a unix socket.
//!
//! `--json` covers scripts that can afford to spawn a process. Anything that
//! polls often — a status bar, an editor plugin, a shell prompt — wants the
//! cached reading without paying for a fetch, which is what these serve.
//!
//! Both are read-only, local-only, and carry exactly what the notch shows.

use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::dock::{DockIface, SharedDock};
use crate::types::Snapshot;

/// What the readers see: the last poll's snapshots, plus a flag the app checks
/// so a caller can ask for a fresh one.
#[derive(Default)]
pub struct Usage {
    pub snaps: Mutex<Vec<Snapshot>>,
    pub refresh_wanted: AtomicBool,
    /// Set once the bus is up, so a new reading can raise `Changed` without
    /// the poller knowing anything about D-Bus.
    conn: Mutex<Option<zbus::blocking::Connection>>,
}

pub type SharedUsage = Arc<Usage>;

impl Usage {
    pub fn store(&self, snaps: &[Snapshot]) {
        if let Ok(mut g) = self.snaps.lock() {
            *g = snaps.to_vec();
        }
        self.emit_changed();
    }

    /// Emit `io.limitcue.Usage.Changed` with the same JSON `Get` returns.
    ///
    /// A reader that only wants to know when to re-read can subscribe to this
    /// instead of polling `Get`; the payload saves it a round trip. Failure is
    /// ignored: a signal that could not be sent must not take the reading with
    /// it, and the next `Get` still answers.
    fn emit_changed(&self) {
        let Ok(guard) = self.conn.lock() else { return };
        let Some(conn) = guard.as_ref() else { return };
        let Ok(iface) = conn
            .object_server()
            .interface::<_, UsageIface>("/io/limitcue/usage")
        else {
            return;
        };
        let json = self.json();
        let _ = zbus::block_on(UsageIface::changed(iface.signal_emitter(), &json));
    }

    pub fn json(&self) -> String {
        let snaps = self.snaps.lock().map(|g| g.clone()).unwrap_or_default();
        crate::cli::json_doc(&snaps)
    }

    /// Did someone ask for a refresh since the last check? Clears the flag.
    pub fn take_refresh(&self) -> bool {
        self.refresh_wanted.swap(false, Ordering::Relaxed)
    }
}

struct UsageIface {
    usage: SharedUsage,
}

#[zbus::interface(name = "io.limitcue.Usage")]
impl UsageIface {
    /// The current reading, in the same shape `limitcue --json` prints.
    fn get(&self) -> String {
        self.usage.json()
    }

    /// Ask the app to poll now. Returns immediately — the result arrives on
    /// the next `Get`, since a caller should not be blocked on someone else's
    /// rate limit.
    fn refresh(&self) {
        self.usage.refresh_wanted.store(true, Ordering::Relaxed);
    }

    /// Raised whenever a new reading is stored, carrying the same JSON as
    /// `Get`. A subscriber can drop its polling loop and read this instead.
    #[zbus(signal)]
    async fn changed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        json: &str,
    ) -> zbus::Result<()>;
}

/// Own `io.limitcue` and serve both objects on it: the dock position the KWin
/// script reports into, and the usage readers. One connection, because a
/// well-known name can only be owned once.
pub fn start_dbus(dock: SharedDock, usage: SharedUsage) -> Result<(), zbus::Error> {
    let conn = zbus::blocking::connection::Builder::session()?
        .name("io.limitcue")?
        .serve_at("/io/limitcue/dock", DockIface::new(dock))?
        .serve_at(
            "/io/limitcue/usage",
            UsageIface {
                usage: usage.clone(),
            },
        )?
        .build()?;
    // The connection owns the object server and must outlive this call. The
    // usage handle keeps it alive, and is what `emit_changed` reaches through
    // to raise the signal.
    if let Ok(mut slot) = usage.conn.lock() {
        *slot = Some(conn);
    }
    Ok(())
}

/// `$XDG_RUNTIME_DIR/limitcue.sock`: connect, read one JSON line, done. No
/// request format to learn and nothing to get wrong — `socat - UNIX:...` or
/// `nc -U` is a complete client.
pub fn start_socket(usage: SharedUsage) {
    let Some(path) = socket_path() else { return };
    // A stale socket from a crashed run would block the bind; ours is the only
    // process that should own this name.
    let _ = std::fs::remove_file(&path);
    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("limitcue: no usage socket at {} ({e})", path.display());
            return;
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut line = usage.json();
            line.push('\n');
            let _ = stream.write_all(line.as_bytes());
        }
    });
}

pub fn socket_path() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .or_else(dirs::runtime_dir)
        .map(|d| d.join("limitcue.sock"))
}
