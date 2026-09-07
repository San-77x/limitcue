// LimitCue compositor integration: keep-above + edge docking.
//
// Wayland apps can't position their own windows, so all the geometry work
// lives here:
//   - keep the pill above all windows;
//   - when the user finishes a drag near an edge, snap flush to it;
//   - when the app resizes itself (expand/collapse), re-clamp so the pill
//     stays glued to its edge;
//   - report every settled position to the app over D-Bus (io.limitcue.Dock
//     StorePosition) so it persists to dock.json.
//
// The startup *restore* direction is app-driven: the app writes a one-shot
// generated script (limitcue-restore) via KWin's Scripting interface, since
// callDBus on this Plasma build cannot return values.
//
// Edges: 0 free, 1 top, 2 bottom, 3 left, 4 right.

const SNAP_PX = 32; // distance from an output edge that triggers docking
const APP = "limitcue";

function isLimitCue(w) {
    return (w.resourceClass + "").indexOf(APP) >= 0 || (w.caption + "").indexOf(APP) >= 0;
}

// ---- geometry helpers ------------------------------------------------------

// The visible area of the window's output (struts already removed).
function workArea(w) {
    let out = w.output;
    if (out) { return out.geometry; }
    return workspace.virtualScreenGeometry;
}

// Frame geometry flush against `edge` of the window's output.
// (Assign plain objects — QML's Qt.rect is not available in KWin scripts.)
function flushGeometry(w, edge) {
    let g = w.frameGeometry;
    let a = workArea(w);
    if (edge === 1) return { x: g.x, y: a.y, width: g.width, height: g.height };               // top
    if (edge === 2) return { x: g.x, y: a.y + a.height - g.height, width: g.width, height: g.height }; // bottom
    if (edge === 3) return { x: a.x, y: g.y, width: g.width, height: g.height };               // left
    if (edge === 4) return { x: a.x + a.width - g.width, y: g.y, width: g.width, height: g.height };   // right
    return g;
}

function isFlush(w, edge) {
    let g = w.frameGeometry;
    let a = workArea(w);
    if (edge === 1) return Math.abs(g.y - a.y) < 1;
    if (edge === 2) return Math.abs(g.y + g.height - (a.y + a.height)) < 1;
    if (edge === 3) return Math.abs(g.x - a.x) < 1;
    if (edge === 4) return Math.abs(g.x + g.width - (a.x + a.width)) < 1;
    return false;
}

// Which edge the window is currently closest to (within SNAP_PX), else 0.
function nearestEdge(w) {
    let g = w.frameGeometry;
    let a = workArea(w);
    let dTop = Math.abs(g.y - a.y);
    let dBottom = Math.abs(g.y + g.height - (a.y + a.height));
    let dLeft = Math.abs(g.x - a.x);
    let dRight = Math.abs(g.x + g.width - (a.x + a.width));
    let best = 0, bestD = SNAP_PX;
    if (dTop < bestD) { best = 1; bestD = dTop; }
    if (dBottom < bestD) { best = 2; bestD = dBottom; }
    if (dLeft < bestD) { best = 3; bestD = dLeft; }
    if (dRight < bestD) { best = 4; bestD = dRight; }
    return best;
}

// Re-clamp to the docked edge unless the user is mid-drag; keeps the pill
// attached when the app resizes itself (expand/collapse animation).
function clampToEdge(w, edge) {
    if (!edge || edge === 0 || w.move || w.resize) return;
    if (isFlush(w, edge)) return;
    let target = flushGeometry(w, edge);
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
    print("LC-DOCK pinned " + w.resourceClass);

    w.interactiveMoveResizeFinished.connect(function () {
        let edge = nearestEdge(w);
        if (edge > 0) {
            w.frameGeometry = flushGeometry(w, edge);
            print("LC-DOCK snapped to edge " + edge);
        }
        reportPosition(w, edge);
    });

    w.frameGeometryChanged.connect(function () {
        if (!isLimitCue(w) || w.move || w.resize) return;
        // re-clamp to whatever edge we're flush-ish against
        let edge = nearestEdge(w);
        clampToEdge(w, edge);
    });
}

workspace.windowAdded.connect(onWindowAdded);
for (const w of workspace.windowList()) { if (isLimitCue(w)) onWindowAdded(w); }
