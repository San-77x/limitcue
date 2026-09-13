# Providers

A provider is whatever answers the question "how much of this plan is left?".
Every adapter declares a **fidelity** so the UI never presents a guess as more
certain than it is:

- **official** — the vendor's own endpoint.
- **derived** — reverse-engineered or inferred; it can break without notice.
- **manual** — a source you configured yourself.

## Built-in adapters

| Provider | Credential source | Endpoint | Fidelity |
|---|---|---|---|
| Claude (Max/Pro) | `~/.claude/.credentials.json` | Anthropic OAuth usage API (what `/usage` reads) | official |
| ChatGPT / Codex | `~/.codex/auth.json` | `chatgpt.com/backend-api/wham/usage` | official |
| Grok / xAI | `~/.grok/auth.json`, `XAI_API_KEY`, or Grok Build login | `cli-chat-proxy.grok.com/v1/billing` | derived |
| MiniMax (Coding/Token plan) | `MINIMAX_API_KEY` or config | `api.minimax.io/v1/token_plan/remains` | derived |
| Kimi For Coding | `~/.kimi/config.toml`, `KIMI_API_KEY`, or Kimi Code CLI credentials | `api.kimi.com/coding/v1/usages` | derived |
| New-API gateways (AgentRouter, …) | an `sk-` key in config | `…/v1/dashboard/billing/{subscription,usage}` | official |
| Anything else | user-defined `[[provider]]` | any JSON URL | manual |

Credentials are read from where the vendor's own CLI already put them. LimitCue
never signs you in and never sends a key anywhere except its owning provider's
domain. If an adapter has no real endpoint, it ships as `derived` — or it does
not ship.

## The `Provider` trait

An adapter implements four methods:

```rust
fn id(&self) -> &str;                 // stable key, also the state.json key
fn snapshot(&self) -> Snapshot;       // one live reading
fn is_present(&self) -> bool;         // is it usable on this machine?
fn fidelity(&self) -> Fidelity;       // official / derived / manual
```

A `Snapshot` carries a `Reading`, which is one of:

- `Ok { windows, detail }` — one `Window` per quota window (label, remaining
  percent or counts, reset time).
- `NeedsAuth(String)` — credentials exist but are expired or invalid.
- `NotConfigured` — nothing to read here; hidden unless you opt in.
- `Error(String)` — the fetch failed. The previous reading is kept and shown
  with an age stamp rather than blanked.

## Adding a first-party adapter

1. Add `src/providers/<name>.rs` implementing `Provider`.
2. Register it in `build_all` (and in `adapter_for` if it can be named by a
   `[[provider]]` entry's `adapter` key).
3. Pin the response shape with tests built from a recorded fixture. The
   adapters read endpoints we do not control, so those tests are the early
   warning system when a vendor changes something.
4. Document it in the table above, with its credential source and fidelity.

## Custom providers (no Rust required)

If an endpoint already reports usage as JSON, add it in
Settings → Providers → *Anything with a JSON endpoint* (or by hand):

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

The custom adapter tolerates the shapes real endpoints use — a percentage, a
fraction, n-of-total, used-of-total, or a balance against a known cap; numbers
arriving as strings; and timestamps as seconds, milliseconds, RFC3339, or a
countdown. A custom provider's fidelity is `manual`, because you chose the
mapping.
