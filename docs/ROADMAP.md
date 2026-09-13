# Roadmap

This is a living document. Priorities are driven by what makes LimitCue
useful to someone who codes against a metered plan every day — not by
feature count. Anything here can be reordered by an issue or a pull
request; nothing here is a promise.

## Now — the core loop

- Keep the notch honest: every adapter pinned to a real endpoint, every
  reading labelled with its fidelity (`official` / `derived` / `manual`),
  failures degrading to visible states rather than invented numbers.
- Keep it small: a single native binary, no runtime, numbers-only local
  cache, no telemetry.

## Next — quality of life

- **Wayland layer-shell** — portable edge pinning and a true notch on
  compositors other than KDE Plasma, replacing the KWin-script dependency
  for placement. This is the largest remaining piece. The approach and its
  costs are worked through in [LAYER_SHELL.md](LAYER_SHELL.md).
- **Packaging** — AUR (`limitcue-bin`), AppImage, Flatpak, Nix flake, and
  `cargo-binstall` metadata, so installing does not mean compiling.
- **More first-party adapters** — GLM / Z.ai, GitHub Copilot, Windsurf,
  Gemini CLI. The generic custom-provider path already covers endpoints
  that report usage as JSON; first-party adapters are for vendors whose
  numbers need interpretation.
- **Cost rollup** — API-equivalent spend for today and the week, derived
  from local session logs and labelled `derived`, because a subscription
  is not a bill.

## Later — reach

- **Windows and macOS builds.** The adapters come first: a platform is
  only supported once its credential sources are understood, not once it
  compiles.
- **A docs page per adapter** — endpoint, auth source, and the reasoning
  behind its fidelity label.
- **Change notifications over D-Bus** so a reader can subscribe instead of
  polling `Usage.Get`.
- **Screensaver / idle hide**, and richer session awareness (the current
  pulse reads modification times only; listing live sessions would mean
  reading log contents, which is a privacy decision we have not taken).

## Engineering

- Fixture tests for every adapter response shape, so upstream drift turns
  red in CI instead of at a user's desk.
- Extract the UI state machine out of `main.rs` and add snapshot tests for
  the expand/collapse and card-placement paths.
- Adopt `cargo fmt` across the tree as a deliberate, once-and-for-all
  commit, then turn on `--check` in CI.
- Supply-chain checks (`cargo deny` / `cargo vet`) in CI.

## Non-goals

These are deliberate and are not up for debate:

- No sign-in flow of our own, no OAuth token refresh, no proxy for model
  traffic.
- No cloud sync, no accounts, no analytics, ever.
- No invented numbers. An adapter without a real endpoint ships as
  `derived`, or it does not ship.
