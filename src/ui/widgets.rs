//! Hand-drawn widgets: ring gauges, monogram discs, quota bars, badges,
//! icon buttons. Everything is painted directly for full control.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

use super::theme::{self, Palette};

/// Shared semantic color for a remaining percentage. All surfaces use the
/// same used-based heat scale so a quota never changes meaning by location.
pub fn pct_color(pct: Option<f64>, ok: bool, pal: &Palette) -> Color32 {
    if !ok {
        return pal.stale;
    }
    pct.map(|remaining| theme::heat((1.0 - remaining / 100.0) as f32, pal))
        .unwrap_or(pal.muted)
}

/// Track circle + progress arc with rounded caps, starting at 12 o'clock.
#[allow(clippy::too_many_arguments)]
pub fn ring(
    ui: &egui::Ui,
    center: Pos2,
    r: f32,
    width: f32,
    frac: f32,
    color: Color32,
    pal: &Palette,
    alpha: f32,
) {
    let p = ui.painter();
    p.circle_stroke(center, r, Stroke::new(width, pal.track.linear_multiply(alpha)));
    let frac = frac.clamp(0.0, 1.0);
    if frac <= 0.001 {
        return;
    }
    let color = color.linear_multiply(alpha);
    if frac >= 0.999 {
        p.circle_stroke(center, r, Stroke::new(width, color));
        return;
    }
    let a0 = -std::f32::consts::FRAC_PI_2;
    let a1 = a0 + std::f32::consts::TAU * frac;
    let steps = ((72.0 * frac) as usize).max(3);
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * (i as f32 / steps as f32);
            egui::pos2(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect();
    p.add(Shape::line(pts.clone(), Stroke::new(width, color)));
    // rounded end caps (epaint lines have none)
    p.circle_filled(pts[0], width / 2.0, color);
    p.circle_filled(*pts.last().unwrap(), width / 2.0, color);
}

/// Brand-colored disc with the provider's initial, wrapped by a progress ring.
/// The progress arc is the gauge; the disc is the provider identity.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub fn monogram_ring(
    ui: &egui::Ui,
    center: Pos2,
    r: f32,
    letter: &str,
    brand: Color32,
    frac: f32,
    color: Color32,
    pal: &Palette,
    alpha: f32,
) {
    monogram_ring_on(ui, center, r, letter, brand, frac, color, pal, alpha, pal.card_hover)
}

/// [`monogram_ring`] with an explicit disc fill color.
#[allow(clippy::too_many_arguments)]
pub fn monogram_ring_on(
    ui: &egui::Ui,
    center: Pos2,
    r: f32,
    letter: &str,
    brand: Color32,
    frac: f32,
    color: Color32,
    pal: &Palette,
    alpha: f32,
    disc: Color32,
) {
    let p = ui.painter();
    let disc_r = r - 3.0;
    p.circle_filled(center, disc_r, disc.linear_multiply(0.92 * alpha));
    if disc_r >= 6.0 && !letter.is_empty() {
        p.text(
            center,
            egui::Align2::CENTER_CENTER,
            letter,
            FontId::monospace(disc_r * 1.05),
            theme::on_brand(brand).linear_multiply(alpha),
        );
    }
    ring(ui, center, r, 2.5, frac, color, pal, alpha);
}

/// Like [`monogram_ring`], but the disc carries the provider's real logo
/// (white PNG) instead of a letter. `logo` None falls back to the monogram.
/// `disc` overrides the disc fill (the rail uses a near-black disc).
#[allow(clippy::too_many_arguments)]
pub fn logo_ring(
    ui: &egui::Ui,
    center: Pos2,
    r: f32,
    logo: Option<&egui::TextureHandle>,
    letter: &str,
    brand: Color32,
    frac: f32,
    color: Color32,
    pal: &Palette,
    alpha: f32,
) {
    logo_ring_on(ui, center, r, logo, letter, brand, frac, color, pal, alpha, pal.card_hover)
}

/// [`logo_ring`] with an explicit disc fill color.
#[allow(clippy::too_many_arguments)]
pub fn logo_ring_on(
    ui: &egui::Ui,
    center: Pos2,
    r: f32,
    logo: Option<&egui::TextureHandle>,
    letter: &str,
    brand: Color32,
    frac: f32,
    color: Color32,
    pal: &Palette,
    alpha: f32,
    disc: Color32,
) {
    logo_ring_stroke_on(ui, center, r, logo, letter, brand, frac, color, pal, alpha, disc, 2.5)
}

/// [`logo_ring_on`] with a caller-chosen ring stroke width (the rail's
/// small cells use a thinner stroke so the arc doesn't read chunky).
#[allow(clippy::too_many_arguments)]
pub fn logo_ring_stroke_on(
    ui: &egui::Ui,
    center: Pos2,
    r: f32,
    logo: Option<&egui::TextureHandle>,
    letter: &str,
    brand: Color32,
    frac: f32,
    color: Color32,
    pal: &Palette,
    alpha: f32,
    disc: Color32,
    stroke: f32,
) {
    let Some(tex) = logo else {
        return monogram_ring_on(ui, center, r, letter, brand, frac, color, pal, alpha, disc);
    };
    let p = ui.painter();
    // Dark disc so the white mark pops on any theme.
    let disc_r = r - 3.0;
    p.circle_filled(center, disc_r, disc.linear_multiply(0.95 * alpha));
    let side = disc_r * 1.15; // logo square inside the disc
    p.image(
        tex.id(),
        Rect::from_center_size(center, Vec2::splat(side)),
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        pal.ink.linear_multiply(alpha),
    );
    ring(ui, center, r, stroke, frac, color, pal, alpha);
}

/// Gauge cell for the rail and card header: track ring + heat-colored
/// progress arc (fraction = share *used*), with the provider's white mark
/// sitting directly on the notch black inside — no disc, the ring is the
/// only chrome. Falls back to a monogram letter when there is no logo.
#[allow(clippy::too_many_arguments)]
pub fn gauge(
    ui: &egui::Ui,
    center: Pos2,
    r: f32,
    stroke: f32,
    logo: Option<&egui::TextureHandle>,
    letter: &str,
    used: f32,
    color: Color32,
    track: Color32,
    // Colour for the provider's mark. White on a dark theme, near-black on a
    // light or fluorescent one — the marks are white PNGs, so tinting them
    // dark is what lets the notch have a bright surface at all.
    ink: Color32,
    alpha: f32,
    glow: f32,
) {
    let p = ui.painter();
    p.circle_stroke(center, r, Stroke::new(stroke, track.linear_multiply(alpha)));
    let side = r * 1.05; // white mark inside the ring
    match logo {
        Some(tex) => {
            p.image(
                tex.id(),
                Rect::from_center_size(center, Vec2::splat(side)),
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                ink.linear_multiply(alpha),
            );
        }
        None => {
            p.text(
                center,
                egui::Align2::CENTER_CENTER,
                letter,
                theme::semibold(r * 0.95),
                ink.linear_multiply(alpha),
            );
        }
    }
    // arc on top of the track, rounded caps, from 12 o'clock
    let frac = used.clamp(0.0, 1.0);
    if frac <= 0.004 {
        return;
    }
    let col = color.linear_multiply(alpha);
    if frac >= 0.999 {
        if glow > 0.0 {
            p.circle_stroke(center, r, Stroke::new(stroke * 2.8, col.gamma_multiply(0.14 * glow)));
        }
        p.circle_stroke(center, r, Stroke::new(stroke, col));
        return;
    }
    let a0 = -std::f32::consts::FRAC_PI_2;
    let a1 = a0 + std::f32::consts::TAU * frac;
    let steps = ((72.0 * frac) as usize).max(3);
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * (i as f32 / steps as f32);
            egui::pos2(center.x + r * a.cos(), center.y + r * a.sin())
        })
        .collect();
    // Bloom first, so the arc proper sits on top of its own halo.
    if glow > 0.0 {
        let halo = col.gamma_multiply(0.16 * glow);
        p.add(Shape::line(pts.clone(), Stroke::new(stroke * 2.8, halo)));
        p.circle_filled(pts[0], stroke * 1.4, halo);
        p.circle_filled(*pts.last().unwrap(), stroke * 1.4, halo);
    }
    p.circle_filled(pts[0], stroke / 2.0, col);
    p.circle_filled(*pts.last().unwrap(), stroke / 2.0, col);
    p.add(Shape::line(pts, Stroke::new(stroke, col)));
}

/// Slim rounded progress bar with a track.
pub fn bar(ui: &egui::Ui, rect: Rect, frac: f32, color: Color32, pal: &Palette, alpha: f32) {
    let p = ui.painter();
    let rounding = rect.height() / 2.0;
    p.rect_filled(rect, rounding, pal.track.linear_multiply(alpha));
    let w = (rect.width() * frac.clamp(0.0, 1.0)).max(rect.height());
    p.rect_filled(
        Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
        rounding,
        color.linear_multiply(alpha),
    );
}

/// Small outlined uppercase tag (fidelity label), allocated as a widget.
pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32, alpha: f32) -> egui::Response {
    let font = FontId::monospace(9.0);
    let galley = ui.painter().layout_no_wrap(text.to_uppercase(), font, color);
    let size = Vec2::new(galley.rect.width() + 10.0, 14.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect(
        rect,
        4.0_f32,
        Color32::TRANSPARENT,
        Stroke::new(1.0_f32, color.linear_multiply(0.55 * alpha)),
    );
    p.galley(
        egui::pos2(rect.center().x - galley.rect.width() / 2.0, rect.center().y - galley.rect.height() / 2.0),
        galley,
        color,
    );
    resp
}

/// Bordered `+N` chip for providers hidden on the collapsed pill. Click to expand.
pub fn overflow_chip(ui: &mut egui::Ui, n: usize, pal: &Palette) -> egui::Response {
    let text = format!("+{n}");
    let font = FontId::monospace(11.5);
    let galley = ui.painter().layout_no_wrap(text, font, pal.text);
    let size = Vec2::new(galley.rect.width() + 14.0, 20.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hover = resp.hovered();
    let p = ui.painter();
    p.rect(
        rect,
        7.0_f32,
        if hover { pal.card_hover } else { Color32::TRANSPARENT },
        Stroke::new(1.0_f32, if hover { pal.muted } else { pal.border }),
    );
    p.galley(
        egui::pos2(
            rect.center().x - galley.rect.width() / 2.0,
            rect.center().y - galley.rect.height() / 2.0,
        ),
        galley,
        if hover { pal.text } else { pal.muted },
    );
    resp.on_hover_text("show all providers")
}

/// A status dot (needs-auth / error marker).
pub fn dot(ui: &egui::Ui, center: Pos2, color: Color32, alpha: f32) {
    ui.painter().circle_filled(center, 3.0, color.linear_multiply(alpha));
}

/// Icon button drawn from a texture; `angle` optionally spins the glyph
/// (manual mesh rotation — egui 0.29 has no painter transform).
pub fn icon_button(
    ui: &mut egui::Ui,
    tex: &egui::TextureHandle,
    tip: &str,
    alpha: f32,
    pal: &Palette,
    angle: Option<f32>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::splat(26.0),
        if alpha > 0.9 { Sense::click() } else { Sense::hover() },
    );
    if alpha > 0.02 {
        let hovered = resp.hovered() && alpha > 0.9;
        if hovered {
            ui.painter().rect_filled(rect.shrink(2.0), 8.0_f32, pal.card_hover);
        }
        let base = if hovered { pal.ink } else { pal.ink.gamma_multiply(0.82) };
        let center = rect.center();
        let size = 17.0;
        match angle {
            None => {
                ui.painter().image(
                    tex.id(),
                    Rect::from_center_size(center, Vec2::splat(size)),
                    Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    base.linear_multiply(alpha),
                );
            }
            Some(a) => {
                let (s, c) = a.sin_cos();
                let h = size / 2.0;
                let rot = |dx: f32, dy: f32| egui::pos2(center.x + dx * c - dy * s, center.y + dx * s + dy * c);
                let mut mesh = egui::Mesh::with_texture(tex.id());
                let corners = [rot(-h, -h), rot(h, -h), rot(h, h), rot(-h, h)];
                let uvs = [egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.0), egui::pos2(1.0, 1.0), egui::pos2(0.0, 1.0)];
                let color = base.linear_multiply(alpha);
                for (pos, uv) in corners.iter().zip(uvs) {
                    mesh.vertices.push(egui::epaint::Vertex { pos: *pos, uv, color });
                }
                let i = 0;
                mesh.add_triangle(i, i + 1, i + 2);
                mesh.add_triangle(i, i + 2, i + 3);
                ui.painter().add(Shape::mesh(mesh));
            }
        }
    }
    if alpha > 0.9 {
        resp.on_hover_text(tip)
    } else {
        resp
    }
}

// ===========================================================================
// Text + surface primitives shared by the usage card and the settings sheet.
// ===========================================================================

/// Single-line galley elided with `…` at `max_w`. Use for any label that
/// shares a row with a right-aligned value — it is what keeps the two
/// columns from ever painting over each other.
pub fn elide(
    ui: &egui::Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_w: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::text::TextFormat::simple(font, color),
    );
    job.wrap = egui::text::TextWrapping {
        max_width: max_w.max(8.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// Hairline rule, one physical-ish pixel tall, inset to `x0..x1`.
pub fn rule(painter: &egui::Painter, y: f32, x0: f32, x1: f32, color: Color32) {
    painter.rect_filled(
        Rect::from_min_max(egui::pos2(x0, y), egui::pos2(x1, y + 1.0)),
        0.0_f32,
        color,
    );
}

/// Specular lip along the top edge of a glass panel: a hairline that is
/// brightest at the centre and fades out before the corners. This is the
/// single cheapest cue that a surface is glass rather than flat paint.
pub fn sheen(painter: &egui::Painter, rect: Rect, corner: f32, color: Color32) {
    let (x0, x1) = (rect.left() + corner, rect.right() - corner);
    if x1 - x0 < 8.0 {
        return;
    }
    let y = rect.top() + 1.0;
    let clear = Color32::from_rgba_premultiplied(0, 0, 0, 0);
    let mut mesh = egui::Mesh::default();
    for (x, c) in [(x0, clear), (rect.center().x, color), (x1, clear)] {
        mesh.colored_vertex(egui::pos2(x, y), c);
        mesh.colored_vertex(egui::pos2(x, y + 1.0), c);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 2, 3);
    mesh.add_triangle(2, 3, 4);
    mesh.add_triangle(3, 4, 5);
    painter.add(Shape::mesh(mesh));
}

/// Slim quota meter: rounded track with a rounded fill. Unlike [`bar`] the
/// fill can be genuinely empty (no minimum stub), so 0 % reads as 0 %.
pub fn meter(
    painter: &egui::Painter,
    rect: Rect,
    frac: f32,
    color: Color32,
    track: Color32,
    glow: f32,
) {
    let r = rect.height() / 2.0;
    painter.rect_filled(rect, r, track);
    let frac = frac.clamp(0.0, 1.0);
    if frac <= 0.0005 {
        return;
    }
    let w = (rect.width() * frac).max(rect.height());
    let filled = Rect::from_min_size(rect.min, Vec2::new(w, rect.height()));
    if glow > 0.0 {
        let spread = rect.height() * 0.9;
        painter.rect_filled(
            filled.expand(spread),
            r + spread,
            color.gamma_multiply(0.13 * glow),
        );
    }
    painter.rect_filled(filled, r, color);
}

// ===========================================================================
// Settings controls. Everything is hand-painted: egui's stock widgets carry
// a different visual language (and its arrow/trash glyphs are not in Inter,
// so they render as tofu).
// ===========================================================================

/// iOS-style switch. Animated, and the whole row-height rect is clickable.
pub fn switch(ui: &mut egui::Ui, on: &mut bool, pal: &Palette) -> egui::Response {
    let size = Vec2::new(34.0, 20.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.13);
    let track = theme::mix(pal.control_hi, pal.accent, t);
    let p = ui.painter();
    p.rect_filled(rect, rect.height() / 2.0, track);
    if t < 0.999 {
        p.rect_stroke(
            rect,
            rect.height() / 2.0,
            Stroke::new(1.0_f32, pal.border.gamma_multiply(1.0 - t)),
        );
    }
    let knob_r = rect.height() / 2.0 - 3.0;
    let x0 = rect.left() + 3.0 + knob_r;
    let cx = x0 + (rect.width() - 6.0 - knob_r * 2.0) * t;
    // On a light theme a white knob on a light accent would vanish.
    let knob = if t > 0.5 { theme::on_brand(pal.accent) } else { pal.muted };
    p.circle_filled(egui::pos2(cx, rect.center().y), knob_r, knob);
    resp
}

/// Minimal slider: 3 px track, accent fill, small knob, tabular value on the
/// right. Returns true when the value changed this frame.
pub fn slider(
    ui: &mut egui::Ui,
    value: &mut i64,
    range: std::ops::RangeInclusive<i64>,
    step: i64,
    suffix: &str,
    pal: &Palette,
) -> bool {
    let value_w = 54.0;
    let h = 20.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click_and_drag());
    const KNOB_R: f32 = 7.0;
    let track = Rect::from_min_max(
        egui::pos2(rect.left() + KNOB_R, rect.center().y - 1.5),
        egui::pos2(rect.right() - value_w - KNOB_R, rect.center().y + 1.5),
    );
    let (lo, hi) = (*range.start() as f32, *range.end() as f32);
    let mut changed = false;
    if resp.is_pointer_button_down_on() {
        if let Some(pos) = ui.ctx().input(|i| i.pointer.interact_pos()) {
            let t = ((pos.x - track.left()) / track.width().max(1.0)).clamp(0.0, 1.0);
            let raw = lo + (hi - lo) * t;
            let snapped = ((raw / step as f32).round() as i64 * step).clamp(*range.start(), *range.end());
            if snapped != *value {
                *value = snapped;
                changed = true;
            }
        }
    }
    let t = ((*value as f32 - lo) / (hi - lo).max(1.0)).clamp(0.0, 1.0);
    let p = ui.painter();
    p.rect_filled(track, 1.5_f32, pal.control_hi);
    p.rect_filled(
        Rect::from_min_size(track.min, Vec2::new(track.width() * t, track.height())),
        1.5_f32,
        pal.accent,
    );
    let knob_c = egui::pos2(track.left() + track.width() * t, track.center().y);
    let hot = resp.hovered() || resp.is_pointer_button_down_on();
    let knob_r = if hot { KNOB_R } else { KNOB_R - 1.0 };
    p.circle_filled(knob_c, knob_r, pal.bg);
    p.circle_stroke(knob_c, knob_r, Stroke::new(2.0_f32, pal.accent));
    p.text(
        egui::pos2(rect.right(), rect.center().y),
        egui::Align2::RIGHT_CENTER,
        format!("{value}{suffix}"),
        theme::mono(11.5),
        pal.text,
    );
    changed
}

/// Segmented control (settings tabs). Returns the index that is selected
/// after this frame's clicks.
pub fn segmented(ui: &mut egui::Ui, labels: &[&str], selected: usize, pal: &Palette) -> usize {
    let h = 28.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    ui.painter().rect_filled(rect, 9.0_f32, pal.control);
    let seg_w = rect.width() / labels.len() as f32;
    // The thumb slides between segments so switching tabs reads as motion,
    // not a repaint.
    let t = ui.ctx().animate_value_with_time(
        ui.id().with(("seg-thumb", labels.len())),
        selected as f32,
        0.14,
    );
    let thumb = Rect::from_min_size(
        egui::pos2(rect.left() + seg_w * t + 3.0, rect.top() + 3.0),
        Vec2::new(seg_w - 6.0, h - 6.0),
    );
    ui.painter().rect_filled(thumb, 7.0_f32, pal.control_hi);
    let mut out = selected;
    for (i, label) in labels.iter().enumerate() {
        let seg = Rect::from_min_size(
            egui::pos2(rect.left() + seg_w * i as f32, rect.top()),
            Vec2::new(seg_w, h),
        );
        let resp = ui.interact(seg, ui.id().with(("seg", i)), Sense::click());
        if resp.clicked() {
            out = i;
        }
        let col = if i == selected {
            pal.text
        } else if resp.hovered() {
            pal.muted
        } else {
            pal.faint
        };
        ui.painter().text(
            seg.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            theme::medium(12.0),
            col,
        );
    }
    out
}

/// Pill button. `primary` fills with the accent; otherwise it is a ghost
/// with a hairline border.
pub fn pill_button(
    ui: &mut egui::Ui,
    label: &str,
    primary: bool,
    enabled: bool,
    pal: &Palette,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(label.to_owned(), theme::medium(12.0), pal.text);
    let size = Vec2::new(galley.rect.width() + 26.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hot = enabled && resp.hovered();
    let p = ui.painter();
    let (fill, stroke, fg) = match (primary, enabled) {
        (true, true) => (
            if hot { theme::mix(pal.accent, Color32::WHITE, 0.14) } else { pal.accent },
            Color32::TRANSPARENT,
            theme::on_brand(pal.accent),
        ),
        (true, false) => (pal.control, Color32::TRANSPARENT, pal.faint),
        (false, true) => (
            if hot { pal.control_hi } else { Color32::TRANSPARENT },
            pal.border,
            if hot { pal.text } else { pal.muted },
        ),
        (false, false) => (Color32::TRANSPARENT, pal.border, pal.faint),
    };
    p.rect(rect, 8.0_f32, fill, Stroke::new(1.0_f32, stroke));
    p.galley(
        egui::pos2(
            rect.center().x - galley.rect.width() / 2.0,
            rect.center().y - galley.rect.height() / 2.0,
        ),
        ui.painter().layout_no_wrap(label.to_owned(), theme::medium(12.0), fg),
        fg,
    );
    resp
}

/// Small painted glyph button used for row actions — no font glyphs, so it
/// can never render as tofu. `mark` selects the shape.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Up,
    Down,
    Cross,
    Pencil,
}

pub fn glyph_button(
    ui: &mut egui::Ui,
    mark: Mark,
    enabled: bool,
    tip: &str,
    pal: &Palette,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::splat(22.0),
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let hot = enabled && resp.hovered();
    let p = ui.painter();
    if hot {
        p.rect_filled(rect, 6.0_f32, pal.control_hi);
    }
    let col = if !enabled {
        pal.faint.gamma_multiply(0.45)
    } else if hot {
        if mark == Mark::Cross { pal.bad } else { pal.text }
    } else {
        pal.muted
    };
    let c = rect.center();
    let s = Stroke::new(1.4_f32, col);
    match mark {
        Mark::Up | Mark::Down => {
            let dy = if mark == Mark::Up { 1.0 } else { -1.0 };
            p.add(Shape::line(
                vec![
                    egui::pos2(c.x - 4.0, c.y + 2.0 * dy),
                    egui::pos2(c.x, c.y - 2.0 * dy),
                    egui::pos2(c.x + 4.0, c.y + 2.0 * dy),
                ],
                s,
            ));
        }
        Mark::Cross => {
            p.line_segment([egui::pos2(c.x - 3.5, c.y - 3.5), egui::pos2(c.x + 3.5, c.y + 3.5)], s);
            p.line_segment([egui::pos2(c.x + 3.5, c.y - 3.5), egui::pos2(c.x - 3.5, c.y + 3.5)], s);
        }
        Mark::Pencil => {
            // nib on a shaft, drawn on the diagonal
            p.line_segment([egui::pos2(c.x - 4.0, c.y + 4.0), egui::pos2(c.x + 3.0, c.y - 3.0)], s);
            p.line_segment([egui::pos2(c.x + 2.0, c.y - 4.5), egui::pos2(c.x + 4.5, c.y - 2.0)], s);
            p.circle_filled(egui::pos2(c.x - 4.0, c.y + 4.0), 1.1, col);
        }
    }
    if enabled {
        resp.on_hover_text(tip)
    } else {
        resp
    }
}

/// Theme swatch chip: three gauge stops over the theme's own background, so
/// a theme is chosen by looking at it rather than by reading its name.
pub fn theme_swatch(ui: &mut egui::Ui, name: &str, selected: bool, pal: &Palette) -> egui::Response {
    let other = theme::palette(name);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(80.0, 46.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 9.0_f32, other.bg);
    let border = if selected {
        Stroke::new(1.6_f32, pal.accent)
    } else if resp.hovered() {
        Stroke::new(1.0_f32, other.muted)
    } else {
        Stroke::new(1.0_f32, other.border)
    };
    p.rect_stroke(rect, 9.0_f32, border);
    // three heat stops as a miniature gauge strip
    let strip = Rect::from_min_size(
        egui::pos2(rect.left() + 9.0, rect.top() + 11.0),
        Vec2::new(rect.width() - 18.0, 4.0),
    );
    let seg = strip.width() / 3.0;
    for (i, c) in [other.gauge[0], other.gauge[1], other.gauge[3]].iter().enumerate() {
        let r = Rect::from_min_size(
            egui::pos2(strip.left() + seg * i as f32, strip.top()),
            Vec2::new(seg - 3.0, strip.height()),
        );
        // Carry the theme's own bloom into the swatch: how lit a palette
        // looks is half of what is being chosen here.
        if other.glow > 0.0 {
            p.rect_filled(r.expand(3.0), 5.0_f32, c.gamma_multiply(0.18 * other.glow));
        }
        p.rect_filled(r, 2.0_f32, *c);
    }
    let label = elide(
        ui,
        name.split('-').next().unwrap_or(name),
        theme::medium(10.0),
        if selected { other.text } else { other.muted },
        rect.width() - 12.0,
    );
    p.galley(
        egui::pos2(rect.center().x - label.rect.width() / 2.0, rect.bottom() - 20.0),
        label,
        other.text,
    );
    resp
}

/// The little "opens elsewhere" arrow. Painted rather than typed: Inter has
/// no such glyph, and a tofu box next to a provider's name would be worse
/// than no affordance at all.
pub fn open_arrow(p: &egui::Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(1.3_f32, color);
    p.line_segment([egui::pos2(c.x - 3.0, c.y + 3.0), egui::pos2(c.x + 3.0, c.y - 3.0)], s);
    p.line_segment([egui::pos2(c.x - 0.5, c.y - 3.0), egui::pos2(c.x + 3.0, c.y - 3.0)], s);
    p.line_segment([egui::pos2(c.x + 3.0, c.y - 3.0), egui::pos2(c.x + 3.0, c.y + 0.5)], s);
}

/// A soft pulse on the gauge rim, marking a provider an agent is working
/// against right now. Deliberately unlike the alert badge: this is
/// information, not a problem, so it breathes rather than shouts, and sits on
/// the opposite corner so the two can coexist.
/// `phase` drives the breathe. `None` paints the settled mid-point of that
/// breathe instead — the same dot, holding still — so the caller can stop
/// asking for frames when nobody is watching the notch.
pub fn live_pulse(ui: &egui::Ui, center: Pos2, r: f32, phase: Option<f32>, color: Color32, alpha: f32) {
    let breathe = match phase {
        Some(t) => 0.55 + 0.45 * (t * std::f32::consts::TAU).sin(),
        // The DC term of the sine above: what the breathe averages out to.
        None => 0.55,
    };
    let p = ui.painter();
    p.circle_filled(center, r + 2.0, color.gamma_multiply(0.22 * breathe * alpha));
    p.circle_filled(center, r, color.gamma_multiply((0.65 + 0.35 * breathe) * alpha));
}

/// Attention badge pinned to the rim of a gauge: a filled disc carrying a
/// hand-painted exclamation mark, knocked out of the ring beneath it by a dark
/// rim so it reads cleanly on top of the arc. This is how a rail cell reports
/// trouble when its percentage label is switched off — the mark is painted
/// rather than typed so it can never depend on a font's glyph coverage.
pub fn alert_badge(ui: &egui::Ui, center: Pos2, r: f32, color: Color32, rim: Color32, alpha: f32) {
    let p = ui.painter();
    p.circle_filled(center, r + 1.6, rim.linear_multiply(alpha));
    p.circle_filled(center, r, color.linear_multiply(alpha));
    let fg = theme::on_brand(color).linear_multiply(alpha);
    let w = (r * 0.34).max(1.6);
    p.rect_filled(
        Rect::from_min_max(
            egui::pos2(center.x - w / 2.0, center.y - r * 0.58),
            egui::pos2(center.x + w / 2.0, center.y + r * 0.12),
        ),
        w / 2.0,
        fg,
    );
    p.circle_filled(egui::pos2(center.x, center.y + r * 0.46), w / 2.0, fg);
}

/// Placeholder text takes its colour from the *base* text colour, while a
/// field's value is coloured explicitly — so dimming the base for the duration
/// of the widget separates a hint from a real value. Without this they render
/// at almost the same brightness and a hint reads as a filled-in path.
fn with_dim_hint<R>(ui: &mut egui::Ui, pal: &Palette, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let prev = ui.style().visuals.override_text_color;
    ui.style_mut().visuals.override_text_color = Some(pal.faint);
    let out = add(ui);
    ui.style_mut().visuals.override_text_color = prev;
    out
}

/// Single-line text field styled like the rest of the sheet. Returns true
/// while the value changed this frame.
pub fn text_field(ui: &mut egui::Ui, value: &mut String, hint: &str, pal: &Palette) -> bool {
    let h = 28.0;
    let w = ui.available_width().max(1.0);
    with_dim_hint(ui, pal, |ui| {
        ui.add_sized(
            Vec2::new(w, h),
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .margin(egui::Margin::symmetric(9.0, 6.0))
                .font(theme::sans(11.5))
                .text_color(pal.text),
        )
        .changed()
    })
}

/// A labelled field whose value is masked until asked for. Anything secret
/// should be unreadable over a shoulder by default; the toggle is right where
/// the eye already is, on the label line.
pub fn secret_field(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    value: &mut String,
    revealed: &mut bool,
    pal: &Palette,
) -> bool {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(1.0), 14.0), Sense::hover());
    let col = theme::mix(pal.faint, pal.muted, 0.55);
    let g = ui.painter().layout_job(theme::caps_job(label, 9.0, col));
    ui.painter().galley(egui::pos2(r.left() + 2.0, r.top()), g, col);
    if !value.is_empty() {
        let word = if *revealed { "hide" } else { "show" };
        let g = ui.painter().layout_job(theme::caps_job(word, 9.0, pal.accent));
        let w = g.rect.width();
        let hit = Rect::from_min_size(egui::pos2(r.right() - w - 6.0, r.top() - 3.0), Vec2::new(w + 12.0, 18.0));
        if ui.interact(hit, ui.id().with(("reveal", label)), Sense::click()).clicked() {
            *revealed = !*revealed;
        }
        ui.painter().galley(egui::pos2(r.right() - w, r.top()), g, pal.accent);
    }
    ui.add_space(4.0);
    let h = 28.0;
    let full = ui.available_width().max(1.0);
    let changed = with_dim_hint(ui, pal, |ui| {
        ui.add_sized(
            Vec2::new(full, h),
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .password(!*revealed)
                .margin(egui::Margin::symmetric(9.0, 6.0))
                .font(theme::sans(11.5))
                .text_color(pal.text),
        )
        .changed()
    });
    ui.add_space(10.0);
    changed
}

/// A labelled field: caps label over an input. The label is what makes a form
/// scannable at a glance; the hint inside the box says what a good value looks
/// like.
pub fn field(
    ui: &mut egui::Ui,
    label: &str,
    hint: &str,
    value: &mut String,
    pal: &Palette,
) -> bool {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(1.0), 13.0), Sense::hover());
    let col = theme::mix(pal.faint, pal.muted, 0.55);
    let g = ui.painter().layout_job(theme::caps_job(label, 9.0, col));
    ui.painter().galley(egui::pos2(r.left() + 2.0, r.top()), g, col);
    ui.add_space(4.0);
    let changed = text_field(ui, value, hint, pal);
    ui.add_space(10.0);
    changed
}
