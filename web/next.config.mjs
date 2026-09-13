/** @type {import('next').NextConfig} */
// When deployed to a GitHub Pages project site the app is served from
// a subpath rather than the domain root, so the Pages workflow sets
// NEXT_PUBLIC_BASE_PATH. Local development leaves it empty.
const basePath = process.env.NEXT_PUBLIC_BASE_PATH || '';

const nextConfig = {
  output: 'export',
  images: { unoptimized: true },
  basePath: basePath || undefined,
  trailingSlash: true,
};

export default nextConfig;
