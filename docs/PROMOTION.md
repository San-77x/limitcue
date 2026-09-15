# Promotion checklist

Everything here is optional, but it's the difference between a repository
that exists and one people can find.

## Already done

- Repository description, 12 topics, and homepage URL set.
- GitHub Discussions enabled.
- Landing page live at <https://san-77x.github.io/limitcue/>.
- `v0.2.0` release with a Linux x86_64 tarball.
- Secret scanning and push protection on.

## Do these by hand (GitHub has no API for them)

1. **Social preview image.** Upload
   [`assets/screenshots/social-preview.png`](../assets/screenshots/social-preview.png)
   at **Settings → General → Social preview**. It is the card shown when the
   repository is linked on Slack, X, Discord, and so on.
2. **Pin the repository** to your GitHub profile (Profile → Customize pins).
3. **Star it yourself** — a repo with zero stars reads as abandoned.

## Announcement blurb

Short version, for a post or a comment:

> **LimitCue** — a tiny always-on-top notch for Linux. It shows what's
> left on every AI coding plan you're signed into (Claude, Codex, Grok,
> MiniMax, Kimi, gateways, or any JSON endpoint) without opening a browser or
> a terminal. Hover a gauge for the numbers; drag it to a screen edge and it
> docks flush. It borrows the credentials your provider CLI already saved, never
> signs you in, and sends nothing anywhere except each provider's own API —
> no telemetry. Rust + egui, one ~6 MB binary, Wayland and X11.
> https://github.com/San-77x/limitcue

Where it tends to land well:

- r/linux, r/selfhosted, r/LocalLLaMA, r/ClaudeAI (only where self-promotion
  is allowed — read each subreddit's rules first).
- Hacker News *Show HN*, posted as an individual, not a launch.
- The KDE / Wayland communities, since edge docking is Plasma-specific today.

## Before you post

- Make sure the landing page's download button reaches a real release (it does).
- Have a screenshot ready; the first reply is usually "what does it look like?".
- Be around to answer questions for a few hours — a Show HN with no author in
  the thread dies.
