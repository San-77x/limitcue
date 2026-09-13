# Changelog

All notable changes to this project are documented here. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

Nothing yet.

## [0.1.0] - 2026-09-13

The first public release.

### Added

- A borderless, always-on-top pill that shows remaining quota for every AI
  coding plan you are signed into, with a heat-coloured ring gauge per
  provider and a `+N` overflow chip.
- First-party adapters for Claude (Max/Pro), ChatGPT / Codex, Grok / xAI,
  MiniMax, and Kimi For Coding, plus a billing adapter for New-API style
  gateways and a generic custom provider for any JSON usage endpoint.
- A vertical rail / notch mode for left and right edge docking, with
  hover usage cards, a live-session pulse, and burn-rate projections.
- Ten themes, each recolouring every surface and carrying its own glow
  weight, plus configurable notch and card opacity.
- Credential borrowing from each vendor's existing CLI with **no sign-in
  flow of our own** and no telemetry.
- Low-quota and refill notifications, latched per window, with
  `cmd_on_low` / `cmd_on_reset` hooks and `--test-alert`.
- A machine-readable surface: `--once`, `--json`, `--line`, `--waybar`,
  `--watch`, `limitcue wait`, the `io.limitcue.Usage` D-Bus interface, and
  a local Unix socket.
- `limitcue init` first-run detection, in-app settings, provider
  catalogue, and config hot-reload.
- A KWin integration for edge snapping and persisted dock position on KDE
  Plasma (Wayland and X11).

[Unreleased]: https://github.com/San-77x/limitcue/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/San-77x/limitcue/releases/tag/v0.1.0
