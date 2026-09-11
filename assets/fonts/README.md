Inter (https://rsms.me/inter/), SIL Open Font License 1.1 — see
LICENSE-Inter.txt. Three static weights embedded via include_bytes! and
registered in src/ui/theme.rs (install_fonts): Regular as the default
proportional face, Medium and SemiBold as named families (theme::medium /
theme::semibold).

Hack (https://sourcefoundry.org/hack/), MIT-style licence — see
LICENSE-Hack.txt. Vendored from epaint_default_fonts 0.29.1, which we no
longer pull in: eframe's `default_fonts` feature is off, since three of the
four faces it bundles (NotoEmoji, emoji-icon-font, Ubuntu-Light) never drew a
glyph here. Hack backs FontFamily::Monospace and every theme::mono call, where
its tabular digits keep the card's numeric columns aligned.

Only the *-sub.ttf files are compiled in. The full originals stay here so the
subsets can be regenerated: run scripts/subset-fonts.sh after replacing one,
and commit both halves. The subset covers ASCII, Latin-1, Latin Extended-A and
the punctuation the layout draws (· — – … ‹ › → ↗ • quotes £ €) -- 185 KB in
total instead of 1.5 MB.
