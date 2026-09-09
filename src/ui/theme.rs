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
}

pub const THEMES: &[&str] = &["midnight", "tokyo-night", "catppuccin", "gruvbox"];

pub const MIDNIGHT: Palette = Palette {
    bg: Color32::from_rgb(15, 16, 18),
    border: Color32::from_rgb(42, 44, 48),
    card: Color32::from_rgb(24, 26, 29),
    card_hover: Color32::from_rgb(34, 36, 40),
    text: Color32::from_rgb(239, 240, 235),
    muted: Color32::from_rgb(151, 155, 157),
    faint: Color32::from_rgb(94, 99, 103),
    ok: Color32::from_rgb(106, 220, 171),
    warn: Color32::from_rgb(246, 190, 81),
    bad: Color32::from_rgb(248, 108, 104),
    stale: Color32::from_rgb(116, 121, 125),
    track: Color32::from_rgb(47, 50, 54),
    accent: Color32::from_rgb(139, 187, 255),
    rail_bg: Color32::from_rgba_premultiplied(4, 6, 10, 168),
    rail_disc: Color32::from_rgb(0x24, 0x26, 0x2C),
    rail_deep: Color32::from_rgba_premultiplied(12, 15, 21, 174),
    hairline: Color32::from_rgba_premultiplied(16, 16, 16, 16),
    sheen: Color32::from_rgba_premultiplied(30, 30, 30, 30),
    control: Color32::from_rgb(28, 30, 34),
    control_hi: Color32::from_rgb(40, 43, 48),
    gauge: [Color32::from_rgb(52, 211, 97), Color32::from_rgb(255, 214, 10), Color32::from_rgb(255, 149, 0), Color32::from_rgb(255, 69, 58)],
};

pub const TOKYO_NIGHT: Palette = Palette {
    bg: Color32::from_rgb(26, 27, 38),
    border: Color32::from_rgb(47, 51, 77),
    card: Color32::from_rgb(31, 34, 51),
    card_hover: Color32::from_rgb(40, 44, 66),
    text: Color32::from_rgb(192, 202, 245),
    muted: Color32::from_rgb(154, 165, 206),
    faint: Color32::from_rgb(86, 95, 137),
    ok: Color32::from_rgb(158, 206, 106),
    warn: Color32::from_rgb(224, 175, 104),
    bad: Color32::from_rgb(247, 118, 142),
    stale: Color32::from_rgb(107, 115, 159),
    track: Color32::from_rgb(42, 46, 66),
    accent: Color32::from_rgb(122, 162, 247),
    rail_bg: Color32::from_rgba_premultiplied(4, 6, 10, 168),
    rail_disc: Color32::from_rgb(0x24, 0x26, 0x2C),
    rail_deep: Color32::from_rgba_premultiplied(12, 15, 21, 174),
    hairline: Color32::from_rgba_premultiplied(16, 16, 16, 16),
    sheen: Color32::from_rgba_premultiplied(30, 30, 30, 30),
    control: Color32::from_rgb(36, 40, 59),
    control_hi: Color32::from_rgb(48, 53, 76),
    gauge: [Color32::from_rgb(158, 206, 106), Color32::from_rgb(224, 175, 104), Color32::from_rgb(255, 158, 100), Color32::from_rgb(247, 118, 142)],
};

pub const CATPPUCCIN: Palette = Palette {
    bg: Color32::from_rgb(24, 24, 37),
    border: Color32::from_rgb(49, 50, 68),
    card: Color32::from_rgb(30, 30, 46),
    card_hover: Color32::from_rgb(40, 40, 58),
    text: Color32::from_rgb(205, 214, 244),
    muted: Color32::from_rgb(166, 173, 200),
    faint: Color32::from_rgb(108, 112, 134),
    ok: Color32::from_rgb(166, 227, 161),
    warn: Color32::from_rgb(249, 226, 175),
    bad: Color32::from_rgb(243, 139, 168),
    stale: Color32::from_rgb(127, 132, 156),
    track: Color32::from_rgb(49, 50, 68),
    accent: Color32::from_rgb(137, 180, 250),
    rail_bg: Color32::from_rgba_premultiplied(4, 6, 10, 168),
    rail_disc: Color32::from_rgb(0x24, 0x26, 0x2C),
    rail_deep: Color32::from_rgba_premultiplied(12, 15, 21, 174),
    hairline: Color32::from_rgba_premultiplied(16, 16, 16, 16),
    sheen: Color32::from_rgba_premultiplied(30, 30, 30, 30),
    control: Color32::from_rgb(35, 35, 52),
    control_hi: Color32::from_rgb(49, 50, 68),
    gauge: [Color32::from_rgb(166, 227, 161), Color32::from_rgb(249, 226, 175), Color32::from_rgb(250, 179, 135), Color32::from_rgb(243, 139, 168)],
};

pub const GRUVBOX: Palette = Palette {
    bg: Color32::from_rgb(40, 40, 40),
    border: Color32::from_rgb(60, 56, 54),
    card: Color32::from_rgb(50, 48, 47),
    card_hover: Color32::from_rgb(60, 56, 54),
    text: Color32::from_rgb(235, 219, 178),
    muted: Color32::from_rgb(189, 174, 147),
    faint: Color32::from_rgb(146, 131, 116),
    ok: Color32::from_rgb(184, 187, 38),
    warn: Color32::from_rgb(250, 189, 47),
    bad: Color32::from_rgb(251, 73, 52),
    stale: Color32::from_rgb(146, 131, 116),
    track: Color32::from_rgb(60, 56, 54),
    accent: Color32::from_rgb(131, 165, 152),
    rail_bg: Color32::from_rgba_premultiplied(4, 6, 10, 168),
    rail_disc: Color32::from_rgb(0x24, 0x26, 0x2C),
    rail_deep: Color32::from_rgba_premultiplied(12, 15, 21, 174),
    hairline: Color32::from_rgba_premultiplied(16, 16, 16, 16),
    sheen: Color32::from_rgba_premultiplied(30, 30, 30, 30),
    control: Color32::from_rgb(56, 53, 51),
    control_hi: Color32::from_rgb(72, 68, 65),
    gauge: [Color32::from_rgb(184, 187, 38), Color32::from_rgb(250, 189, 47), Color32::from_rgb(254, 128, 25), Color32::from_rgb(251, 73, 52)],
};

pub fn palette(name: &str) -> Palette {
    match name {
        "tokyo-night" => TOKYO_NIGHT,
        "catppuccin" => CATPPUCCIN,
        "gruvbox" => GRUVBOX,
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
