import type { Metadata } from 'next';
import './globals.css';

const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? '';

const fontFaces = `
@font-face{font-family:Inter;src:url('${basePath}/fonts/Inter-Regular.ttf')}
@font-face{font-family:Inter;src:url('${basePath}/fonts/Inter-Medium.ttf');font-weight:500}
@font-face{font-family:Inter;src:url('${basePath}/fonts/Inter-SemiBold.ttf');font-weight:650}
`;

export const metadata: Metadata = {
  title: 'LimitCue — Always-on-top quota pill for Linux',
  description:
    'See how much is left on every AI coding plan you’re signed into — no browser, no fake numbers.',
  openGraph: {
    title: 'LimitCue — Always-on-top quota pill for Linux',
    description:
      'See how much is left on every AI coding plan you’re signed into — no browser, no fake numbers.',
    type: 'website',
  },
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <style dangerouslySetInnerHTML={{ __html: fontFaces }} />
      </head>
      <body>{children}</body>
    </html>
  );
}
