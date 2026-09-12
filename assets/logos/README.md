Provider brand logos, pre-rasterized 64px white RGBA PNGs
(loaded as textures at startup; `logo_ring` draws them on the brand disc).

  claude.png       Claude (Anthropic)
  codex.png        OpenAI / Codex
  gemini.png       Google Gemini
  grok.png         Grok (xAI)
  kimi.png         Kimi (Moonshot)
  minimax.png      MiniMax
  agentrouter.png  AgentRouter (Moonshot logo)

Source: https://simpleicons.org — CDN serves single-path 24×24 SVGs with the
brand color replaced by the requested fill; regen with `scripts/fetch-logos.sh`
(rsvg-convert rasterization, same pipeline as assets/icons). The codex logo
is simple-icons' `openai`; agentrouter uses Moonshot's mark. Simple Icons
packaging is CC0; logos remain trademarks of their owners, used for
identification in this personal tool.

Unknown providers have no logo: the UI falls back to the monogram letter.
