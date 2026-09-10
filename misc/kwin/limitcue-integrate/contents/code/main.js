// LimitCue compositor integration: keep-above + edge docking.
//
// Wayland apps can't position their own windows, so all the geometry work
// lives here:
//   - keep the pill above all windows;
//   - when the user finishes a drag near an edge, snap flush to it;
//   - when the app resizes itself (a hover card opening), re-clamp so the
//     pill stays glued to its edge;
//   - report every settled position to the app over D-Bus (io.limitcue.Dock
//     StorePosition) so it persists to dock.json.
//
// The startup *placement* direction is app-driven: the app writes a one-shot
// generated script (limitcue-restore) via KWin's Scripting interface, since
// callDBus on this Plasma build cannot return values.
//
// Edges: 0 free, 1 top, 2 bottom, 3 left, 4 right.

const APP = "limitcue";
// Bumped whenever the app needs a behaviour only a newer script has. The app
// reads this back and says so if the installed copy is behind.
const LC_SCRIPT_VERSION = 3;

function isLimitCue(w) {
    return (w.resourceClass + "").indexOf(APP) >= 0 || (w.caption + "").indexOf(APP) >= 0;
}

// ---- geometry helpers ------------------------------------------------------

// The output the window is *currently* on. Only trustworthy while the window
// is at rest: a hover card widens it, and a wide window straddling a monitor
// boundary is reassigned to whichever output it now covers most.
function currentOutput(w) {
    let out = w.output;
    if (out) { return out.geometry; }
    return workspace.virtualScreenGeometry;
}

// Frame geometry flush against `edge` of `a`.
// (Assign plain objects — QML's Qt.rect is not available in KWin scripts.)
function flushGeometry(w, edge, a) {
    let g = w.frameGeometry;
    if (edge === 1) return { x: g.x, y: a.y, width: g.width, height: g.height };               // top
    if (edge === 2) return { x: g.x, y: a.y + a.height - g.height, width: g.width, height: g.height }; // bottom
    if (edge === 3) return { x: a.x, y: g.y, width: g.width, height: g.height };               // left
    if (edge === 4) return { x: a.x + a.width - g.width, y: g.y, width: g.width, height: g.height };   // right
    return g;
}

function isFlush(w, edge, a) {
    let g = w.frameGeometry;
    if (edge === 1) return Math.abs(g.y - a.y) < 1;
    if (edge === 2) return Math.abs(g.y + g.height - (a.y + a.height)) < 1;
    if (edge === 3) return Math.abs(g.x - a.x) < 1;
    if (edge === 4) return Math.abs(g.x + g.width - (a.x + a.width)) < 1;
    return false;
}

// Which SIDE edge of `a` the window is closest to. The notch docks left/right
// only — top/bottom drags snap to whichever side is nearer, so a window is
// never left undocked.
function nearestEdge(w, a) {
    let g = w.frameGeometry;
    let dLeft = Math.abs(g.x - a.x);
    let dRight = Math.abs(g.x + g.width - (a.x + a.width));
    return dLeft <= dRight ? 3 : 4;
}

// Re-clamp to the docked edge unless the user is mid-drag; keeps the pill
// attached when the app resizes itself to open a hover card.
//
// `a` is the output the window was *docked to*, not the one it currently
// overlaps. That distinction is the whole point of this version: a card
// opening on a right-docked notch widens the window rightwards, and on an
// extended desktop that pushes it across the boundary. KWin then reassigns it
// to the next monitor, and clamping against *that* output snapped the notch to
// the far screen's left bezel — so the notch hopped displays and its card
// started opening on the wrong side. Measured against the docked output, the
// same widening is simply pulled back so the window grows leftwards and the
// notch never moves.
function clampToEdge(w, edge, a) {
    if (!edge || edge === 0 || w.move || w.resize) return;
    if (isFlush(w, edge, a)) return;
    let target = flushGeometry(w, edge, a);
    if (target !== w.frameGeometry) {
        w.frameGeometry = target;
        print("LC-DOCK reclamped to edge " + edge + " -> " + JSON.stringify(target));
    }
}

// Report the settled position to the app (it persists to dock.json and
// uses it to restore next launch).
function reportPosition(w, edge) {
    let g = w.frameGeometry;
    callDBus("io.limitcue", "/io/limitcue/dock", "io.limitcue.Dock",
             "StorePosition", Math.round(g.x), Math.round(g.y), edge);
    print("LC-DOCK stored " + g.x + "," + g.y + " edge=" + edge);
}

// ---- event wiring ------------------------------------------------------------

function onWindowAdded(w) {
    if (!isLimitCue(w)) return;
    w.keepAbove = true;
    print("LC-DOCK pinned " + w.resourceClass + " (script v" + LC_SCRIPT_VERSION + ")");

    // The output and edge this window is docked to.
    //
    // `w.output` is only trustworthy while the window is narrow: at its
    // resting width the notch sits wholly on one monitor, so whatever KWin
    // says is right. Once a hover card widens it, a window docked near a
    // shared boundary straddles two monitors and gets reassigned to the other
    // one — so the docked output is remembered from the last time the window
    // was at its narrowest, and held for as long as it is wide.
    let docked = { edge: 0, area: null };
    let restingWidth = w.frameGeometry.width;

    function reDock() {
        docked.area = currentOutput(w);
        docked.edge = nearestEdge(w, docked.area);
    }
    reDock();

    // Re-derive whenever the window is back to (or below) its narrowest seen
    // width. That covers a drag, and the app placing itself at startup.
    function reDockIfAtRest() {
        let width = w.frameGeometry.width;
        if (width <= restingWidth) {
            restingWidth = width;
            reDock();
            return true;
        }
        return false;
    }

    w.interactiveMoveResizeFinished.connect(function () {
        // The user chose where this goes, so believe the current output even
        // if a card happens to be open.
        restingWidth = w.frameGeometry.width;
        reDock();
        w.frameGeometry = flushGeometry(w, docked.edge, docked.area);
        print("LC-DOCK snapped to edge " + docked.edge);
        reportPosition(w, docked.edge);
    });

    w.frameGeometryChanged.connect(function () {
        if (!isLimitCue(w) || w.move || w.resize) return;
        if (!docked.area) { reDock(); }
        // A narrow window is wherever KWin says it is; a wide one is still
        // docked where it was before the card opened.
        reDockIfAtRest();
        clampToEdge(w, docked.edge, docked.area);
    });
}

workspace.windowAdded.connect(onWindowAdded);
for (const w of workspace.windowList()) { if (isLimitCue(w)) onWindowAdded(w); }
