#!/bin/sh
# Install the LimitCue KWin integration (keep-above + edge docking) for the
# current user on KDE Plasma (Wayland or X11). Re-run safely to update.

set -e

SRC="$(dirname "$0")/../misc/kwin/limitcue-integrate"
DST="$HOME/.local/share/kwin/scripts/limitcue-integrate"

mkdir -p "$(dirname "$DST")"
rm -rf "$DST"
cp -r "$SRC" "$DST"

# Enable the script in KWin; remove the superseded keep-above-only script.
kwriteconfig6 --file kwinrc --group Plugins --key limitcue-integrateEnabled --type bool true
kwriteconfig6 --file kwinrc --group Plugins --key limitcue-pinEnabled --type bool false 2>/dev/null || true
rm -rf "$HOME/.local/share/kwin/scripts/limitcue-pin"

# Reload KWin configuration so the script is enabled...
if command -v gdbus >/dev/null; then
    gdbus call --session --dest org.kde.KWin --object-path /KWin \
        --method org.kde.KWin.reconfigure 2>/dev/null || true

    # ...and force the code itself to be re-read. `reconfigure` only re-reads
    # settings: a script KWin has already loaded keeps running its old source
    # until it is unloaded, so updating this file otherwise does nothing until
    # the next login. That is worth knowing — an "installed" fix that silently
    # does not apply is worse than one that fails loudly.
    gdbus call --session --dest org.kde.KWin --object-path /Scripting \
        --method org.kde.kwin.Scripting.unloadScript "limitcue-integrate" >/dev/null 2>&1 || true
    gdbus call --session --dest org.kde.KWin --object-path /Scripting \
        --method org.kde.kwin.Scripting.loadScript \
        "$DST/contents/code/main.js" "limitcue-integrate" >/dev/null 2>&1 || true
    gdbus call --session --dest org.kde.KWin --object-path /Scripting \
        --method org.kde.kwin.Scripting.start >/dev/null 2>&1 || true
fi

echo "LimitCue KWin integration installed: $DST"
echo "The notch will now stay on top and snap to screen edges (drag it within ~32px of an edge)."
