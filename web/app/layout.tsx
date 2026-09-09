import type { Metadata } from 'next';
import './globals.css';

export const metadata: Metadata = {
  title: 'LimitCue — Know your limits before they interrupt your flow',
  description: 'A native Linux quota monitor for AI coding plans. See usage windows and reset times without opening another browser tab.',
  openGraph: {
    title: 'LimitCue — Private Linux quota monitor preview',
    description: 'A private testing preview of a quiet Linux quota monitor for AI coding plans.',
    type: 'website',
  },
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>{children}</body></html>;
}
