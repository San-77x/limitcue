# Security Policy

LimitCue runs with your credentials and reads files that belong to other
tools, so security reports are taken seriously and handled privately.

## Reporting a vulnerability

Please **do not open a public issue** for a security problem. Use GitHub's
private reporting instead:

1. Go to the repository's **Security** tab.
2. Choose **Report a vulnerability**.

If private reporting is unavailable to you, open a minimal issue that
asks for a private channel, without any details of the vulnerability.

Please include, where you can:

- what the issue is and the impact you believe it has,
- the version or commit you tested,
- the smallest reproduction you have, and
- any suggested fix.

You can expect an acknowledgement within a few days and a coordinated
disclosure once a fix is available. Please give us a reasonable window
to release a fix before publishing details.

## Scope

Areas that are especially relevant:

- credential handling in `src/providers/` and how keys are read,
- the local D-Bus service (`io.limitcue.Usage`, `io.limitcue.Dock`) and
  the `$XDG_RUNTIME_DIR/limitcue.sock` socket,
- file permissions on `config.toml`, `state.json`, `history.json`, and
  `dock.json`,
- anything that could send a credential somewhere other than its owning
  provider.

## The security model

The intended guarantees, which a report may show to be violated:

- **Credentials are sent only to their owning provider.** Every adapter
  is pinned to that provider's domain.
- **The local surface is user-only.** The Unix socket is created `0600`;
  the D-Bus interfaces are session-local and read-only apart from an
  explicit refresh request; the config file is written `0600`.
- **Local state is numbers-only.** `state.json` and `history.json` hold
  percentages, counts and reset timestamps — never credentials or
  identity.
- **No telemetry.** The only network traffic is the usage fetch itself.

## Supported versions

LimitCue is pre-1.0 and ships from `main`. Fixes are made on `main`; there
are no maintained release branches yet.
