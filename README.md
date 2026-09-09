# LimitCue

A tiny always-on-top quota pill for your Linux desktop. See how much usage is
left on every AI coding plan you're signed into — without opening a browser,
a terminal, or trusting a made-up number.

```
 ◔ CODEX 63%  ◑ CLAUDE 22%  ◕ M3 69%        ← collapsed: ring gauges
```

Click it and it eases open into a detail card per provider — labelled
quota bars, counts, and live reset countdowns — with smooth scrolling
when you have more plans than fit on screen.

Built with Rust + egui as a single ~5 MB binary — no Electron, no webview, no runtime.

## What it does

- **One pill, many plans.** A compact heat-colored ring gauge per provider —
  green while plenty remains, warming through yellow and orange to red as the
  quota is used — with the percentage in tabular digits. More providers than
  `max_visible_collapsed` (default 4)? The overflow folds into a `+N` chip;
  expanding lists everything in a content-sized detail area that only becomes
  scrollable when it reaches the available screen height. Stale readings fade
  toward grey instead of pretending to be fresh.
- **Drag it anywhere** — grab it anywhere (compositor-native grab;
  Wayland + X11). No grip icon needed: press-and-drag the pill itself, or
  the notch when it's docked to a side. Borderless, always-on-top, eased
  expand/collapse animation.
  Hover any ring for per-window detail; `R` refreshes, `Esc` minimizes.
  On KDE Plasma (Wayland or X11), drag it near a screen edge and it **docks
  flush** — zero gap, square corners on the attached edge — and the detail
  card grows *away* from the edge. The docked position survives restarts.

### The vertical rail (left/right dock)

Docked left or right, the pill becomes a black notch welded to the screen
edge: a pure-`#000` strip, square where it meets the bezel and rounded on
the card side. Each tracked provider gets its own cell — a clean **gauge**: the
provider's white mark (real logo for Claude, Codex/OpenAI, Gemini, Kimi,
MiniMax and AgentRouter; a monogram letter otherwise) framed by a track
ring and a heat-colored arc showing the share *used* — green while
plenty remains, warming through yellow and orange to red as the window
empties, with the used percentage in white underneath. A settings orb sits
at the bottom (arc at rest, gear on hover). The whole notch is draggable:
press and drag anywhere on it (left or middle button) and the compositor
moves the window; on release it snaps back flush to whichever side you
dropped it nearest. Hovering a cell slides out a glass usage card that
answers three questions in reading order:

1. **How much is gone** — a headline `100% used` in the heat colour, with
   the window it belongs to and its reset time beside it ("Resets in 51
   min", "Resets Sat 04:00 AM"). One glance is enough; nothing below the
   headline is required reading.
2. **Where it went** — one row per quota window: label, share used, a 4 px
   heat meter, and the reset/count line under it. The window closest to
   exhaustion is tinted rather than tagged. A provider with a *single*
   window skips the list entirely and shows just its meter — a one-row
   breakdown of a headline that already said the same thing is noise.
3. **How fresh the number is** — the footer carries the reading's age on the
   left and the provider's own note (a dollar balance, a window count) on the
   right; provenance (official / derived / manual, or *stale*) sits in the
   header next to the name.

The panel is glass: a translucent body, a hairline edge, a specular lip along
the top and a soft drop shadow. Long window lists grow with the available
monitor space and scroll only when the screen cannot fit them. The card takes
the side of the hovered notch row with more room, flipping above the row when
the lower side is too short, and is strictly hover-bound: it appears the
moment you point at a cell and fades as soon as you leave the notch.
- **Four dark themes** — midnight, tokyo-night, catppuccin, gruvbox — via the
  `theme` config key. Each theme carries its own gauge heat scale; expanded
  cards show each reading's *fidelity* badge (official / derived / manual)
  so trust is always visible.
- **Borrows existing credentials.** It never signs you in anywhere and never
  sends your keys anywhere but the owning provider's API:

  | Provider | Source it reads | Endpoint |
  |---|---|---|
  | Claude (Max/Pro) | `~/.claude/.credentials.json` | Anthropic OAuth usage API (what `/usage` uses) |
  | ChatGPT / Codex | `~/.codex/auth.json` | `chatgpt.com/backend-api/wham/usage` |
  | MiniMax (Coding/Token plan) | `MINIMAX_API_KEY` or config | `api.minimax.io/v1/token_plan/remains` |
  | Kimi For Coding | `~/.kimi/config.toml`, `KIMI_API_KEY`, or Kimi Code CLI creds | `api.kimi.com/coding/v1/usages` |
  | New-API gateways (AgentRouter etc.) | `sk-` key in config | `…/v1/dashboard/billing/{subscription,usage}` |
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

Written on first launch to `~/.config/limitcue/config.toml` (top-level keys
must appear **before** any `[[provider]]` section — in TOML, keys after a
table header belong to that table):

```toml
poll_interval_secs = 120
hide_unconfigured = true
disabled = []            # e.g. ["codex"]
theme = "midnight"       # midnight | tokyo-night | catppuccin | gruvbox
max_visible_collapsed = 4  # providers on the pill before folding into "+N"
quiet_mode = false         # dim idle rail/pill surfaces until hover
show_rail_percent = false  # show used percentages beneath notch gauges

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

## Edge docking (KDE Plasma)

On Wayland an app cannot move or even know its own window position, so
docking is done compositor-side. LimitCue ships a small KWin script
(`misc/kwin/limitcue-integrate/`) that:

- keeps the pill above other windows (replaces the older `limitcue-pin`
  script — uninstall that one if you have it),
- snaps the pill flush to the nearest screen edge when you drop a drag within
  ~32 px of it (top/bottom/left/right),
- re-clamps it to the edge whenever the pill resizes itself (expand/collapse),
  and
- reports the docked position to the app over D-Bus (`io.limitcue.Dock`), which
  persists it to `~/.local/share/limitcue/dock.json` and restores it on the
  next launch.

Install it with:

```sh
scripts/install-kwin.sh
```

The app itself works everywhere (GNOME, X11, other compositors) — without
KWin you just don't get snap-to-edge and position persistence, and
always-on-top falls back to whatever the compositor allows. `--once` mode
never touches D-Bus or files.

## Settings

The gear icon on the pill — or the orb at the foot of the notch — opens the
settings sheet.

The sheet is three fixed bands — title, tabs, action bar — around one
scrolling body, so *Save* and *Cancel* stay reachable however many providers
you have. Every setting is a row with a plain-language description under its
title; the controls are hand-painted switches, sliders and swatches rather
than stock widgets. Three tabs:

- **General** — poll interval (30–900 s; failures back off automatically),
  what the notch shows at rest (percentage labels, quiet mode,
  hide-unconfigured), and a shortcut to open `config.toml`.
- **Providers** — enable/disable, reorder, remove, and add new ones, each row
  carrying the provider's own mark. API keys are never typed here: adding an
  id creates the entry, and `api_key` / `key_env` go in `config.toml`.
  Built-in adapters (Claude, Codex) can be toggled off.
- **Appearance** — the theme, picked from swatches that show each palette's
  own background and heat scale (and previewed live while the sheet is open),
  plus the collapsed provider count for the undocked pill.

*Save* writes `config.toml` and hot-reloads the poll loop — no restart
needed. Providers can also carry a `priority = <n>` key in config.toml
(lower = earlier in the pill; file order otherwise).

## Development

```sh
cargo run              # GUI
cargo run -- --once    # one-shot poll, print, exit
```

Adding a first-party adapter = one file in `src/providers/` implementing the
`Provider` trait + one registry line. Tests pinned to recorded fixtures are
welcome and encouraged.

UI text is set in [Inter](https://rsms.me/inter/) (SIL OFL; three weights
embedded from `assets/fonts/`, ~1 MB added to the binary) with tabular Hack
mono kept for numerals in the pill. Debug hooks for screenshot automation:
`LIMITCUE_UI_SHOT=/path.png` captures the window after the layout settles
and exits, `LIMITCUE_UI_RAIL=<provider_id>` pins a usage card open,
`LIMITCUE_UI_SNAP=1` skips size animations.

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

## Project status

See [STATUS.md](STATUS.md) for what's done, in progress, and known limitations,
and [docs/v2.md](docs/v2.md) for the feature backlog.

## License

MIT
