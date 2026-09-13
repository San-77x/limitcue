# Promotion checklist

Everything here is optional, but it's the difference between a repository
that exists and one people can find.

## Already done

- Repository description, 12 topics, and homepage URL set.
- GitHub Discussions enabled.
- Landing page live at <https://san-77x.github.io/limitcue/>.
- `v0.1.1` release with Linux x86_64 tarball + `.sha256`
  (`limitcue-linux-x86_64.tar.gz`).
- AUR `limitcue-bin` packaging and `cargo binstall` metadata pointed at that asset.
- Secret scanning and push protection on.
- Public roadmap in `docs/ROADMAP.md` (internal planning docs removed; no poetic essay / STATUS / v2 handoff in-tree).

## Do these by hand (GitHub has no API for them)

1. **Social preview image.** Upload
   [`assets/screenshots/social-preview.png`](../assets/screenshots/social-preview.png)
   at **Settings → General → Social preview**. It is the card shown when the
   repository is linked on Slack, X, Discord, and so on.
2. **Pin the repository** to your GitHub profile (Profile → Customize pins).
3. **Star it yourself** — a repo with zero stars reads as abandoned.
4. **Fill the GitHub release body** for `v0.1.1` (currently empty) — paste the
   CHANGELOG `[0.1.1]` section, or link it. Empty release notes look unfinished.

## Ship notes (v0.1.1)

Facts to match the binary people download:

- Tag / latest: https://github.com/San-77x/limitcue/releases/tag/v0.1.1
- Asset: `limitcue-linux-x86_64.tar.gz` (~4.1 MB) + `limitcue-linux-x86_64.tar.gz.sha256`
- Install paths also covered: AUR `limitcue-bin`, `cargo binstall limitcue`
- What shipped vs `v0.1.0`: optional tray (`tray = true`), `hide_when_idle`,
  D-Bus `io.limitcue.Usage.Changed`, accesskit labels, packaging + `.sha256`,
  dependency majors (toml 1, ureq 3, zbus 5, eframe/egui 0.36; Rust 1.95+)
- What it is: always-on-top Linux quota pill for AI coding plans; borrows
  existing CLI credentials; no sign-in of its own; no telemetry; MIT; Rust + egui

Do not post a ~6 MB or ~6.5 MB size. The shipped tarball is **~4.1 MB**.

## Announcement blurb

Short version, for a post or a comment. Size matches the Nix bar / tarball:

> **LimitCue** — a tiny always-on-top quota pill for Linux. It shows what's
> left on every AI coding plan you're signed into (Claude, Codex, Grok,
> MiniMax, Kimi, gateways, or any JSON endpoint) without opening a browser or
> a terminal. It borrows the credentials your provider CLI already saved, never
> signs you in, and sends nothing anywhere except each provider's own API —
> no telemetry. Rust + egui, one ~4.1 MB tarball, Wayland and X11.
> https://github.com/San-77x/limitcue

Where it tends to land well:

- r/linux, r/selfhosted, r/LocalLLaMA, r/ClaudeAI (only where self-promotion
  is allowed — read each subreddit's rules first).
- Hacker News *Show HN*, posted as an individual, not a launch.
- The KDE / Wayland communities, since edge docking is Plasma-specific today.

## Before you post

- Confirm the landing download button reaches a real release (it links to
  the latest `limitcue-linux-x86_64.tar.gz` when present; latest is `v0.1.1`).
- Have a screenshot ready; the first reply is usually "what does it look like?".
- Be around to answer questions for a few hours — a Show HN with no author in
  the thread dies.
- Proof / size line is **~4.1 MB · Rust · no Electron · no telemetry · MIT · Linux**.
