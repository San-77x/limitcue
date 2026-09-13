import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Copies repo screenshots when the landing lives next to ../assets/
// (the LimitCue tree Rhea pushes). If that folder is absent — as on this
// isolated landing checkout — the page uses a dark-panel fallback until
// those assets are present.

const webRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');
const srcDir = path.join(webRoot, '..', 'assets', 'screenshots');
const destDir = path.join(webRoot, 'public', 'screenshots');
const names = ['pill.png', 'notch.png', 'expanded.png'];

if (!fs.existsSync(srcDir)) {
  console.warn(
    'assets/screenshots missing — Rhea: copy from the repo assets/screenshots tree on push; landing falls back to a dark panel until then',
  );
  process.exit(0);
}

fs.mkdirSync(destDir, { recursive: true });
let copied = 0;
for (const name of names) {
  const src = path.join(srcDir, name);
  if (!fs.existsSync(src)) continue;
  fs.copyFileSync(src, path.join(destDir, name));
  console.log(`copied assets/screenshots/${name} → public/screenshots/${name}`);
  copied += 1;
}

if (copied === 0) {
  console.warn(
    'assets/screenshots present but pill/notch/expanded.png not found — dark-panel fallback',
  );
}
