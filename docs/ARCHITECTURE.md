# Architecture

LimitCue is a single-process Rust application. One egui window draws the UI,
one background thread polls providers, and a couple of small threads serve
local readers. There is no runtime, no webview, and no server.

## Modules

| Module | Responsibility |
|---|---|
| `src/main.rs` | Application state and rendering. Owns the poll channel, the expand/collapse animation, card placement, and the settings sheet. |
| `src/types.rs` | The shared vocabulary: `Snapshot`, `Window`, `Reading`, `Fidelity`, and the reset-time formatting. |
| `src/providers/` | The `Provider` trait and its implementations. See [PROVIDERS.md](PROVIDERS.md). |
| `src/config.rs` | `config.toml` parsing, defaults, atomic save at mode `0600`, and hot-reload detection. |
| `src/history.rs` | Burn-rate samples (`history.json`), percentages and timestamps only, used for the projection line. |
| `src/notify.rs` | Low-quota and refill alerts, latched per window, with command hooks. |
| `src/activity.rs` | Session-activity detection from CLI log modification times. Contents are never opened. |
| `src/dock.rs` | Edge-docking state and the KWin script handshake over D-Bus (`io.limitcue.Dock`). |
| `src/service.rs` | Read interfaces for other programs: `io.limitcue.Usage` on D-Bus (a `Get`/`Refresh` pair plus a `Changed` signal per new reading) and `$XDG_RUNTIME_DIR/limitcue.sock`. |
| `src/cli.rs` | Headless output: `--once`, `--json`, `--line`, `--waybar`, `--watch`, `wait`, `init`. |
| `src/ui/` | Theme palettes (`theme.rs`), drawing primitives (`widgets.rs`), and composed surfaces (`mod.rs`). |

## Data flow

```
                  build_all(&cfg)                 merge last-good
config.toml ──▶  Vec<Box<dyn Provider>> ──▶ polls ──▶ Vec<Snapshot>
                                                            │
              ┌─────────────────────────────────────────────┤
              ▼                                             ▼
      mpsc channel → UI thread                    state.json (numbers only)
              │                                             │
              ▼                                             ▼
        egui window                        io.limitcue.Usage (D-Bus) + socket
```

1. `spawn_poller` builds the provider list from the config and seeds its
   last-good map from `state.json`, so a failure in the very first poll still
   has a number to show.
2. Each provider is polled on its own schedule. A failed refresh does not
   discard the previous reading — `merge_reading` keeps it and marks the
   attempt as failed.
3. The full snapshot set is published to the UI over an `mpsc` channel, to
   readers through `SharedUsage`, and the good readings are written to
   `state.json`.
4. The UI never fetches. It renders whatever the poller last published and
   repaints on a timer that is cheap when collapsed and one second when
   expanded (so countdowns tick).

## Threads

- **Main** — egui event loop and rendering.
- **Poller** — one thread for all providers, restarted when the config
  changes; each provider backs off independently on failure.
- **Activity** — reads session-log modification times on an interval.
- **D-Bus / socket** — `zbus` connection and a unix-socket listener, both
  read-only apart from an explicit refresh request.

## Local state

| File | Contents |
|---|---|
| `~/.config/limitcue/config.toml` | Settings and provider definitions, written `0600`. |
| `~/.local/share/limitcue/state.json` | Last good reading per provider — percentages, counts, reset times. |
| `~/.local/share/limitcue/history.json` | Percentages against timestamps for the burn-rate projection. |
| `~/.local/share/limitcue/dock.json` | Docked edge and position reported by the KWin script. |

None of these files hold credentials.

## The `Provider` trait

An adapter answers four questions: its `id`, its current `snapshot()`, whether
it is `is_present()` on this machine, and what `fidelity()` its numbers carry.
The trait is deliberately small so a first-party adapter stays one file. See
[PROVIDERS.md](PROVIDERS.md) for the contract and how to add one.
