//! Theme system: semantic palettes + global egui styling.
//!
//! Every color in the UI comes from a [`Palette`]; nothing else hard-codes
//! colors. All themes are dark and share the same semantic roles.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke,
};

/// Semantic palette for one theme.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub border: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub bad: Color32,
    /// Grey for stale/error readings — deliberately non-alarming.
    pub stale: Color32,
    /// Unfilled track behind rings and bars.
    pub track: Color32,
    pub accent: Color32,
    /// Translucent fill of the docked side-rail body.
    pub rail_bg: Color32,
    /// Disc fill behind a provider logo on the rail.
    pub rail_disc: Color32,
    /// Translucent glass surface for the rail card.
    pub rail_deep: Color32,
    /// Colour of anything drawn directly *on* the notch or card surface:
    /// provider marks, the percentage under a gauge, the settings orb.
    ///
    /// These used to be hardcoded white, which quietly required every theme
    /// to have a dark surface. Naming the colour is what lets a theme be
    /// light, or bright, or green.
    pub ink: Color32,
    /// Hairline rule drawn *inside* a glass surface (already translucent).
    pub hairline: Color32,
    /// Specular highlight along the top lip of a glass surface.
    pub sheen: Color32,
    /// Raised control surface inside the settings sheet (rows, inputs, wells).
    pub control: Color32,
    /// Same, one step brighter — hover / active segment.
    pub control_hi: Color32,
    /// Gauge heat scale keyed on percent *used*: calm → caution → hot → out.
    pub gauge: [Color32; 4],
    /// How much the gauges bloom, 0..1. A wide, faint copy of each arc and
    /// bar painted underneath it. This is what separates a flat palette from
    /// one that looks lit from behind.
    pub glow: f32,
}

/// The themes, in picker order.
///
/// Deliberately not six shades of the same near-black: a light one, a
/// fluorescent one, a colourful one, a minimal one, a flashy one, and one
/// restrained dark one. Every surface is themed — the notch body, the card
/// glass, the ink the marks are drawn in — so these are different designs
/// rather than one design with the accent swapped.
pub const THEMES: &[&str] = &[
    "midnight", "paper", "acid", "prism", "slate", "neon", "sakura", "mecha", "pitch", "arcade",
];

/// Restrained near-black. The default, and the one that disappears into any
/// desktop.
pub const MIDNIGHT: Palette = Palette {
    bg: Color32::from_rgb(0x0E, 0x0F, 0x12),
    border: Color32::from_rgb(0x24, 0x26, 0x2B),
    card: Color32::from_rgb(0x16, 0x18, 0x1C),
    card_hover: Color32::from_rgb(0x1E, 0x21, 0x26),
    text: Color32::from_rgb(0xF0, 0xF1, 0xF3),
    muted: Color32::from_rgb(0x9A, 0xA0, 0xA8),
    faint: Color32::from_rgb(0x5E, 0x64, 0x6D),
    ok: Color32::from_rgb(0x34, 0xD3, 0x99),
    warn: Color32::from_rgb(0xFB, 0xBF, 0x24),
    bad: Color32::from_rgb(0xF8, 0x71, 0x71),
    stale: Color32::from_rgb(0x6B, 0x72, 0x80),
    track: Color32::from_rgb(0x2A, 0x2D, 0x33),
    accent: Color32::from_rgb(0x7C, 0xA8, 0xFF),
    ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    rail_bg: Color32::from_rgba_premultiplied(5, 6, 8, 168),
    rail_disc: Color32::from_rgb(0x20, 0x23, 0x29),
    rail_deep: Color32::from_rgba_premultiplied(8, 10, 13, 174),
    hairline: Color32::from_rgba_premultiplied(14, 15, 15, 17),
    sheen: Color32::from_rgba_premultiplied(29, 29, 31, 32),
    control: Color32::from_rgb(0x1C, 0x1E, 0x23),
    control_hi: Color32::from_rgb(0x28, 0x2B, 0x31),
    gauge: [
        Color32::from_rgb(0x34, 0xD3, 0x99),
        Color32::from_rgb(0xFD, 0xE0, 0x47),
        Color32::from_rgb(0xFB, 0x92, 0x3C),
        Color32::from_rgb(0xF4, 0x3F, 0x5E),
    ],
    glow: 0.22,
};

/// Light. A white notch with dark ink — the one for people who do not run a
/// dark desktop and would rather this did not punch a black hole in it.
pub const PAPER: Palette = Palette {
    bg: Color32::from_rgb(0xF7, 0xF6, 0xF3),
    border: Color32::from_rgb(0xDD, 0xDA, 0xD3),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    card_hover: Color32::from_rgb(0xF0, 0xEE, 0xEA),
    text: Color32::from_rgb(0x16, 0x18, 0x1D),
    muted: Color32::from_rgb(0x5C, 0x62, 0x6B),
    faint: Color32::from_rgb(0x8D, 0x94, 0x9E),
    ok: Color32::from_rgb(0x0B, 0x8A, 0x4B),
    warn: Color32::from_rgb(0xB4, 0x76, 0x00),
    bad: Color32::from_rgb(0xC4, 0x25, 0x2B),
    stale: Color32::from_rgb(0x9A, 0xA0, 0xA8),
    track: Color32::from_rgb(0xE2, 0xE0, 0xDA),
    accent: Color32::from_rgb(0x1D, 0x4E, 0xD8),
    ink: Color32::from_rgb(0x14, 0x17, 0x1C),
    rail_bg: Color32::from_rgba_premultiplied(168, 168, 168, 168),
    rail_disc: Color32::from_rgb(0xE8, 0xE6, 0xE0),
    rail_deep: Color32::from_rgba_premultiplied(174, 174, 174, 174),
    hairline: Color32::from_rgba_premultiplied(2, 2, 3, 30),
    sheen: Color32::from_rgba_premultiplied(40, 40, 40, 40),
    control: Color32::from_rgb(0xEE, 0xEC, 0xE7),
    control_hi: Color32::from_rgb(0xE0, 0xDD, 0xD6),
    gauge: [
        Color32::from_rgb(0x0B, 0x8A, 0x4B),
        Color32::from_rgb(0xC9, 0x8A, 0x00),
        Color32::from_rgb(0xDB, 0x62, 0x0B),
        Color32::from_rgb(0xC4, 0x25, 0x2B),
    ],
    glow: 0.0,
};

/// Fluorescent. A highlighter-green notch with black ink and dark saturated
/// gauges.
///
/// Two things follow from the surface being bright rather than dark. The heat
/// scale runs *darker* as it warms, because a pale colour would vanish into
/// the field. And every surface in the theme has to be bright, not just the
/// notch: the card and the settings sheet share one set of text colours, so a
/// dark sheet with a bright card would leave one of them unreadable.
pub const ACID: Palette = Palette {
    bg: Color32::from_rgb(0xB6, 0xEF, 0x14),
    border: Color32::from_rgb(0x7C, 0xA6, 0x0B),
    card: Color32::from_rgb(0xC9, 0xF9, 0x45),
    card_hover: Color32::from_rgb(0xD7, 0xFC, 0x70),
    text: Color32::from_rgb(0x0B, 0x14, 0x00),
    muted: Color32::from_rgb(0x33, 0x45, 0x09),
    faint: Color32::from_rgb(0x59, 0x6F, 0x16),
    ok: Color32::from_rgb(0x0A, 0x4F, 0x2E),
    warn: Color32::from_rgb(0x6B, 0x49, 0x00),
    bad: Color32::from_rgb(0x8E, 0x10, 0x22),
    stale: Color32::from_rgb(0x59, 0x6F, 0x16),
    track: Color32::from_rgba_premultiplied(16, 24, 3, 62),
    accent: Color32::from_rgb(0x12, 0x33, 0x00),
    ink: Color32::from_rgb(0x0B, 0x14, 0x00),
    rail_bg: Color32::from_rgba_premultiplied(132, 168, 20, 168),
    rail_disc: Color32::from_rgba_premultiplied(26, 38, 5, 52),
    rail_deep: Color32::from_rgba_premultiplied(146, 174, 59, 174),
    hairline: Color32::from_rgba_premultiplied(1, 3, 0, 34),
    sheen: Color32::from_rgba_premultiplied(70, 70, 70, 70),
    control: Color32::from_rgb(0xC1, 0xF3, 0x2C),
    control_hi: Color32::from_rgb(0xD1, 0xF8, 0x5C),
    gauge: [
        Color32::from_rgb(0x0A, 0x4F, 0x2E),
        Color32::from_rgb(0x6B, 0x49, 0x00),
        Color32::from_rgb(0x9B, 0x2E, 0x00),
        Color32::from_rgb(0x8E, 0x10, 0x22),
    ],
    glow: 0.0,
};

/// Colourful. A deep violet field with the gauge running right round the
/// wheel — cyan, lime, amber, magenta — rather than four steps of one hue.
pub const PRISM: Palette = Palette {
    bg: Color32::from_rgb(0x14, 0x0C, 0x30),
    border: Color32::from_rgb(0x3A, 0x2A, 0x6E),
    card: Color32::from_rgb(0x1D, 0x12, 0x44),
    card_hover: Color32::from_rgb(0x28, 0x1A, 0x57),
    text: Color32::from_rgb(0xF2, 0xEB, 0xFF),
    muted: Color32::from_rgb(0xB0, 0xA0, 0xDC),
    faint: Color32::from_rgb(0x77, 0x66, 0xA8),
    ok: Color32::from_rgb(0x2D, 0xE0, 0xC8),
    warn: Color32::from_rgb(0xFF, 0xD1, 0x3C),
    bad: Color32::from_rgb(0xFF, 0x4D, 0xA6),
    stale: Color32::from_rgb(0x77, 0x66, 0xA8),
    track: Color32::from_rgb(0x2E, 0x1F, 0x60),
    accent: Color32::from_rgb(0x5B, 0xD4, 0xFF),
    ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    rail_bg: Color32::from_rgba_premultiplied(18, 11, 42, 168),
    rail_disc: Color32::from_rgb(0x2A, 0x1B, 0x5E),
    rail_deep: Color32::from_rgba_premultiplied(25, 15, 57, 174),
    hairline: Color32::from_rgba_premultiplied(20, 19, 26, 26),
    sheen: Color32::from_rgba_premultiplied(33, 49, 54, 54),
    control: Color32::from_rgb(0x21, 0x15, 0x4C),
    control_hi: Color32::from_rgb(0x2E, 0x1E, 0x63),
    gauge: [
        Color32::from_rgb(0x2D, 0xE0, 0xC8),
        Color32::from_rgb(0xB4, 0xF0, 0x3A),
        Color32::from_rgb(0xFF, 0xB0, 0x2E),
        Color32::from_rgb(0xFF, 0x3D, 0x9E),
    ],
    glow: 0.6,
};

/// Minimal. One cool hue, low saturation, no bloom. For people who want to
/// read a number and not be shown a light display.
pub const SLATE: Palette = Palette {
    bg: Color32::from_rgb(0x14, 0x17, 0x1B),
    border: Color32::from_rgb(0x2C, 0x32, 0x39),
    card: Color32::from_rgb(0x1A, 0x1E, 0x23),
    card_hover: Color32::from_rgb(0x23, 0x28, 0x2E),
    text: Color32::from_rgb(0xDF, 0xE4, 0xEA),
    muted: Color32::from_rgb(0x8B, 0x96, 0xA3),
    faint: Color32::from_rgb(0x59, 0x63, 0x6E),
    ok: Color32::from_rgb(0x8F, 0xB8, 0xA6),
    warn: Color32::from_rgb(0xC2, 0xB6, 0x8B),
    bad: Color32::from_rgb(0xC1, 0x8A, 0x90),
    stale: Color32::from_rgb(0x59, 0x63, 0x6E),
    track: Color32::from_rgb(0x2A, 0x30, 0x37),
    accent: Color32::from_rgb(0x8B, 0x96, 0xA3),
    ink: Color32::from_rgb(0xE6, 0xEB, 0xF0),
    rail_bg: Color32::from_rgba_premultiplied(13, 15, 18, 168),
    rail_disc: Color32::from_rgb(0x23, 0x28, 0x2E),
    rail_deep: Color32::from_rgba_premultiplied(17, 20, 23, 174),
    hairline: Color32::from_rgba_premultiplied(10, 11, 12, 16),
    sheen: Color32::from_rgba_premultiplied(17, 18, 19, 22),
    control: Color32::from_rgb(0x1C, 0x21, 0x26),
    control_hi: Color32::from_rgb(0x26, 0x2C, 0x33),
    gauge: [
        Color32::from_rgb(0x8F, 0xB8, 0xA6),
        Color32::from_rgb(0xC2, 0xB6, 0x8B),
        Color32::from_rgb(0xC3, 0x9C, 0x7E),
        Color32::from_rgb(0xC1, 0x8A, 0x90),
    ],
    glow: 0.0,
};

/// Flashy. Searing colour on near-black, glowing as hard as the renderer
/// allows.
pub const NEON: Palette = Palette {
    bg: Color32::from_rgb(0x07, 0x03, 0x0E),
    border: Color32::from_rgb(0x2E, 0x1B, 0x4E),
    card: Color32::from_rgb(0x11, 0x08, 0x1F),
    card_hover: Color32::from_rgb(0x1B, 0x0F, 0x2F),
    text: Color32::from_rgb(0xF6, 0xEC, 0xFF),
    muted: Color32::from_rgb(0xAE, 0x9A, 0xD6),
    faint: Color32::from_rgb(0x6E, 0x5C, 0x96),
    ok: Color32::from_rgb(0x00, 0xFF, 0xB2),
    warn: Color32::from_rgb(0xFF, 0xF0, 0x1F),
    bad: Color32::from_rgb(0xFF, 0x00, 0x55),
    stale: Color32::from_rgb(0x6E, 0x5C, 0x96),
    track: Color32::from_rgb(0x24, 0x14, 0x3C),
    accent: Color32::from_rgb(0x00, 0xE5, 0xFF),
    ink: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    rail_bg: Color32::from_rgba_premultiplied(3, 1, 7, 168),
    rail_disc: Color32::from_rgb(0x1C, 0x10, 0x30),
    rail_deep: Color32::from_rgba_premultiplied(8, 3, 16, 174),
    hairline: Color32::from_rgba_premultiplied(18, 16, 24, 24),
    sheen: Color32::from_rgba_premultiplied(31, 62, 64, 64),
    control: Color32::from_rgb(0x16, 0x0B, 0x28),
    control_hi: Color32::from_rgb(0x22, 0x13, 0x3B),
    gauge: [
        Color32::from_rgb(0x00, 0xFF, 0xB2),
        Color32::from_rgb(0xFF, 0xF0, 0x1F),
        Color32::from_rgb(0xFF, 0x7A, 0x00),
        Color32::from_rgb(0xFF, 0x00, 0x55),
    ],
    glow: 1.0,
};

/// Cherry blossom. A pale pink field with plum ink — the second light theme,
/// and the softest thing in the set.
pub const SAKURA: Palette = Palette {
    bg: Color32::from_rgb(0xFF, 0xF3, 0xF7),
    border: Color32::from_rgb(0xEC, 0xC6, 0xD6),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    card_hover: Color32::from_rgb(0xFD, 0xE9, 0xF0),
    text: Color32::from_rgb(0x3B, 0x11, 0x28),
    muted: Color32::from_rgb(0x8A, 0x5A, 0x71),
    faint: Color32::from_rgb(0xB4, 0x8C, 0x9E),
    ok: Color32::from_rgb(0x0F, 0x7A, 0x5A),
    warn: Color32::from_rgb(0xA8, 0x6A, 0x0A),
    bad: Color32::from_rgb(0xC0, 0x1F, 0x57),
    stale: Color32::from_rgb(0xB4, 0x8C, 0x9E),
    track: Color32::from_rgb(0xF2, 0xD6, 0xE0),
    accent: Color32::from_rgb(0xD1, 0x4C, 0x8A),
    ink: Color32::from_rgb(0x3B, 0x11, 0x28),
    rail_bg: Color32::from_rgba_premultiplied(168, 150, 157, 168),
    rail_disc: Color32::from_rgb(0xF7, 0xDC, 0xE6),
    rail_deep: Color32::from_rgba_premultiplied(174, 164, 167, 174),
    hairline: Color32::from_rgba_premultiplied(7, 2, 5, 30),
    sheen: Color32::from_rgba_premultiplied(44, 44, 44, 44),
    control: Color32::from_rgb(0xFB, 0xE4, 0xEC),
    control_hi: Color32::from_rgb(0xF5, 0xD2, 0xDF),
    gauge: [
        Color32::from_rgb(0x0F, 0x7A, 0x5A),
        Color32::from_rgb(0xC0, 0x8A, 0x12),
        Color32::from_rgb(0xD8, 0x5A, 0x2E),
        Color32::from_rgb(0xC0, 0x1F, 0x57),
    ],
    glow: 0.0,
};

/// Hangar. Deep indigo steel with hazard markings — acid green and warning
/// amber against a cockpit-dark field.
pub const MECHA: Palette = Palette {
    bg: Color32::from_rgb(0x0A, 0x0D, 0x18),
    border: Color32::from_rgb(0x27, 0x2F, 0x4A),
    card: Color32::from_rgb(0x11, 0x16, 0x26),
    card_hover: Color32::from_rgb(0x1A, 0x21, 0x36),
    text: Color32::from_rgb(0xE6, 0xEE, 0xFF),
    muted: Color32::from_rgb(0x94, 0xA6, 0xC6),
    faint: Color32::from_rgb(0x5C, 0x6C, 0x8C),
    ok: Color32::from_rgb(0x8F, 0xFF, 0x00),
    warn: Color32::from_rgb(0xFF, 0xD4, 0x00),
    bad: Color32::from_rgb(0xFF, 0x2E, 0x2E),
    stale: Color32::from_rgb(0x5C, 0x6C, 0x8C),
    track: Color32::from_rgb(0x1E, 0x26, 0x3E),
    accent: Color32::from_rgb(0x7B, 0x5C, 0xFF),
    ink: Color32::from_rgb(0xE6, 0xEE, 0xFF),
    rail_bg: Color32::from_rgba_premultiplied(7, 9, 16, 168),
    rail_disc: Color32::from_rgb(0x1B, 0x22, 0x38),
    rail_deep: Color32::from_rgba_premultiplied(12, 16, 29, 174),
    hairline: Color32::from_rgba_premultiplied(12, 22, 0, 22),
    sheen: Color32::from_rgba_premultiplied(35, 53, 58, 58),
    control: Color32::from_rgb(0x14, 0x1A, 0x2C),
    control_hi: Color32::from_rgb(0x1F, 0x27, 0x40),
    gauge: [
        Color32::from_rgb(0x8F, 0xFF, 0x00),
        Color32::from_rgb(0xFF, 0xD4, 0x00),
        Color32::from_rgb(0xFF, 0x6A, 0x00),
        Color32::from_rgb(0xFF, 0x2E, 0x2E),
    ],
    glow: 0.85,
};

/// Floodlit. Night-match green with chalk-white markings.
pub const PITCH: Palette = Palette {
    bg: Color32::from_rgb(0x06, 0x2A, 0x17),
    border: Color32::from_rgb(0x17, 0x52, 0x33),
    card: Color32::from_rgb(0x09, 0x36, 0x1E),
    card_hover: Color32::from_rgb(0x0E, 0x46, 0x28),
    text: Color32::from_rgb(0xEF, 0xFA, 0xF2),
    muted: Color32::from_rgb(0x93, 0xC4, 0xA8),
    faint: Color32::from_rgb(0x5C, 0x8C, 0x72),
    ok: Color32::from_rgb(0x7C, 0xFC, 0x55),
    warn: Color32::from_rgb(0xFF, 0xE1, 0x4D),
    bad: Color32::from_rgb(0xFF, 0x3B, 0x3B),
    stale: Color32::from_rgb(0x5C, 0x8C, 0x72),
    track: Color32::from_rgb(0x12, 0x46, 0x2A),
    accent: Color32::from_rgb(0xEF, 0xFA, 0xF2),
    ink: Color32::from_rgb(0xEF, 0xFA, 0xF2),
    rail_bg: Color32::from_rgba_premultiplied(4, 32, 17, 168),
    rail_disc: Color32::from_rgb(0x11, 0x44, 0x28),
    rail_deep: Color32::from_rgba_premultiplied(7, 43, 24, 174),
    hairline: Color32::from_rgba_premultiplied(19, 21, 20, 22),
    sheen: Color32::from_rgba_premultiplied(52, 52, 52, 52),
    control: Color32::from_rgb(0x0B, 0x3A, 0x21),
    control_hi: Color32::from_rgb(0x12, 0x4B, 0x2C),
    gauge: [
        Color32::from_rgb(0x7C, 0xFC, 0x55),
        Color32::from_rgb(0xFF, 0xE1, 0x4D),
        Color32::from_rgb(0xFF, 0x8A, 0x3D),
        Color32::from_rgb(0xFF, 0x3B, 0x3B),
    ],
    glow: 0.45,
};

/// Cabinet. Black glass and phosphor, the way a CRT looked with the lights
/// off.
pub const ARCADE: Palette = Palette {
    bg: Color32::from_rgb(0x02, 0x04, 0x02),
    border: Color32::from_rgb(0x1A, 0x38, 0x14),
    card: Color32::from_rgb(0x06, 0x0B, 0x06),
    card_hover: Color32::from_rgb(0x0C, 0x16, 0x0A),
    text: Color32::from_rgb(0xC9, 0xFF, 0xC2),
    muted: Color32::from_rgb(0x5F, 0xB8, 0x53),
    faint: Color32::from_rgb(0x3A, 0x73, 0x33),
    ok: Color32::from_rgb(0x39, 0xFF, 0x14),
    warn: Color32::from_rgb(0xFF, 0xC4, 0x00),
    bad: Color32::from_rgb(0xFF, 0x2D, 0x95),
    stale: Color32::from_rgb(0x3A, 0x73, 0x33),
    track: Color32::from_rgb(0x14, 0x2A, 0x11),
    accent: Color32::from_rgb(0x00, 0xF0, 0xD4),
    ink: Color32::from_rgb(0xC9, 0xFF, 0xC2),
    rail_bg: Color32::from_rgba_premultiplied(0, 0, 0, 168),
    rail_disc: Color32::from_rgb(0x0E, 0x1F, 0x0C),
    rail_deep: Color32::from_rgba_premultiplied(3, 5, 3, 174),
    hairline: Color32::from_rgba_premultiplied(6, 26, 2, 26),
    sheen: Color32::from_rgba_premultiplied(13, 60, 5, 60),
    control: Color32::from_rgb(0x08, 0x12, 0x07),
    control_hi: Color32::from_rgb(0x11, 0x24, 0x0E),
    gauge: [
        Color32::from_rgb(0x39, 0xFF, 0x14),
        Color32::from_rgb(0xFF, 0xC4, 0x00),
        Color32::from_rgb(0xFF, 0x6A, 0x00),
        Color32::from_rgb(0xFF, 0x2D, 0x95),
    ],
    glow: 1.0,
};

pub fn palette(name: &str) -> Palette {
    match name {
        "paper" => PAPER,
        "acid" => ACID,
        "prism" => PRISM,
        "slate" => SLATE,
        "neon" => NEON,
        "sakura" => SAKURA,
        "mecha" => MECHA,
        "pitch" => PITCH,
        "arcade" => ARCADE,
        // Retired palettes map to their nearest survivor rather than silently
        // reverting to the default, so an existing config keeps looking like
        // the thing its owner chose.
        "flux" | "tokyo-night" => NEON,
        "aurora" | "catppuccin" | "ember" | "gruvbox" => PRISM,
        "chrome" => SLATE,
        _ => MIDNIGHT,
    }
}

/// Letter shown inside a provider's monogram disc (distinct per provider).
pub fn monogram(id: &str) -> String {
    match id {
        "claude" => "C".into(),
        "codex" => "X".into(),
        "grok" => "G".into(),
        "minimax" => "M".into(),
        "kimi" => "K".into(),
        other => other
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".into()),
    }
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Inter (SIL OFL, assets/fonts) in three weights. Regular is installed as
/// the default proportional face; Medium/SemiBold are named families.
const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular-sub.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Inter-Medium-sub.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold-sub.ttf");
const HACK_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Hack-Regular-sub.ttf");

pub fn sans(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("inter-medium".into()))
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("inter-semibold".into()))
}

/// Register Inter. Called once before the first frame.
pub fn install_fonts(ctx: &egui::Context) {
    // eframe's `default_fonts` feature is off: nothing is registered for us, so
    // every family the app asks for has to be defined here.
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "inter".into(),
        std::sync::Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    fonts.font_data.insert(
        "inter-medium".into(),
        std::sync::Arc::new(FontData::from_static(INTER_MEDIUM)),
    );
    fonts.font_data.insert(
        "inter-semibold".into(),
        std::sync::Arc::new(FontData::from_static(INTER_SEMIBOLD)),
    );
    fonts.font_data.insert(
        "hack".into(),
        std::sync::Arc::new(FontData::from_static(HACK_REGULAR)),
    );
    fonts
        .families
        .insert(FontFamily::Proportional, vec!["inter".into()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["hack".into()]);
    fonts.families.insert(
        FontFamily::Name("inter-medium".into()),
        vec!["inter-medium".into()],
    );
    fonts.families.insert(
        FontFamily::Name("inter-semibold".into()),
        vec!["inter-semibold".into()],
    );
    ctx.set_fonts(fonts);
}

/// Continuous gauge colour for a *used* fraction (0..1): the theme's calm
/// colour holds until ~45 %, then warms through caution and hot toward the
/// exhausted colour.
pub fn heat(used: f32, pal: &Palette) -> Color32 {
    let [g, y, o, r] = pal.gauge;
    let u = used.clamp(0.0, 1.0);
    let seg = |a: Color32, b: Color32, lo: f32, hi: f32| mix(a, b, (u - lo) / (hi - lo));
    if u < 0.45 {
        g
    } else if u < 0.6 {
        seg(g, y, 0.45, 0.6)
    } else if u < 0.75 {
        seg(y, o, 0.6, 0.75)
    } else if u < 0.95 {
        seg(o, r, 0.75, 0.95)
    } else {
        r
    }
}

/// Re-express a premultiplied translucent surface at a new opacity, keeping
/// its hue. Premultiplied components scale linearly with alpha, so all four
/// channels take the same factor. `opacity` is 0..1; a fully opaque input is
/// returned unchanged only when `opacity` is 1.
pub fn at_opacity(c: Color32, opacity: f32) -> Color32 {
    let a = c.a() as f32;
    if a <= 0.0 {
        return c;
    }
    let k = (opacity.clamp(0.0, 1.0) * 255.0) / a;
    let ch = |v: u8| (v as f32 * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(ch(c.r()), ch(c.g()), ch(c.b()), ch(c.a()))
}

/// Linear blend between two colors, `t`=0 → `a`, `t`=1 → `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Text color that reads on top of a brand disc.
pub fn on_brand(brand: Color32) -> Color32 {
    let lum = 0.299 * brand.r() as f32 + 0.587 * brand.g() as f32 + 0.114 * brand.b() as f32;
    if lum > 130.0 {
        Color32::from_rgb(18, 20, 24) // dark letter on bright disc
    } else {
        Color32::from_rgb(240, 242, 245) // light letter on dark disc
    }
}

/// Install the dark instrument look: dark tooltips, floating scrollbar,
/// styled selection. Call once at startup.
pub fn apply_style(ctx: &egui::Context, pal: &Palette) {
    // Applied to every theme's style once at startup, so switching the OS
    // theme under the app cannot leave one of them undecorated.
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.override_text_color = Some(pal.text);
        v.panel_fill = pal.bg;
        v.window_fill = pal.card; // tooltip background
        v.window_stroke = Stroke::new(1.0_f32, pal.border); // tooltip border
        v.menu_corner_radius = CornerRadius::same(10);
        v.popup_shadow = egui::Shadow::NONE;
        v.window_shadow = egui::Shadow::NONE;
        // separators / non-interactive strokes follow the border color
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.border);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.border);
        v.selection.bg_fill = pal.accent.gamma_multiply(0.30);
        v.selection.stroke = Stroke::new(1.0_f32, pal.accent);
        style.spacing.menu_margin = egui::Margin::symmetric(8, 6);
        style.spacing.item_spacing = egui::Vec2::new(8.0, 4.0);
        style.spacing.scroll = {
            let mut s = egui::style::ScrollStyle::floating();
            s.floating_width = 5.0;
            s.floating_allocated_width = 7.0;
            s.bar_inner_margin = 3.0;
            s.bar_outer_margin = 3.0;
            s.handle_min_length = 24.0;
            s.dormant_handle_opacity = 0.15;
            s.active_handle_opacity = 0.6;
            s
        };
    });
}

/// Micro caps label — uppercase, letter-tracked, used for section headers,
/// units and status words. Tracking is what makes 9–10 px uppercase legible.
pub fn caps_job(text: &str, size: f32, color: Color32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        egui::text::TextFormat {
            font_id: medium(size),
            extra_letter_spacing: (size * 0.09).round().max(1.0),
            color,
            ..Default::default()
        },
    );
    job
}
