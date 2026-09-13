# LimitCue landing page

Static Next.js App Router landing for LimitCue (`/`). Locked Quill pack:
wordmark, hero, download / GitHub CTAs, one-line install strip, proof row,
how-it-works, providers, privacy, tiny FAQ, footer.

## Development

```sh
cd web
npm install
npm run dev
```

`predev` / `prebuild` copy `../assets/screenshots/{pill,notch,expanded}.png`
into `public/screenshots/` when that tree exists (the LimitCue repo layout
Rhea pushes). If those files are missing, the page shows a dark panel
instead of stock art — reuse one real shot in multiple places if only one
is present.

## Checks

```sh
npm run lint   # TypeScript check
npm run build  # Static production export
```

## Download CTA

At build time the page fetches the latest GitHub release and links the
primary button to `limitcue-linux-x86_64.tar.gz` when present. If the
asset is missing, the CTA becomes **Watch releases** (never a fake
download). Secondary link is always the repository. Works without JS.

Proof row is locked to the v0.1.1 tarball size: **~4.1 MB** (not ~6 / ~6.5).

No analytics, no signup, no new client libraries.
