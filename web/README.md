# LimitCue landing page

A static Next.js landing page for LimitCue. It lives separately from the
Rust desktop app and uses the same Inter typeface and provider logo assets.

## Development

```sh
cd web
npm install
npm run dev
```

Open `http://localhost:3000`.

## Checks

```sh
npm run lint   # TypeScript check
npm run build  # Static production export
```

The download CTA points at the repository's public GitHub Releases page.
Update `RELEASES` in `app/page.tsx` if that ever moves.

The page has no analytics, remote images, or runtime services.
