# Third-party notices

LimitCue is MIT licensed (see [LICENSE](LICENSE)). It bundles the
following third-party assets, each under its own license.

## Fonts — `assets/fonts/`

- **Inter** by Rasmus Andersson — SIL Open Font License 1.1.
  Full license: [`assets/fonts/LICENSE-Inter.txt`](assets/fonts/LICENSE-Inter.txt).
  The three embedded weights are subsetted; the full originals are kept
  so the subsets can be regenerated with `scripts/subset-fonts.sh`.
- **Hack** by Christopher Simpkins / Source Foundry — MIT-style license.
  Full license: [`assets/fonts/LICENSE-Hack.txt`](assets/fonts/LICENSE-Hack.txt).
  Vendored from `epaint_default_fonts` 0.29.1.

## Icons — `assets/icons/`

- **Lucide** icons (ISC license) — <https://lucide.dev>. Pre-rasterized
  to 32 px white PNGs; see [`assets/icons/README.md`](assets/icons/README.md).

## Provider logos — `assets/logos/`

- Sourced from **Simple Icons** (<https://simpleicons.org>), whose
  packaging is CC0. The marks themselves remain trademarks of their
  respective owners and are used here only to identify each provider in
  the interface. See [`assets/logos/README.md`](assets/logos/README.md).

## Rust crates

The dependency set and its licenses are recorded in `Cargo.lock`. Run
`cargo license` (from `cargo-license`) for a generated inventory.
