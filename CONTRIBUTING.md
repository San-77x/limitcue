# Contributing to LimitCue

Thanks for taking the time to help. This project stays useful by staying
small, honest, and predictable — the guidelines below exist to keep it
that way.

## Before you start

- For anything larger than a bug fix, open an issue first so we can agree
  on the shape of the change before you write it.
- Security-sensitive reports do not belong in a public issue. See
  [SECURITY.md](SECURITY.md).
- By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

LimitCue is a Rust desktop app (eframe/egui). You need a stable Rust
toolchain and the usual X11/Wayland development headers.

```sh
# Debian/Ubuntu
sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libwayland-dev

cargo build            # debug build
cargo run              # run the GUI
cargo run -- --once    # one-shot poll, print, exit
```

The optional KWin integration is JavaScript that KWin runs, tested with
Node:

```sh
node scripts/test-kwin-script.js
```

## The checks CI runs

Please make sure these pass locally before opening a pull request:

```sh
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
node scripts/test-kwin-script.js
```

The project has held **zero warnings** throughout, and CI enforces it.
`cargo fmt` is *not* enforced yet — see the roadmap — so match the
surrounding style rather than reformatting files you are not otherwise
touching.

## Adding a provider adapter

A first-party adapter is one file in `src/providers/` implementing the
`Provider` trait, plus one line in the registry. Keep it to a single file
unless it genuinely needs more.

Two rules matter more than the code:

1. **Pin the response shape with tests.** Recorded fixtures are the early
   warning system when a vendor changes its API. A parser without tests is
   a future silent failure.
2. **Declare the right fidelity.** `official` means the vendor's own
   endpoint; `derived` means reverse-engineered and liable to break;
   `manual` means user-supplied config. Never present a derived reading as
   authoritative.

If an endpoint simply reports usage as JSON, prefer a catalogue entry for
the generic custom provider over a new Rust file.

## Commit messages

Write the subject as a plain sentence describing what the change does,
imperative mood, no type prefix required:

```
Keep the last good numbers when a refresh fails
```

Add a body when the *why* is not obvious from the subject. One logical
change per commit is preferred but not enforced.

## Pull requests

- Keep the diff focused; unrelated cleanups belong in their own PR.
- Say what you changed and why, and how you verified it.
- If you touched UI, a screenshot or a short clip helps a lot.
- Update the README or `docs/` when behaviour or configuration changes.

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).
