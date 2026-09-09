'use client';

import { useState } from 'react';

const RELEASES = 'https://github.com/San-77x/limitcue/releases';
const GITHUB = 'https://github.com/San-77x/limitcue';

const providers = [
  ['claude', 'Claude'], ['codex', 'Codex'], ['minimax', 'MiniMax'],
  ['kimi', 'Kimi'], ['gemini', 'Gemini'], ['agentrouter', 'AgentRouter'],
];

function Logo({ id }: { id: string }) {
  return <img className="provider-logo" src={`/providers/${id}.png`} alt="" />;
}

function Gauge({ id, used, muted = false }: { id: string; used: number; muted?: boolean }) {
  const color = used > 82 ? '#ff695f' : used > 58 ? '#ffae45' : '#74e4a3';
  return <div className={`gauge ${muted ? 'gauge-muted' : ''}`} style={{ '--used': `${used * 3.6}deg`, '--gauge': color } as React.CSSProperties}>
    <div className="gauge-inner"><Logo id={id} /></div>
  </div>;
}

function ProductMockup() {
  const [active, setActive] = useState('minimax');
  const rows = [
    { id: 'claude', used: 42, label: 'Claude', value: '42%' },
    { id: 'codex', used: 18, label: 'Codex', value: '18%' },
    { id: 'minimax', used: 76, label: 'MiniMax', value: '76%' },
    { id: 'kimi', used: 31, label: 'Kimi', value: '31%' },
  ];
  const current = rows.find((r) => r.id === active) ?? rows[2];
  return <div className="mockup-wrap" aria-label="LimitCue product preview">
    <div className="mockup-window">
      <div className="mockup-wallpaper"><span className="wall-line line-a" /><span className="wall-line line-b" /><span className="wall-line line-c" /></div>
      <div className="mockup-rail">
        {rows.map((row) => <button key={row.id} className={`mock-row ${active === row.id ? 'is-active' : ''}`} onMouseEnter={() => setActive(row.id)} aria-label={`Show ${row.label} usage`}>
          <Gauge id={row.id} used={row.used} muted={active !== row.id} />
          <span>{row.value}</span>
        </button>)}
        <div className="mock-menu"><i /><i /><i /></div>
      </div>
      <div className="mock-card">
        <div className="mock-card-head"><Logo id={current.id} /><div><strong>{current.label} usage</strong><small>official endpoint · updated just now</small></div><span className="head-dot" /></div>
        <div className="mock-summary"><b>{current.value} used</b><span>2 windows</span></div>
        <div className="mock-window-row"><div className="row-top"><span>5 hour window</span><em>Resets in 2h 14m</em></div><div className="bar"><i style={{ width: `${current.used}%`, background: 'var(--gauge)' }} /></div><div className="row-bottom"><b style={{ color: 'var(--gauge)' }}>{current.value} Used</b><span>24 / 100 calls</span></div></div>
        <div className="mock-window-row"><div className="row-top"><span>Weekly window</span><em>Resets Thu 12:00 AM</em></div><div className="bar"><i style={{ width: '34%', background: '#74e4a3' }} /></div><div className="row-bottom"><b style={{ color: '#74e4a3' }}>34% Used</b><span>updated 2m ago</span></div></div>
      </div>
    </div>
    <div className="mockup-caption"><span className="live-dot" /> Live usage, where you work <span>Hover a provider to inspect its windows</span></div>
  </div>;
}

function FeatureIcon({ type }: { type: string }) {
  return <div className={`feature-icon ${type}`} aria-hidden="true">{type === 'gauge' ? '◔' : type === 'timer' ? '◷' : type === 'shield' ? '◇' : '⌘'}</div>;
}

export default function Home() {
  return <main>
    <nav className="nav container"><a href="#top" className="brand"><span className="brand-mark"><i /><i /><i /></span><span>limitcue</span></a><div className="nav-links"><a href="#features">Features</a><a href="#how">How it works</a><a href={GITHUB}>GitHub <span>↗</span></a></div><a className="button button-small" href={RELEASES}>Download for Linux <span>↓</span></a></nav>

    <section className="hero container" id="top"><div className="hero-copy"><div className="eyebrow"><span className="eyebrow-line" /> NATIVE LINUX QUOTA MONITOR</div><h1>Know your limits<br /><em>before they interrupt</em> your flow.</h1><p className="hero-lede">One quiet rail for every AI coding plan you use. See what is left, when it resets, and whether the number is real — without opening another browser tab.</p><div className="hero-actions"><a className="button" href={RELEASES}>Download for Linux <span>↓</span></a><a className="text-link" href={GITHUB}>View source on GitHub <span>↗</span></a></div><div className="proof-row"><span>Rust + egui</span><span>Wayland + X11</span><span>No telemetry</span><span>MIT licensed</span></div></div><ProductMockup /></section>

    <section className="signal-section"><div className="container signal-grid"><div><span className="section-kicker">THE PROBLEM</span><h2>Quota is invisible<br />until it becomes a blocker.</h2></div><div className="signal-copy"><p>Usage is scattered across browser tabs, hidden behind provider dashboards, and easy to misread when a reset window is moving.</p><div className="signal-list"><div><b>01</b><span>Too many tabs to check before a long session.</span></div><div><b>02</b><span>Several windows, each with a different reset clock.</span></div><div><b>03</b><span>Numbers you cannot tell are official or stale.</span></div></div><div className="answer"><span>↳</span><p><strong>LimitCue makes the signal ambient.</strong><br />It stays visible, small, and honest.</p></div></div></div></section>

    <section className="features container" id="features"><div className="section-intro"><span className="section-kicker">THE RAIL</span><h2>Small enough to forget.<br /><em>Useful enough to miss.</em></h2></div><div className="feature-grid"><article><FeatureIcon type="gauge" /><span className="feature-number">01</span><h3>One rail. Every plan.</h3><p>Claude, Codex, MiniMax, Kimi, gateways, and custom JSON providers in one compact surface.</p></article><article><FeatureIcon type="timer" /><span className="feature-number">02</span><h3>Reset windows at a glance.</h3><p>See the session, weekly, and billing windows that actually shape your next hour.</p></article><article><FeatureIcon type="shield" /><span className="feature-number">03</span><h3>Honest by design.</h3><p>Official, derived, or manual. Fidelity is visible so a number never pretends to be more certain than it is.</p></article><article><FeatureIcon type="native" /><span className="feature-number">04</span><h3>Native, tiny, private.</h3><p>Rust and egui. No Electron, no webview, no telemetry, and a numbers-only local cache.</p></article></div></section>

    <section className="how-section" id="how"><div className="container"><div className="section-intro"><span className="section-kicker">HOW IT WORKS</span><h2>Three steps between<br /><em>you and a browser tab.</em></h2></div><div className="steps"><div className="step"><span>01</span><div className="step-line" /><h3>Install once.</h3><p>Build the single Linux binary or grab the latest release.</p></div><div className="step"><span>02</span><div className="step-line" /><h3>Keep your credentials.</h3><p>LimitCue reads the local credentials your provider CLI already uses.</p></div><div className="step"><span>03</span><div className="step-line" /><h3>Read the rail.</h3><p>Hover a provider when you need detail. Otherwise, keep coding.</p></div></div></div></section>

    <section className="providers container"><div><span className="section-kicker">WORKS WITH YOUR STACK</span><h2>One surface.<br /><em>Many providers.</em></h2></div><div className="provider-grid">{providers.map(([id, name]) => <div className="provider-tile" key={id}><Logo id={id} /><span>{name}</span></div>)}<div className="provider-tile custom"><span className="plus">＋</span><span>Custom JSON</span></div></div></section>

    <section className="privacy-section" id="privacy"><div className="container privacy-grid"><div><span className="section-kicker">THE QUIET PART</span><h2>Your keys stay<br /><em>with their providers.</em></h2></div><div><p className="privacy-lede">LimitCue does not create another account, proxy your requests, or collect a telemetry trail. It reads usage from the provider-owned sources already on your machine.</p><div className="privacy-points"><span>↳ No analytics</span><span>↳ No identity cache</span><span>↳ No surprise network calls</span></div></div></div></section>

    <section className="final-cta container"><span className="section-kicker">MAKE THE LIMIT VISIBLE</span><h2>Keep your context.<br /><em>Lose the quota anxiety.</em></h2><p>Download the native Linux monitor and get back to the work that used the quota in the first place.</p><a className="button" href={RELEASES}>Download for Linux <span>↓</span></a><small>Open source on GitHub · MIT licensed</small></section>

    <footer className="footer container"><a href="#top" className="brand"><span className="brand-mark"><i /><i /><i /></span><span>limitcue</span></a><span>Ambient quota visibility for Linux.</span><div><a href={GITHUB}>GitHub ↗</a><a href="#privacy">Privacy</a><a href="#top">Back to top ↑</a></div></footer>
  </main>;
}
