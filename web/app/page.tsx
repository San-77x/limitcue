import fs from 'node:fs';
import path from 'node:path';

const REPO = 'https://github.com/San-77x/limitcue';
const RELEASES = `${REPO}/releases`;
const BASE_PATH = process.env.NEXT_PUBLIC_BASE_PATH ?? '';
const LINUX_ASSET = 'limitcue-linux-x86_64.tar.gz';
const DEFAULT_TAG = 'v0.1.1';

type Download =
  | { kind: 'asset'; url: string; tag: string }
  | { kind: 'watch'; url: string; tag: string };

async function resolveLinuxDownload(): Promise<Download> {
  try {
    const res = await fetch(
      'https://api.github.com/repos/San-77x/limitcue/releases/latest',
      {
        headers: {
          Accept: 'application/vnd.github+json',
          'User-Agent': 'limitcue-web',
        },
        cache: 'force-cache',
      },
    );
    if (!res.ok) return { kind: 'watch', url: RELEASES, tag: DEFAULT_TAG };
    const data = (await res.json()) as {
      tag_name?: string;
      assets?: { name: string; browser_download_url: string }[];
    };
    const tag = data.tag_name || DEFAULT_TAG;
    const asset = data.assets?.find((a) => a.name === LINUX_ASSET);
    if (!asset?.browser_download_url) {
      return { kind: 'watch', url: RELEASES, tag };
    }
    return { kind: 'asset', url: asset.browser_download_url, tag };
  } catch {
    return { kind: 'watch', url: RELEASES, tag: DEFAULT_TAG };
  }
}

const SHOT_NAMES = ['pill', 'notch', 'expanded'] as const;
type ShotName = (typeof SHOT_NAMES)[number];

function availableShots(): Partial<Record<ShotName, string>> {
  const dir = path.join(process.cwd(), 'public', 'screenshots');
  const found: Partial<Record<ShotName, string>> = {};
  for (const name of SHOT_NAMES) {
    try {
      if (fs.existsSync(path.join(dir, `${name}.png`))) {
        found[name] = `${BASE_PATH}/screenshots/${name}.png`;
      }
    } catch {
      /* missing public tree is fine — dark panel fallback */
    }
  }
  return found;
}

function pickShot(
  found: Partial<Record<ShotName, string>>,
  prefer: ShotName[],
): string | undefined {
  for (const name of prefer) {
    if (found[name]) return found[name];
  }
  return Object.values(found)[0];
}

function Shot({
  src,
  alt,
  className,
}: {
  src?: string;
  alt: string;
  className?: string;
}) {
  if (src) {
    return (
      // eslint-disable-next-line @next/next/no-img-element
      <img className={className ?? 'shot'} src={src} alt={alt} />
    );
  }
  return (
    <figure className="shot-fallback">
      <div className="shot-fallback-panel" aria-hidden="true" />
      <figcaption>Preview</figcaption>
    </figure>
  );
}

const HOW = [
  'Borrows credentials your CLIs already wrote — LimitCue never signs you in.',
  'Polls each provider’s own usage endpoint and shows honest remaining quota.',
  'Stays on top as a pill or edge-docked notch; expand for bars, resets, and freshness.',
] as const;

const PROVIDERS = [
  'Claude (Max/Pro)',
  'ChatGPT/Codex',
  'Grok/xAI',
  'MiniMax',
  'Kimi For Coding',
  'New-API gateways (e.g. AgentRouter)',
] as const;

const PRIVACY = [
  'Keys go only to the provider API.',
  'No telemetry.',
  'Local cache stores numbers and timestamps only.',
] as const;

const FAQ = [
  {
    q: 'Invent percentages?',
    a: 'No — fidelity shown; needs-auth/stale.',
  },
  {
    q: 'macOS/Windows?',
    a: 'Linux only (x86_64) for now.',
  },
  {
    q: 'License?',
    a: 'MIT · Aerium Studio',
  },
] as const;

export default async function Home() {
  const download = await resolveLinuxDownload();
  const shots = availableShots();
  const heroShot = pickShot(shots, ['pill', 'notch', 'expanded']);
  const detailShot = pickShot(shots, ['expanded', 'notch', 'pill']);
  const primaryHref = download.url;
  const primaryLabel =
    download.kind === 'asset'
      ? `Download Linux x86_64 tarball ${download.tag}`
      : 'Watch releases';

  return (
    <main>
      <header className="site-header container">
        <a className="wordmark" href="#top" id="top">
          LimitCue
        </a>
        <p className="studio-credit">
          by <span>Aerium Studio</span>
        </p>
      </header>

      <section className="hero container">
        <div className="hero-copy">
          <h1>Always-on-top quota pill for Linux.</h1>
          <p className="hero-sub">
            See how much is left on every AI coding plan you&apos;re signed into
            — no browser, no fake numbers.
          </p>
          <div className="hero-actions">
            <a className="button" href={primaryHref}>
              {primaryLabel}
            </a>
            <a className="text-link" href={REPO}>
              View on GitHub
            </a>
          </div>
        </div>
        <div className="hero-visual">
          <Shot
            src={heroShot}
            alt="LimitCue quota pill on the Linux desktop"
            className="shot shot-hero"
          />
        </div>
      </section>

      <section className="install container" aria-label="Install">
        <ol className="install-strip">
          <li>
            <span className="step-n">1</span>
            Download the Linux x86_64 tarball ({download.tag})
          </li>
          <li>
            <span className="step-n">2</span>
            Extract and run ./limitcue
          </li>
          <li>
            <span className="step-n">3</span>
            Optional: limitcue init
          </li>
        </ol>
      </section>

      <p className="proof-row container">
        ~4.1 MB · Rust · no Electron · no telemetry · MIT · Linux
      </p>

      <section className="how container">
        <h2>How it works</h2>
        <div className="how-grid">
          <ol className="how-list">
            {HOW.map((text) => (
              <li key={text}>{text}</li>
            ))}
          </ol>
          <div className="how-visual">
            <Shot
              src={detailShot}
              alt="LimitCue expanded usage bars, resets, and freshness"
              className="shot shot-detail"
            />
          </div>
        </div>
      </section>

      <section className="providers container">
        <h2>Providers</h2>
        <ul className="provider-chips">
          {PROVIDERS.map((name) => (
            <li key={name}>{name}</li>
          ))}
        </ul>
        <p className="provider-else">
          Anything else: <code>[[provider]]</code> JSON URL.
        </p>
      </section>

      <section className="privacy container">
        <h2>Privacy</h2>
        <ul className="privacy-list">
          {PRIVACY.map((text) => (
            <li key={text}>{text}</li>
          ))}
        </ul>
      </section>

      <section className="faq container">
        <h2>FAQ</h2>
        <dl className="faq-list">
          {FAQ.map((item) => (
            <div className="faq-item" key={item.q}>
              <dt>{item.q}</dt>
              <dd>{item.a}</dd>
            </div>
          ))}
        </dl>
      </section>

      <footer className="site-footer container">
        <p className="footer-line">MIT · Aerium Studio</p>
        <a className="text-link" href={REPO}>
          GitHub
        </a>
      </footer>
    </main>
  );
}
