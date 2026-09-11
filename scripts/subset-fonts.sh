#!/usr/bin/env bash
# Regenerate the subsetted faces that get compiled into the binary.
#
# The full originals stay in assets/fonts/ so this stays repeatable; only the
# *-sub.ttf files are include_bytes!'d by src/ui/theme.rs. Run this after
# replacing an original, and commit both halves.
#
# Needs fonttools:  pip install fonttools  (or: pacman -S python-fonttools)
set -euo pipefail

cd "$(dirname "$0")/.."
fonts=assets/fonts

# Everything the UI can draw: ASCII, Latin-1 and Latin Extended-A for accented
# provider names, plus the handful of punctuation marks the layout uses --
# · — … ‹ › → ↗ and the quotes. Deliberately the same range for the mono face:
# a narrower one there would mean re-deciding which family draws which glyph
# every time the UI changes, to save about 20 KB.
range='U+0020-007E,U+00A0-017F,U+00B7,U+2013,U+2014,U+2018-201D,U+2022,U+2026,U+2039,U+203A,U+2192,U+2197,U+00A3,U+20AC'

for face in Inter-Regular Inter-Medium Inter-SemiBold Hack-Regular; do
  src="$fonts/$face.ttf"
  out="$fonts/$face-sub.ttf"
  [ -f "$src" ] || { echo "missing $src" >&2; exit 1; }
  pyftsubset "$src" \
    --output-file="$out" \
    --unicodes="$range" \
    --layout-features='kern,liga,calt,tnum,ccmp,locl,mark,mkmk' \
    --no-hinting \
    --desubroutinize \
    --drop-tables+=DSIG
  printf '%-18s %7d -> %6d bytes\n' "$face" "$(stat -c%s "$src")" "$(stat -c%s "$out")"
done
