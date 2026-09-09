import type { Metadata } from 'next';
import './globals.css';

export const metadata: Metadata = {
  title: 'LimitCue — Know your limits before they interrupt your flow',
  description: 'A native Linux quota monitor for AI coding plans. See usage windows and reset times without opening another browser tab.',
  openGraph: {
    title: 'LimitCue — Native quota visibility for Linux',
    description: 'One quiet rail for every AI coding plan you use.',
    type: 'website',
  },
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>{children}</body></html>;
}
