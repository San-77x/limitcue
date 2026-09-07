# LimitCue

A tiny always-on-top quota pill for your Linux desktop. See how much usage is
left on every AI coding plan you're signed into — without opening a browser,
a terminal, or trusting a made-up number.

```
 ● CODEX 63%   ● CLAUDE 22%   ● M3 69%
```

Built with Rust + egui as a single ~5 MB binary — no Electron, no webview, no runtime.

## What it does

- **One pill, many plans.** A dot + short label per provider, colour-coded
  (green > 50%, amber > 15%, red, grey = stale/error).
- **Drag it anywhere.** Borderless, always-on-top window. Hover a provider for
  per-window detail: remaining %, counts, and reset countdowns. Click `⋯` for
  refresh / quit.
- **Borrows existing credentials.** It never signs you in anywhere and never
  sends your keys anywhere but the owning provider's API:

  | Provider | Source it reads | Endpoint |
  |---|---|---|
  | Claude (Max/Pro) | `~/.claude/.credentials.json` | Anthropic OAuth usage API (what `/usage` uses) |
  | ChatGPT / Codex | `~/.codex/auth.json` | `chatgpt.com/backend-api/wham/usage` |
  | MiniMax (Coding/Token plan) | `MINIMAX_API_KEY` or config | `api.minimax.io/v1/token_plan/remains` |
  | Kimi For Coding | `~/.kimi/config.toml`, `KIMI_API_KEY`, or Kimi Code CLI creds | `api.kimi.com/coding/v1/usages` |
  | Anything else | user-defined `[[provider]]` in config | any JSON URL, dot-path field mapping |

- **Honest by design.** Every adapter declares a *fidelity*: `official`
  (the vendor's own endpoint), `derived` (reverse-engineered, may break), or
  `manual` (user config). The UI shows `user-configured source` for manual
  readings. Failures degrade to visible states (`needs auth`, `stale`);
  LimitCue never invents a percentage.
- **Rate-limit friendly.** Exponential backoff on errors and `429`s (capped at
  15 min), configurable healthy interval (default 120 s), and the last good
  reading is persisted to disk — a restart shows the old numbers with an age
  stamp instead of going blank.

## Install

Prebuilt binaries: see Releases (Linux x86_64, AppImage).

```sh
# build from source
cargo build --release
./target/release/limitcue

# or just try the fetcher without any GUI
limitcue --once
```

## Configuration

Written on first launch to `~/.config/limitcue/config.toml`:

```toml
poll_interval_secs = 120
hide_unconfigured = true
disabled = []            # e.g. ["codex"]

[[provider]]
id = "minimax"
name = "MiniMax"
base_url = "https://api.minimax.io"   # CN plans: https://api.minimaxi.com
key_env = "MINIMAX_API_KEY"
# api_key = "sk-cp-..."               # or inline (less safe — chmod 600)
```

### Bring your own provider

Any plan with a JSON usage endpoint works — no code needed:

```toml
[[provider]]
id = "glm"
name = "GLM Coding"
url = "https://api.z.ai/biz/v1/usage"
auth_header = "Authorization: Bearer {key}"
key_env = "Z_AI_API_KEY"
windows = [
  { label = "5h",     remaining_path = "data.remaining" },
  { label = "weekly", remaining_path = "data.week_remaining", resets_at_path = "data.week_reset" },
]
```

## Autostart

```sh
mkdir -p ~/.config/autostart
cp misc/limitcue.desktop ~/.config/autostart/
```

## Wayland note

Always-on-top and borderless work on KDE/GNOME. Programmatic window drag
honours the compositor's rules; if your compositor ignores it, you can still
move the window via its window-operations menu (usually Super+left-drag).

## Development

```sh
cargo run              # GUI
cargo run -- --once    # one-shot poll, print, exit
```

Adding a first-party adapter = one file in `src/providers/` implementing the
`Provider` trait + one registry line. Tests pinned to recorded fixtures are
welcome and encouraged.

## Privacy

- **Keys never leave your machine except to their own owner.** Each adapter is
  pinned to its provider's domain; your token is sent only in the request to
  that provider — the exact request the provider's own CLI makes.
- **No telemetry, no analytics, no update pings.** The only network traffic in
  the entire app is the usage fetch itself.
- **Local cache is numbers-only.** `state.json` stores percentages, counts and
  reset timestamps — never credentials or identity.
- **Provider config is mode `0600`** (user-only readable) since it may contain
  inline keys.

## License

MIT
