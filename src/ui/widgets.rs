//! Hand-drawn widgets: ring gauges, monogram discs, quota bars, badges,
//! icon buttons. Everything is painted directly for full control.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke, Vec2};

use super::theme::{self, Palette};

/// Percent → semantic color (`ok` = the reading is usable at all).
pub fn pct_color(pct: Option<f64>, ok: bool, pal: &Palette) -> Color32 {
    if !ok {
        return pal.stale;
    }
    match pct {
        None => pal.muted,
        Some(p) if p > 50.0 => pal.ok,
        Some(p) if p > 15.0 => pal.warn,
        Some(_) => pal.bad,
    }
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
    let steps = ((36.0 * frac) as usize).max(2);
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
        Color32::WHITE.linear_multiply(alpha),
    );
    ring(ui, center, r, stroke, frac, color, pal, alpha);
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

/// Concave fillet where the rail body meets a screen edge: a filled corner
/// whose hypotenuse curves inward (quarter-circle of radius `r`), so the pill
/// reads as flaring out of the bezel. `alpha` tweens it in/out with the size
/// animation. `along` and `away` are signed unit directions: `along` points
/// from the corner along the screen edge into the pill, `away` points from
/// the corner into the pill (perpendicular to the edge).
pub fn edge_flare(ui: &egui::Ui, corner: Pos2, along: f32, away: f32, r: f32, color: Color32, alpha: f32) {
    if alpha <= 0.02 || r <= 0.5 {
        return;
    }
    let col = color.linear_multiply(alpha);
    let p = ui.painter();
    // Quarter circle: center at corner + along*r + away*r, radius r. The arc
    // runs from the point r·along off the center to the point r·away off it,
    // bulging toward the corner; sample angles between those two endpoints
    // through the quadrant that keeps the arc on the corner side.
    let cx = corner.x + along * r;
    let cy = corner.y + away * r;
    let ang_a = if along > 0.0 { 0.0 } else { std::f32::consts::PI };
    let ang_b = if away > 0.0 { std::f32::consts::FRAC_PI_2 } else { -std::f32::consts::FRAC_PI_2 };
    let mut d = ang_b - ang_a;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    let steps = 8;
    let mut pts = Vec::with_capacity(steps + 2);
    for i in 0..=steps {
        let ang = ang_a + d * (i as f32 / steps as f32);
        pts.push(egui::pos2(cx + r * ang.cos(), cy + r * ang.sin()));
    }
    pts.push(corner);
    p.add(Shape::convex_polygon(pts, col, Stroke::NONE));
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
        let base = if hovered { Color32::WHITE } else { Color32::from_gray(215) };
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
