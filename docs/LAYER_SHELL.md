# Layer-shell: design spike

**Status:** design only, not implemented. This document exists so the work is
de-risked before anyone starts it, and so the reason it is not a one-file
feature is on record.

## The goal

Portable edge-pinning. Today, "dock the notch flush to the screen edge and
remember where it was" is KDE-only, done by a KWin script
(`misc/kwin/limitcue-integrate/`) that moves the window from the compositor
side and reports the position back over D-Bus. On GNOME, sway, Hyprland and
everything else, the notch is a normal window that the user can place but the
app cannot pin.

The portable Wayland answer is the `wlr-layer-shell-unstable-v1` protocol: a
surface that anchors itself to screen edges and reserves space, which is
exactly what a panel or a notch is.

## Why eframe cannot do this today

eframe renders through `egui-winit` → `winit`. The winit version in the tree
(0.30.x) has **no layer-shell support** — its Wayland backend creates
`xdg-shell` toplevels, which a compositor is free to place anywhere and an app
cannot pin to an edge. There is no eframe/winit option, feature or config that
changes this; the surface type is chosen inside winit.

That rules out every approach that keeps eframe as the windowing layer:

- `ViewportCommand` can move an `xdg-shell` window only on X11, and only
  where the compositor allows; on Wayland a client cannot position itself.
- A layer-shell surface cannot be "attached" to an eframe window, because the
  surface is the window.

## The shape of a real implementation

A second, opt-in windowing backend. The application logic does not change — it
already lives behind `eframe::App`, whose `logic()`/`ui()` split is exactly
"tick, then draw into a `Ui`" — but the host does.

1. **Wayland connection.** `wayland-client` + `wayland-protocols-wlr` (for
   `zwlr_layer_shell_v1`) via `smithay-client-toolkit` for the registry,
   outputs, input and the surface lifecycle. sctk is already in the dependency
   tree transitively (`smithay-clipboard`), so this is not a new ecosystem.
2. **A wgpu surface.** `egui-wgpu` exposes a `Renderer` over a
   `wgpu::Surface`. The layer-shell surface is configured with a raw
   `wgpu::SurfaceTarget` on the Wayland display/window handles, at the
   `wl_output`'s scale, and presented on `wl_surface.commit`.
3. **An egui host.** Drive `egui::Context::run` with a `RawInput` built from
   Wayland events translated by hand: pointer motion/enter/leave/button,
   keyboard (only `R`, `Esc`, and text entry inside the settings sheet),
   `wl_output` scale and geometry, and the frame clock. This is the largest
   and least glamorous part; `egui-winit` does all of it today and cannot be
   reused because it is bound to a winit window.
4. **Placement, which gets easier.** Layer-shell clients *can* set their own
   position, through anchors and margins. So the whole KWin-script handshake —
   `dock.rs`, the one-shot placement script, `StorePosition`, the headroom
   band — is **not needed** on this path: the surface anchors to an edge and
   offsets by margins, and moving the notch is a margin update. That is a
   strong argument that this backend is not just parity, it is simpler than
   the KDE one.
5. **Runtime selection.** `--features layer-shell` builds the alternate host,
   or the binary probes for the protocol at startup and falls back to eframe.
   A feature flag is safer for a first cut, so the working path is never at
   risk.

### What carries over unchanged

`Provider`, `Config`, `state.json`, `history.rs`, `notify.rs`, `activity.rs`,
`cli.rs` and the whole `ui/` module are host-agnostic. `service.rs` (D-Bus and
the socket) is glue that does not care which window exists. `dock.rs` is
KDE-specific and would simply be bypassed.

## Risks and open questions

- **Input coverage.** Keyboard text entry (provider keys, custom URLs) has to
  work through the hand-written translation, including modifier state. This is
  where a from-scratch host most often feels broken.
- **Fractional scaling.** Layer-shell surfaces are per-`wl_output`; scale
  changes need a reconfigure and a surface reconfigure, and the notch size
  math (`RAIL_*` constants, the headroom band) is written in egui points.
- **Multi-monitor.** The surface must be created on the right `wl_output`, and
  recreated when the user drags it across (which on this path means changing
  the surface's output, a destroy-and-recreate).
- **Two backends to maintain.** Every eframe upgrade touches one; every change
  to window behaviour now has two paths and two test matrices. The repo's CI
  runs headless, so neither path gets automated UI coverage today.
- **Upstream movement.** winit has had layer-shell discussions; if it lands,
  this entire document collapses into a config flag. Worth re-checking before
  starting.

## Recommendation

Do **not** block 1.0 on this. The KWin script covers the most common Linux
desktop, and the app is usable without any docking at all. When it is picked
up:

1. Prototype steps 1–3 outside the repo, on a single monitor at scale 1.0,
   proving that egui can be driven and painted on a layer-shell surface.
2. Only if that works, add it behind `--features layer-shell` with the eframe
   host as the default.
3. Port edge-snapping to anchors/margins (step 4) and delete `dock.rs` from
   that path, keeping it for KDE.

Stop after step 1 if the input translation does not feel solid — a notch that
cannot take a pasted API key is worse than one that cannot pin itself.
