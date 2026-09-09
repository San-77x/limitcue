//! Composite UI pieces built from the primitives in `widgets`:
//! collapsed-pill chips, detail cards, tooltips.

use std::collections::HashMap;

use eframe::egui::{self, Color32, RichText, Vec2};
use eframe::egui::text::{LayoutJob, TextFormat};

use crate::types::{fmt_countdown, now_unix, Fidelity, Reading, Snapshot, Window};

pub mod theme;
pub mod widgets;

use theme::Palette;
use widgets::{badge, bar, dot, logo_ring, pct_color};

/// Short uppercase tag for the collapsed pill.
pub fn tag(id: &str) -> String {
    match id {
        "claude" => "CLAUDE".into(),
        "codex" => "CODEX".into(),
        "minimax" => "M3".into(),
        other => other.to_uppercase(),
    }
}

/// Chip label: TAG + percent, tabular mono, percent colored by severity.
/// `pct` is passed in (may be mid-animation); `None` renders `…`/status.
pub fn chip_job(s: &Snapshot, pct: Option<f64>, pal: &Palette, alpha: f32, stale: bool) -> LayoutJob {
    let ok = matches!(s.reading, Reading::Ok { .. });
    let label = match (&s.reading, pct) {
        (Reading::Ok { .. }, Some(p)) => format!("{p:.0}%"),
        (Reading::Ok { .. }, None) => "…".into(),
        (Reading::NeedsAuth(_), _) => "auth".into(),
        (Reading::Error(_), _) => "err".into(),
        _ => "?".into(),
    };
    let label_col = match &s.reading {
        Reading::NeedsAuth(_) => pal.warn,
        Reading::Error(_) => pal.bad,
        _ => pct_color(pct, ok, pal),
    };
    let tag_col = if stale { theme::mix(pal.text, pal.faint, 0.45) } else { pal.text };
    let mut job = LayoutJob::default();
    job.append(
        &tag(&s.provider_id),
        0.0,
        TextFormat::simple(theme::mono(11.5), tag_col.linear_multiply(alpha)),
    );
    job.append(
        &label,
        5.0,
        TextFormat::simple(theme::mono(12.0), label_col.linear_multiply(alpha)),
    );
    job
}

/// Width of the chip label text (TAG + percent) at current fonts.
/// Stale tinting doesn't change metrics, so widths are always computed fresh.
pub fn chip_text_w(ctx: &egui::Context, s: &Snapshot, pct: Option<f64>, pal: &Palette, alpha: f32) -> f32 {
    ctx.fonts(|f| f.layout_job(chip_job(s, pct, pal, alpha, false)).rect.width())
}

/// Total width of a collapsed chip: hover padding + ring + gap + text.
pub const CHIP_RING_D: f32 = 19.0; // ring diameter on the collapsed pill

pub fn chip_width(text_w: f32) -> f32 {
    10.0 + CHIP_RING_D + 5.0 + text_w + 6.0
}

/// Draw one collapsed chip into `rect` (already sized via [`chip_width`]).
/// `hovered` paints the chip's hover background; `logos` carries the brand
/// marks (missing entries fall back to the monogram letter).
#[allow(clippy::too_many_arguments)]
pub fn draw_chip(
    ui: &egui::Ui,
    rect: eframe::egui::Rect,
    s: &Snapshot,
    pct: Option<f64>,
    pal: &Palette,
    alpha: f32,
    stale: bool,
    hovered: bool,
    logos: &HashMap<String, egui::TextureHandle>,
) {
    let ok = matches!(s.reading, Reading::Ok { .. });
    let p = ui.painter();
    if hovered {
        p.rect_filled(rect, 8.0_f32, pal.card_hover);
    }
    let cy = rect.center().y;
    let frac = (pct.unwrap_or(0.0) / 100.0) as f32;
    let ring_col = if stale { theme::mix(pct_color(pct, ok, pal), pal.stale, 0.6) } else { pct_color(pct, ok, pal) };
    logo_ring(
        ui,
        egui::pos2(rect.left() + 10.0 + CHIP_RING_D / 2.0, cy),
        CHIP_RING_D / 2.0,
        logos.get(&s.provider_id),
        &theme::monogram(&s.provider_id),
        theme::brand(&s.provider_id, pal),
        frac,
        ring_col,
        pal,
        alpha,
    );
    let galley = p.layout_job(chip_job(s, pct, pal, alpha, stale));
    p.galley(
        egui::pos2(
            rect.left() + 10.0 + CHIP_RING_D + 5.0,
            cy - galley.rect.height() / 2.0,
        ),
        galley,
        pal.text,
    );
}

/// Usage card shown beside the side-rail strip when a provider row is
/// hovered: bare white mark + "<Provider> Usage" on top, then one block per
/// window — label (left) + human reset copy (right), a 6px heat bar showing
/// the share used, and "N% Used" underneath, hairline-separated. The tail
/// triangle is drawn by the caller (it needs the hovered row's offset).
#[allow(clippy::too_many_arguments)]
pub fn rail_card(
    ui: &mut egui::Ui,
    s: &Snapshot,
    pal: &Palette,
    alpha: f32,
    stale: bool,
    logos: &HashMap<String, egui::TextureHandle>,
    now: u64,
    width: f32,
) {
    let hairline = Color32::from_white_alpha(14);
    egui::Frame::none()
        .fill(pal.rail_deep.linear_multiply(alpha))
        .stroke(egui::Stroke::new(1.0_f32, hairline.linear_multiply(alpha)))
        .rounding(egui::Rounding::same(18.0))
        .inner_margin(egui::Margin::symmetric(16.0, 14.0))
        .show(ui, |ui| {
            ui.set_width(width - 32.0);
            // header: white mark + "<Provider> Usage"
            let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), egui::Sense::hover());
            let p = ui.painter();
            match logos.get(&s.provider_id) {
                Some(tex) => {
                    p.image(
                        tex.id(),
                        eframe::egui::Rect::from_center_size(egui::pos2(hr.left() + 8.0, hr.center().y), Vec2::splat(15.0)),
                        eframe::egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        Color32::WHITE.linear_multiply(alpha),
                    );
                }
                None => {
                    p.text(
                        egui::pos2(hr.left() + 8.0, hr.center().y),
                        egui::Align2::CENTER_CENTER,
                        theme::monogram(&s.provider_id),
                        theme::semibold(13.0),
                        Color32::WHITE.linear_multiply(alpha),
                    );
                }
            }
            let name_col = if stale { theme::mix(pal.text, pal.faint, 0.4) } else { Color32::WHITE };
            p.text(
                egui::pos2(hr.left() + 22.0, hr.center().y),
                egui::Align2::LEFT_CENTER,
                format!("{} Usage", s.display_name),
                theme::semibold(13.5),
                name_col.linear_multiply(alpha),
            );
            ui.add_space(10.0);
            match &s.reading {
                Reading::Ok { windows, .. } => {
                    for (i, w) in windows.iter().enumerate() {
                        if i > 0 {
                            // hairline separator between windows
                            ui.add_space(10.0);
                            let (lr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
                            ui.painter().rect_filled(lr, 0.0_f32, hairline.linear_multiply(alpha));
                            ui.add_space(10.0);
                        }
                        let used01 = (w.remaining_percent.map(|p| 1.0 - p / 100.0).unwrap_or(0.0)) as f32;
                        let heat = theme::heat(used01, pal);
                        // label (left) + reset copy (right)
                        let (r, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 16.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().text(
                            egui::pos2(r.left(), r.center().y),
                            egui::Align2::LEFT_CENTER,
                            &w.label,
                            theme::medium(12.0),
                            pal.text.linear_multiply(alpha),
                        );
                        if let Some(t) = w.resets_at {
                            ui.painter().text(
                                egui::pos2(r.right(), r.center().y),
                                egui::Align2::RIGHT_CENTER,
                                crate::types::fmt_reset(t, now),
                                theme::sans(10.5),
                                pal.muted.linear_multiply(alpha),
                            );
                        }
                        // 6px heat bar (share used) under the label row
                        ui.add_space(6.0);
                        let (br, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 6.0), egui::Sense::hover());
                        bar(ui, br, used01, heat, pal, alpha);
                        ui.add_space(5.0);
                        let (ur, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 15.0), egui::Sense::hover());
                        ui.painter().text(
                            egui::pos2(ur.left(), ur.center().y),
                            egui::Align2::LEFT_CENTER,
                            match w.remaining_percent {
                                Some(p) => format!("{:.0}% Used", 100.0 - p),
                                None => "no reading".into(),
                            },
                            theme::medium(11.5),
                            heat.linear_multiply(alpha),
                        );
                    }
                }
                Reading::NeedsAuth(m) | Reading::Error(m) => {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                        let (col, tip) = match &s.reading {
                            Reading::NeedsAuth(_) => (pal.warn, "credentials need attention"),
                            _ => (pal.bad, "last fetch failed"),
                        };
                        dot(ui, r.center(), col, alpha);
                        ui.label(RichText::new(m.clone()).color(pal.muted.linear_multiply(alpha)).size(11.0))
                            .on_hover_text(tip);
                    });
                }
                Reading::NotConfigured => {
                    ui.add_space(2.0);
                    ui.label(RichText::new("not configured").color(pal.faint.linear_multiply(alpha)).size(11.0));
                }
            }
            ui.add_space(10.0);
            let age = now.saturating_sub(s.fetched_at);
            let mut age_line = format!("Updated {} ago", fmt_countdown(age));
            if stale {
                age_line.insert_str(0, "Stale \u{b7} ");
            }
            ui.label(RichText::new(age_line).color(pal.faint.linear_multiply(alpha)).size(9.5));
        });
}

/// Height of the [`rail_card`] layout for one provider (frame margins +
/// header + one block per window with hairline separators, or the
/// error/auth row).
pub fn rail_card_height(s: Option<&Snapshot>) -> f32 {
    let Some(s) = s else { return 120.0 };
    let rows = match &s.reading {
        Reading::Ok { windows, .. } => windows.len().max(1) as f32,
        _ => 1.0,
    };
    // margins 28 + header 18 + 10 + rows*(16+6+6+5+15) + (rows-1)*21 + 10 + footer 14
    28.0 + 18.0 + 10.0 + rows * 48.0 + (rows - 1.0) * 21.0 + 10.0 + 14.0
}

/// Hover tooltip with the full per-window breakdown for one provider.
pub fn chip_tooltip(ui: &mut egui::Ui, s: &Snapshot, pal: &Palette, stale: bool) {
    ui.strong(RichText::new(s.display_name.clone()));
    match &s.reading {
        Reading::Ok { windows, .. } => {
            for w in windows {
                let pctt = w.remaining_percent.map(|p| format!("{p:.0}% left")).unwrap_or_default();
                let reset = w
                    .resets_at
                    .map(|t| fmt_countdown(t.saturating_sub(now_unix())))
                    .map(|c| format!(" · resets in {c}"))
                    .unwrap_or_default();
                ui.label(format!("{}: {pctt}{reset}", w.label));
            }
        }
        Reading::NeedsAuth(m) => {
            ui.colored_label(pal.warn, format!("needs auth: {m}"));
        }
        Reading::Error(m) => {
            ui.colored_label(pal.bad, m.clone());
        }
        Reading::NotConfigured => {
            ui.label("not configured");
        }
    }
    let age = now_unix().saturating_sub(s.fetched_at);
    let mut age_line = format!("updated {} ago", fmt_countdown(age));
    if stale {
        age_line.push_str(" · stale");
    }
    ui.colored_label(pal.faint, age_line);
}

/// Right-aligned value column for one window row: `63% · 12/30 · in 2h 41m`.
fn value_job(w: &Window, pal: &Palette, alpha: f32, now: u64) -> LayoutJob {
    let mut job = LayoutJob::default();
    if let Some(p) = w.remaining_percent {
        job.append(
            &format!("{p:.0}%"),
            0.0,
            TextFormat::simple(theme::mono(11.5), pct_color(w.remaining_percent, true, pal).linear_multiply(alpha)),
        );
    }
    if let (Some(a), Some(b)) = (w.remaining_count, w.total_count) {
        if b > 0 {
            job.append(
                &format!(" {a}/{b}"),
                4.0,
                TextFormat::simple(theme::mono(11.5), pal.muted.linear_multiply(alpha)),
            );
        }
    }
    if let Some(t) = w.resets_at {
        job.append(
            &format!(" · in {}", fmt_countdown(t.saturating_sub(now))),
            0.0,
            TextFormat::simple(theme::mono(11.5), pal.faint.linear_multiply(alpha)),
        );
    }
    job
}

fn fidelity_badge(ui: &mut egui::Ui, s: &Snapshot, pal: &Palette, alpha: f32) {
    let (text, color, explain) = match s.fidelity {
        Fidelity::Official => ("official", pal.ok, "the provider's own endpoint"),
        Fidelity::Derived => ("derived", pal.warn, "reverse-engineered endpoint — may break without notice"),
        Fidelity::Manual => ("manual", pal.muted, "user-configured source"),
    };
    badge(ui, text, color, alpha).on_hover_text(explain);
}

/// One provider's detail card. Hover lifts the fill (via previous-frame response).
/// `pct` may be mid-animation (tweened ring/header), windows always show real data.
#[allow(clippy::too_many_arguments)]
pub fn provider_card(
    ui: &mut egui::Ui,
    s: &Snapshot,
    pct: Option<f64>,
    pal: &Palette,
    alpha: f32,
    stale: bool,
    logos: &HashMap<String, egui::TextureHandle>,
) {
    let card_id = ui.id().with(("card", &s.provider_id));
    let hovered = ui.ctx().read_response(card_id).map(|r| r.hovered()).unwrap_or(false);
    egui::Frame::none()
        .fill(if hovered { pal.card_hover } else { pal.card })
        .stroke(egui::Stroke::new(
            1.0_f32,
            if hovered { pal.accent.linear_multiply(0.42) } else { pal.border },
        ))
        .rounding(egui::Rounding::same(13.0))
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            let now = now_unix();
            let ok = matches!(s.reading, Reading::Ok { .. });
            let frac = (pct.unwrap_or(0.0) / 100.0) as f32;
            let ring_col = if stale {
                theme::mix(pct_color(pct, ok, pal), pal.stale, 0.6)
            } else {
                pct_color(pct, ok, pal)
            };

            // header: logo (or monogram) + name + fidelity badge · age
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::hover());
                logo_ring(
                    ui,
                    r.center(),
                    9.0,
                    logos.get(&s.provider_id),
                    &theme::monogram(&s.provider_id),
                    theme::brand(&s.provider_id, pal),
                    frac,
                    ring_col,
                    pal,
                    alpha,
                );
                ui.add_space(1.0);
                let name_col = if stale { theme::mix(pal.text, pal.faint, 0.4) } else { pal.text };
                ui.label(RichText::new(&s.display_name).color(name_col.linear_multiply(alpha)).size(13.0));
                fidelity_badge(ui, s, pal, alpha);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let age = now.saturating_sub(s.fetched_at);
                    let mut line = format!("{} ago", fmt_countdown(age));
                    if stale {
                        line.insert_str(0, "stale · ");
                    }
                    ui.label(RichText::new(line).color(pal.faint.linear_multiply(alpha)).size(10.5).monospace());
                });
            });

            ui.add_space(2.0);

            match &s.reading {
                Reading::Ok { windows, .. } => {
                    for w in windows {
                        ui.add_space(3.0);
                        let (r, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 16.0),
                            egui::Sense::hover(),
                        );
                        // window label (fixed column, truncated)
                        ui.put(
                            egui::Rect::from_min_size(egui::pos2(r.min.x, r.center().y - 8.0), Vec2::new(96.0, 16.0)),
                            egui::Label::new(
                                RichText::new(&w.label).color(pal.muted.linear_multiply(alpha)).size(11.5),
                            )
                            .selectable(false)
                            .truncate(),
                        );
                        // bar between label and value columns
                        let bar_rect = eframe::egui::Rect::from_min_max(
                            egui::pos2(r.min.x + 104.0, r.center().y - 3.0),
                            egui::pos2((r.right() - 140.0).max(r.min.x + 180.0), r.center().y + 3.0),
                        );
                        if bar_rect.width() >= 30.0 {
                            bar(
                                ui,
                                bar_rect,
                                (w.remaining_percent.unwrap_or(0.0) / 100.0) as f32,
                                pct_color(w.remaining_percent, true, pal),
                                pal,
                                alpha,
                            );
                        }
                        // right-aligned tabular value
                        let galley = ui.painter().layout_job(value_job(w, pal, alpha, now));
                        ui.painter().galley(
                            egui::pos2(r.right() - galley.rect.width(), r.center().y - galley.rect.height() / 2.0),
                            galley,
                            pal.text,
                        );
                    }
                }
                Reading::NeedsAuth(m) | Reading::Error(m) => {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), egui::Sense::hover());
                        let (col, tip) = match &s.reading {
                            Reading::NeedsAuth(_) => (pal.warn, "credentials need attention"),
                            _ => (pal.bad, "last fetch failed"),
                        };
                        dot(ui, r.center(), col, alpha);
                        ui.label(RichText::new(m.clone()).color(pal.muted.linear_multiply(alpha)).size(11.5))
                            .on_hover_text(tip);
                    });
                }
                Reading::NotConfigured => {
                    ui.add_space(2.0);
                    ui.label(RichText::new("not configured").color(pal.faint.linear_multiply(alpha)).size(11.5));
                }
            }

            // hover region for the whole card (fill is chosen next frame)
            ui.interact(ui.max_rect(), card_id, egui::Sense::hover());
        });
}
