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

The primary download CTA currently points at the GitHub Releases page. Update
`RELEASES` in `app/page.tsx` when a specific release artifact URL is available.
The page has no analytics, remote images, or runtime services.
