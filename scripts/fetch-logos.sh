#!/usr/bin/env sh
# Fetch provider brand logos (simple-icons CDN, white fill) and rasterize
# them to 64px RGBA PNGs for include_bytes! embedding. One source per line:
#   <output-name>:<cdn-slug>
# Requires: curl, rsvg-convert. Run from anywhere; writes into assets/logos/.
set -eu
cd "$(dirname "$0")/.."
mkdir -p assets/logos

while IFS=: read -r name slug; do
  [ -z "$name" ] && continue
  curl -fsSL --max-time 20 "https://cdn.simpleicons.org/$slug/ffffff" -o "$name.svg" \
    || { echo "warn: $slug not on simple-icons, trying lobehub" >&2
         curl -fsSL --max-time 20 "https://cdn.jsdelivr.net/npm/@lobehub/icons-static-svg@latest/icons/$slug.svg" -o "$name.svg"
         sed -i 's/fill="currentColor"/fill="#ffffff"/' "$name.svg"; }
  rsvg-convert -w 64 -h 64 "$name.svg" -o "assets/logos/$name.png"
  echo "$name.png  ($slug)"
  rm -f "$name.svg"
done <<'EOF'
claude:claude
codex:openai
gemini:googlegemini
grok:grok
kimi:kimi
minimax:minimax
agentrouter:moonshotai
EOF
echo "done -> assets/logos/"
