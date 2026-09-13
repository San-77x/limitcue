import type { Metadata } from 'next';
import './globals.css';

// Served from /<repo>/ on a GitHub Pages project site; empty in development.
const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? '';

// Inlined rather than kept in globals.css so the font URLs carry the same
// base path as the rest of the export.
const fontFaces = `
@font-face{font-family:Inter;src:url('${basePath}/fonts/Inter-Regular.ttf')}
@font-face{font-family:Inter;src:url('${basePath}/fonts/Inter-Medium.ttf');font-weight:500}
@font-face{font-family:Inter;src:url('${basePath}/fonts/Inter-SemiBold.ttf');font-weight:650}
`;

export const metadata: Metadata = {
  title: 'LimitCue — Know your limits before they interrupt your flow',
  description: 'A native Linux quota monitor for AI coding plans. See usage windows and reset times without opening another browser tab.',
  openGraph: {
    title: 'LimitCue — Know your limits before they interrupt your flow',
    description: 'A quiet, open-source Linux quota monitor for AI coding plans.',
    type: 'website',
  },
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <style dangerouslySetInnerHTML={{ __html: fontFaces }} />
      </head>
      <body>{children}</body>
    </html>
  );
}
