//! Theme system: semantic palettes + global egui styling.
//!
//! Every color in the UI comes from a [`Palette`]; nothing else hard-codes
//! colors. All themes are dark and share the same semantic roles.

use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, Style};

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

/// The themes, in picker order. Each one re-colours *everything* — including
/// the notch body and the card glass, which used to be identical across all
/// four themes, so switching theme changed only the settings sheet and the
/// gauge hues. That is why the old set felt like it did nothing.
pub const THEMES: &[&str] = &["midnight", "flux", "ember", "aurora", "chrome"];

/// Restrained neutral. The default, and the one that disappears into any
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

/// Fluorescent. Violet-black with acid green, electric yellow and hot
/// magenta, glowing hard. The one that gets noticed.
pub const FLUX: Palette = Palette {
    bg: Color32::from_rgb(0x0A, 0x07, 0x12),
    border: Color32::from_rgb(0x2A, 0x1F, 0x45),
    card: Color32::from_rgb(0x13, 0x0E, 0x23),
    card_hover: Color32::from_rgb(0x1D, 0x15, 0x33),
    text: Color32::from_rgb(0xF2, 0xEC, 0xFF),
    muted: Color32::from_rgb(0xA9, 0x9C, 0xC8),
    faint: Color32::from_rgb(0x6B, 0x5F, 0x8C),
    ok: Color32::from_rgb(0x39, 0xFF, 0x88),
    warn: Color32::from_rgb(0xEA, 0xFF, 0x00),
    bad: Color32::from_rgb(0xFF, 0x2E, 0x88),
    stale: Color32::from_rgb(0x6B, 0x5F, 0x8C),
    track: Color32::from_rgb(0x24, 0x1A, 0x3B),
    accent: Color32::from_rgb(0x00, 0xE5, 0xFF),
    rail_bg: Color32::from_rgba_premultiplied(7, 4, 13, 168),
    rail_disc: Color32::from_rgb(0x1E, 0x16, 0x36),
    rail_deep: Color32::from_rgba_premultiplied(12, 7, 22, 174),
    hairline: Color32::from_rgba_premultiplied(17, 14, 22, 22),
    sheen: Color32::from_rgba_premultiplied(34, 55, 58, 58),
    control: Color32::from_rgb(0x17, 0x10, 0x2A),
    control_hi: Color32::from_rgb(0x24, 0x19, 0x38),
    gauge: [
        Color32::from_rgb(0x39, 0xFF, 0x88),
        Color32::from_rgb(0xEA, 0xFF, 0x00),
        Color32::from_rgb(0xFF, 0x9E, 0x00),
        Color32::from_rgb(0xFF, 0x2E, 0x88),
    ],
    glow: 0.95,
};

/// Warm dark: charcoal, copper and amber, like a lamp on a desk.
pub const EMBER: Palette = Palette {
    bg: Color32::from_rgb(0x12, 0x0E, 0x0A),
    border: Color32::from_rgb(0x33, 0x26, 0x1C),
    card: Color32::from_rgb(0x1A, 0x14, 0x10),
    card_hover: Color32::from_rgb(0x24, 0x1B, 0x14),
    text: Color32::from_rgb(0xF6, 0xED, 0xE2),
    muted: Color32::from_rgb(0xBF, 0xA9, 0x8F),
    faint: Color32::from_rgb(0x7A, 0x65, 0x53),
    ok: Color32::from_rgb(0x7B, 0xD8, 0x8F),
    warn: Color32::from_rgb(0xFF, 0xC2, 0x4B),
    bad: Color32::from_rgb(0xFF, 0x5C, 0x46),
    stale: Color32::from_rgb(0x7A, 0x65, 0x53),
    track: Color32::from_rgb(0x2E, 0x24, 0x1B),
    accent: Color32::from_rgb(0xFF, 0x9E, 0x3D),
    rail_bg: Color32::from_rgba_premultiplied(11, 7, 5, 168),
    rail_disc: Color32::from_rgb(0x24, 0x1A, 0x13),
    rail_deep: Color32::from_rgba_premultiplied(18, 12, 8, 174),
    hairline: Color32::from_rgba_premultiplied(20, 16, 13, 20),
    sheen: Color32::from_rgba_premultiplied(46, 37, 25, 46),
    control: Color32::from_rgb(0x1D, 0x16, 0x10),
    control_hi: Color32::from_rgb(0x2A, 0x20, 0x18),
    gauge: [
        Color32::from_rgb(0x7B, 0xD8, 0x8F),
        Color32::from_rgb(0xFF, 0xD2, 0x4B),
        Color32::from_rgb(0xFF, 0x8A, 0x3D),
        Color32::from_rgb(0xFF, 0x44, 0x38),
    ],
    glow: 0.55,
};

/// Cool and luminous: deep teal with aqua, lime and a violet accent.
pub const AURORA: Palette = Palette {
    bg: Color32::from_rgb(0x07, 0x11, 0x0F),
    border: Color32::from_rgb(0x1B, 0x3A, 0x38),
    card: Color32::from_rgb(0x0C, 0x1A, 0x18),
    card_hover: Color32::from_rgb(0x13, 0x25, 0x23),
    text: Color32::from_rgb(0xE6, 0xFB, 0xF6),
    muted: Color32::from_rgb(0x90, 0xB8, 0xB2),
    faint: Color32::from_rgb(0x54, 0x75, 0x71),
    ok: Color32::from_rgb(0x3B, 0xE8, 0xC0),
    warn: Color32::from_rgb(0xF5, 0xD7, 0x6E),
    bad: Color32::from_rgb(0xFF, 0x6B, 0x9D),
    stale: Color32::from_rgb(0x54, 0x75, 0x71),
    track: Color32::from_rgb(0x17, 0x30, 0x2D),
    accent: Color32::from_rgb(0xA7, 0x8B, 0xFA),
    rail_bg: Color32::from_rgba_premultiplied(3, 9, 9, 168),
    rail_disc: Color32::from_rgb(0x12, 0x23, 0x20),
    rail_deep: Color32::from_rgba_premultiplied(6, 14, 14, 174),
    hairline: Color32::from_rgba_premultiplied(12, 19, 17, 20),
    sheen: Color32::from_rgba_premultiplied(32, 48, 44, 48),
    control: Color32::from_rgb(0x10, 0x20, 0x1E),
    control_hi: Color32::from_rgb(0x1A, 0x2E, 0x2B),
    gauge: [
        Color32::from_rgb(0x3B, 0xE8, 0xC0),
        Color32::from_rgb(0xA8, 0xE8, 0x5C),
        Color32::from_rgb(0xFF, 0xC2, 0x4B),
        Color32::from_rgb(0xFF, 0x6B, 0x9D),
    ],
    glow: 0.7,
};

/// Polished metal: cool greys with a hard specular edge and a steel-blue
/// accent. The most "lit" of the set without being loud.
pub const CHROME: Palette = Palette {
    bg: Color32::from_rgb(0x10, 0x12, 0x16),
    border: Color32::from_rgb(0x33, 0x39, 0x41),
    card: Color32::from_rgb(0x19, 0x1D, 0x23),
    card_hover: Color32::from_rgb(0x23, 0x28, 0x30),
    text: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    muted: Color32::from_rgb(0xA8, 0xB2, 0xC0),
    faint: Color32::from_rgb(0x6A, 0x74, 0x84),
    ok: Color32::from_rgb(0x5E, 0xEA, 0xD4),
    warn: Color32::from_rgb(0xFD, 0xE6, 0x8A),
    bad: Color32::from_rgb(0xFB, 0x71, 0x85),
    stale: Color32::from_rgb(0x6A, 0x74, 0x84),
    track: Color32::from_rgb(0x2B, 0x31, 0x3A),
    accent: Color32::from_rgb(0x93, 0xC5, 0xFD),
    rail_bg: Color32::from_rgba_premultiplied(9, 11, 13, 168),
    rail_disc: Color32::from_rgb(0x26, 0x2C, 0x34),
    rail_deep: Color32::from_rgba_premultiplied(14, 16, 20, 174),
    hairline: Color32::from_rgba_premultiplied(23, 24, 26, 26),
    sheen: Color32::from_rgba_premultiplied(70, 70, 70, 70),
    control: Color32::from_rgb(0x1C, 0x20, 0x27),
    control_hi: Color32::from_rgb(0x27, 0x2D, 0x36),
    gauge: [
        Color32::from_rgb(0x5E, 0xEA, 0xD4),
        Color32::from_rgb(0xFD, 0xE6, 0x8A),
        Color32::from_rgb(0xFD, 0xBA, 0x74),
        Color32::from_rgb(0xFB, 0x71, 0x85),
    ],
    glow: 0.8,
};

pub fn palette(name: &str) -> Palette {
    match name {
        "flux" => FLUX,
        "ember" => EMBER,
        "aurora" => AURORA,
        "chrome" => CHROME,
        // The retired palettes map to their nearest survivor rather than
        // silently reverting to the default, so an existing config keeps
        // looking like the thing its owner chose.
        "tokyo-night" => FLUX,
        "catppuccin" => AURORA,
        "gruvbox" => EMBER,
        _ => MIDNIGHT,
    }
}

/// Brand color for a provider's monogram disc. Unknown providers get the accent.
pub fn brand(id: &str, pal: &Palette) -> Color32 {
    match id {
        "claude" => Color32::from_rgb(0xD9, 0x77, 0x57),
        "codex" => Color32::from_rgb(0x10, 0xA3, 0x7F),
        "minimax" => Color32::from_rgb(0xFF, 0x5A, 0x5F),
        "kimi" => Color32::from_rgb(0x7C, 0x8C, 0xFF),
        _ => pal.accent,
    }
}

/// Letter shown inside a provider's monogram disc (distinct per provider).
pub fn monogram(id: &str) -> String {
    match id {
        "claude" => "C".into(),
        "codex" => "X".into(),
        "minimax" => "M".into(),
        "kimi" => "K".into(),
        other => other.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into()),
    }
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

/// Inter (SIL OFL, assets/fonts) in three weights. Regular is installed as
/// the default proportional face; Medium/SemiBold are named families.
const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-SemiBold.ttf");

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
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("inter".into(), FontData::from_static(INTER_REGULAR));
    fonts.font_data.insert("inter-medium".into(), FontData::from_static(INTER_MEDIUM));
    fonts.font_data.insert("inter-semibold".into(), FontData::from_static(INTER_SEMIBOLD));
    // Keep egui's bundled emoji/symbol faces as fallbacks behind Inter.
    let fallbacks = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let with = |primary: &str| {
        let mut v = vec![primary.to_string()];
        v.extend(fallbacks.iter().cloned());
        v
    };
    fonts.families.insert(FontFamily::Proportional, with("inter"));
    fonts.families.insert(FontFamily::Name("inter-medium".into()), with("inter-medium"));
    fonts.families.insert(FontFamily::Name("inter-semibold".into()), with("inter-semibold"));
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
    let mut style: Style = (*ctx.style()).clone();
    let v = &mut style.visuals;
    v.override_text_color = Some(pal.text);
    v.panel_fill = pal.bg;
    v.window_fill = pal.card; // tooltip background
    v.window_stroke = Stroke::new(1.0_f32, pal.border); // tooltip border
    v.menu_rounding = Rounding::same(10.0);
    v.popup_shadow = egui::Shadow::NONE;
    v.window_shadow = egui::Shadow::NONE;
    // separators / non-interactive strokes follow the border color
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.border);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.border);
    v.selection.bg_fill = pal.accent.gamma_multiply(0.30);
    v.selection.stroke = Stroke::new(1.0_f32, pal.accent);
    style.spacing.menu_margin = egui::Margin::symmetric(8.0, 6.0);
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
    ctx.set_style(style);
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
