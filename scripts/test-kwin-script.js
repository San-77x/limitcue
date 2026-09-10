#!/usr/bin/env node
// Regression tests for the geometry helpers in the KWin integration.
//
// These cannot be covered from Rust — the logic lives in a script KWin runs —
// and they are the arithmetic behind the bug where a right-docked notch hopped
// to the next monitor when its hover card opened. Run: node scripts/test-kwin-script.js

const fs = require('fs');
const path = require('path');

// Stub the compositor globals the script touches at load time.
const workspace = {
    windowAdded: { connect() {} },
    windowList: () => [],
    virtualScreenGeometry: { x: 0, y: 0, width: 7808, height: 2160 },
};
const callDBus = () => {};
const print = () => {};

const src = fs.readFileSync(
    path.join(__dirname, '../misc/kwin/limitcue-integrate/contents/code/main.js'),
    'utf8'
);
// eslint-disable-next-line no-eval
eval(src); // brings flushGeometry / isFlush / nearestEdge into this scope

let failures = 0;
function check(name, got, want) {
    const ok = JSON.stringify(got) === JSON.stringify(want);
    if (!ok) {
        failures++;
        console.log(`FAIL ${name}\n  got  ${JSON.stringify(got)}\n  want ${JSON.stringify(want)}`);
    } else {
        console.log(`ok   ${name}`);
    }
}

// The desktop this was reported on: HDMI, then the laptop immediately right
// of it, then a third screen.
const HDMI = { x: 0, y: 0, width: 3840, height: 2160 };
const LAPTOP = { x: 3840, y: 0, width: 2048, height: 1280 };
const STRIP = 48;
const OPEN = 322; // strip + gap + card

const win = (x, width) => ({ frameGeometry: { x, y: 400, width, height: 405 } });

// --- resting -----------------------------------------------------------
check('a notch flush right of HDMI reads as right',
    nearestEdge(win(3840 - STRIP, STRIP), HDMI), 4);
check('...and is already flush, so nothing moves',
    isFlush(win(3840 - STRIP, STRIP), 4, HDMI), true);

// --- the bug: card opens, window widens --------------------------------
const opened = win(3840 - STRIP, OPEN); // grew rightwards from the same x
check('a widened window is no longer flush',
    isFlush(opened, 4, HDMI), false);
check('clamped against the DOCKED output it grows leftwards and stays on HDMI',
    flushGeometry(opened, 4, HDMI).x, 3840 - OPEN);

// This is what used to happen: the widened window straddles the boundary,
// KWin reassigns it to the laptop, and measuring against *that* output makes
// it look left-docked and snaps it onto the far screen.
check('measured against the neighbour it would look left-docked',
    nearestEdge(opened, LAPTOP), 3);
check('...and would be snapped onto the neighbour (the reported jump)',
    flushGeometry(opened, 3, LAPTOP).x, 3840);

// --- left dock stays put too -------------------------------------------
check('a notch flush left of the laptop reads as left',
    nearestEdge(win(3840, STRIP), LAPTOP), 3);
check('opening a card there grows rightwards, still on the laptop',
    flushGeometry(win(3840, OPEN), 3, LAPTOP).x, 3840);

console.log(failures === 0 ? '\nall passed' : `\n${failures} failed`);
process.exit(failures === 0 ? 0 : 1);
