# LimitCue landing page

A static Next.js landing page for LimitCue. It lives separately from the Rust desktop app and uses the same Inter typeface and provider logo assets.

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

The primary preview CTA currently points at the configured testing-build URL.
Update `RELEASES` in `app/page.tsx` when the private distribution URL is ready.
The page has no analytics, remote images, or runtime services.
