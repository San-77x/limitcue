//! Composite UI pieces built from the primitives in `widgets`: the notch's
//! hover usage card.

use std::collections::HashMap;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, Color32, Vec2};

use crate::types::{fmt_countdown, Fidelity, Reading, Snapshot, Window};

pub mod theme;
pub mod widgets;

use theme::Palette;

// ===========================================================================
// The hover usage card.
//
// One glass panel that answers three questions in reading order: how much of
// this provider is gone (the hero), where it went (the window list), and how
// fresh the number is (the footer). Every measurement below is shared by the
// painter and by `rail_card_content_height`, so the card can never lay out
// taller than the rect it was handed — the previous version budgeted its
// height separately and clipped its own footer.
// ===========================================================================

pub const CARD_CORNER: f32 = 16.0;
const PAD_X: f32 = 16.0;
const PAD_TOP: f32 = 14.0;
const PAD_BOT: f32 = 12.0;
const HEAD_H: f32 = 18.0; // provider mark + name + fidelity
const HEAD_GAP: f32 = 13.0;
const HERO_H: f32 = 34.0; // headline percentage + which window it refers to
const HERO_GAP: f32 = 13.0;
/// The burn-rate line, when there is enough history to draw one.
const PACE_H: f32 = 15.0;
/// The "couldn't refresh" line, on the pace line's metrics so the two stack
/// without the card looking assembled from different kits.
const NOTE_H: f32 = PACE_H;
const PACE_GAP: f32 = 11.0;
const RULE_GAP: f32 = 12.0; // rule → first window row
const ROW_H: f32 = 47.0; // label line + meter + meta line
const ROW_GAP: f32 = 15.0;
const FOOT_GAP: f32 = 12.0; // last row → footer rule
const FOOT_RULE_GAP: f32 = 10.0;
const FOOT_H: f32 = 13.0;
/// Height of the lone meter a single-window provider gets instead of a list.
const SOLO_METER_H: f32 = 6.0;
/// A status message stands in for the window list on non-Ok readings.
const STATUS_BODY_H: f32 = 34.0;
/// Vertical clearance between the hovered row's centre line and the card.
pub const RAIL_CARD_GAP_Y: f32 = 8.0;

/// Chrome above the body: padding, header, hero.
const CARD_TOP_H: f32 = PAD_TOP + HEAD_H + HEAD_GAP + HERO_H + HERO_GAP;
/// Chrome below the body: footer rule, footer line, padding.
const CARD_BOTTOM_H: f32 = FOOT_GAP + 1.0 + FOOT_RULE_GAP + FOOT_H + PAD_BOT;
/// The separating rule and the gap under it, drawn only above a list.
const CARD_RULE_H: f32 = 1.0 + RULE_GAP;
/// Everything except a scrolling list.
const CARD_FIXED_H: f32 = CARD_TOP_H + CARD_RULE_H + CARD_BOTTOM_H;

/// What fills the middle of the card.
enum Body {
    /// One window: the hero already names it and says when it resets, so all
    /// that is left to show is the bar. A one-row list under a hero that says
    /// the same thing reads as a bug, not as detail.
    Solo(f32),
    /// Several windows: one row each, scrolling if the screen is too short.
    Rows(usize),
    /// Nothing to chart — a sign-in prompt, an error, an unconfigured provider.
    Status,
}

fn body(s: &Snapshot) -> Body {
    match &s.reading {
        Reading::Ok { windows, .. } => match windows.len() {
            0 => Body::Status,
            1 => Body::Solo(
                windows[0]
                    .remaining_percent
                    .map(|p| 1.0 - p / 100.0)
                    .unwrap_or(0.0) as f32,
            ),
            n => Body::Rows(n),
        },
        _ => Body::Status,
    }
}

/// Height the card wants when nothing constrains it.
pub fn rail_card_content_height(s: &Snapshot, pace: bool) -> f32 {
    // Every line that can appear above the body has to be counted here as
    // well as painted below. Budget and painter disagreeing is what clipped
    // the footer the last time this card grew a line.
    let extra = if pace { PACE_H + PACE_GAP } else { 0.0 }
        + if s.fetch_error.is_some() {
            NOTE_H + PACE_GAP
        } else {
            0.0
        };
    extra
        + match body(s) {
            Body::Solo(_) => CARD_TOP_H + SOLO_METER_H + CARD_BOTTOM_H,
            Body::Rows(n) => CARD_FIXED_H + n as f32 * ROW_H + (n as f32 - 1.0) * ROW_GAP + 3.0,
            Body::Status => CARD_FIXED_H + STATUS_BODY_H,
        }
}

pub fn rail_card_height(s: &Snapshot, pace: bool) -> f32 {
    rail_card_content_height(s, pace)
}

/// Index of the window closest to exhaustion — the one the hero reports.
fn peak_window(windows: &[Window]) -> Option<usize> {
    windows
        .iter()
        .enumerate()
        .filter_map(|(i, w)| w.remaining_percent.map(|p| (i, p)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(i, _)| i)
}

/// Draw the whole card — surface included — into `rect`.
#[allow(clippy::too_many_arguments)]
pub fn rail_card(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    s: &Snapshot,
    pal: &Palette,
    alpha: f32,
    stale: bool,
    logos: &HashMap<String, egui::TextureHandle>,
    now: u64,
    list_height: f32,
    opacity: f32,
    pace: Option<&str>,
    // Whether the header should offer a link to the provider's dashboard.
    console: bool,
) -> bool {
    let a = alpha.clamp(0.0, 1.0);
    let dim = |c: Color32| c.linear_multiply(a);
    let p = ui.painter().clone();

    // ---- glass surface ---------------------------------------------------
    // Shadow first (it must sit under the fill), then the translucent body,
    // a hairline edge, and a specular lip along the top so the panel reads as
    // glass lifted off the desktop rather than a flat rectangle.
    p.add(
        egui::epaint::Shadow {
            offset: [0, 10],
            blur: 30,
            spread: 0,
            color: Color32::from_black_alpha(132).linear_multiply(a),
        }
        .as_shape(rect, egui::CornerRadius::same(CARD_CORNER as u8)),
    );
    p.rect_filled(
        rect,
        CARD_CORNER,
        dim(theme::at_opacity(pal.rail_deep, opacity)),
    );
    // No outline: the shadow already separates the panel from the desktop, and
    // a hairline ring around a translucent surface reads as a seam. The sheen
    // below is a highlight along the top lip, not a border.
    widgets::sheen(&p, rect, CARD_CORNER, dim(pal.sheen));

    let x0 = rect.left() + PAD_X;
    let x1 = rect.right() - PAD_X;
    let w = x1 - x0;
    let mut y = rect.top() + PAD_TOP;

    // ---- header: mark · name · provenance --------------------------------
    let head = egui::Rect::from_min_size(egui::pos2(x0, y), Vec2::new(w, HEAD_H));
    match logos.get(&s.provider_id) {
        Some(tex) => {
            p.image(
                tex.id(),
                egui::Rect::from_center_size(
                    egui::pos2(x0 + 7.5, head.center().y),
                    Vec2::splat(15.0),
                ),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                dim(pal.ink),
            );
        }
        None => {
            p.text(
                egui::pos2(x0 + 7.5, head.center().y),
                egui::Align2::CENTER_CENTER,
                theme::monogram(&s.provider_id),
                theme::semibold(13.0),
                dim(pal.ink),
            );
        }
    }
    // Provenance sits right-aligned in the header: it qualifies the whole
    // card, so it belongs next to the name, not buried under the numbers.
    let (mark_text, mark_col) = if stale {
        ("stale".to_string(), pal.stale)
    } else {
        (
            match s.fidelity {
                Fidelity::Official => "official",
                Fidelity::Derived => "derived",
                Fidelity::Manual => "manual",
            }
            .to_string(),
            pal.faint,
        )
    };
    let mark = p.layout_job(theme::caps_job(&mark_text, 9.0, dim(mark_col)));
    let mark_w = mark.rect.width();
    p.galley(
        egui::pos2(x1 - mark_w, head.center().y - mark.rect.height() / 2.0),
        mark,
        mark_col,
    );
    let name_col = if stale {
        theme::mix(pal.ink, pal.faint, 0.4)
    } else {
        pal.ink
    };
    let arrow_w = if console { 18.0 } else { 0.0 };
    let name = widgets::elide(
        ui,
        &s.display_name,
        theme::semibold(13.5),
        dim(name_col),
        w - 22.0 - mark_w - 10.0 - arrow_w,
    );
    let name_w = name.rect.width();
    p.galley(
        egui::pos2(x0 + 22.0, head.center().y - name.rect.height() / 2.0),
        name,
        name_col,
    );
    // A quota you have run out of usually ends with a trip to the provider's
    // own page; the name is the obvious thing to click for that.
    let mut console_clicked = false;
    if console {
        let hit = egui::Rect::from_min_size(
            egui::pos2(x0 + 18.0, head.top() - 2.0),
            Vec2::new(name_w + 4.0 + arrow_w, head.height() + 4.0),
        );
        let resp = ui
            .interact(
                hit,
                ui.id().with(("console", &s.provider_id)),
                egui::Sense::click(),
            )
            .on_hover_text("Open this provider's dashboard");
        console_clicked = resp.clicked();
        let c = egui::pos2(x0 + 31.0 + name_w, head.center().y - 1.0);
        let col = if resp.hovered() {
            pal.accent
        } else {
            pal.faint
        };
        widgets::open_arrow(&p, c, dim(col));
    }
    y = head.bottom() + HEAD_GAP;

    // ---- hero ------------------------------------------------------------
    let hero = egui::Rect::from_min_size(egui::pos2(x0, y), Vec2::new(w, HERO_H));
    match &s.reading {
        Reading::Ok { windows, .. } => {
            let peak = peak_window(windows);
            let used = peak
                .and_then(|i| windows[i].remaining_percent)
                .map(|p| 100.0 - p);
            let heat = used
                .map(|u| theme::heat((u / 100.0) as f32, pal))
                .unwrap_or(pal.muted);
            let heat = if stale {
                theme::mix(heat, pal.stale, 0.55)
            } else {
                heat
            };
            // "100% used" on one baseline: the count carries the weight, the
            // unit and the verb stay quiet.
            let num = used
                .map(|u| format!("{u:.0}"))
                .unwrap_or_else(|| "—".into());
            let g_num = p.layout_no_wrap(num, theme::semibold(29.0), dim(heat));
            let num_bottom = hero.top() + g_num.rect.height();
            p.galley(egui::pos2(x0, hero.top()), g_num.clone(), heat);
            let mut cursor = x0 + g_num.rect.width() + 2.0;
            if used.is_some() {
                let g_sym = p.layout_no_wrap(
                    "%".into(),
                    theme::medium(15.0),
                    dim(heat.gamma_multiply(0.8)),
                );
                p.galley(
                    egui::pos2(cursor, num_bottom - g_sym.rect.height() - 4.0),
                    g_sym.clone(),
                    heat,
                );
                cursor += g_sym.rect.width() + 6.0;
            }
            let g_used = p.layout_job(theme::caps_job("used", 9.5, dim(pal.faint)));
            p.galley(
                egui::pos2(cursor, num_bottom - g_used.rect.height() - 5.0),
                g_used.clone(),
                pal.faint,
            );
            let left_w = cursor + g_used.rect.width() - x0;

            // Right column names the window the hero is about, and when it
            // comes back — the two facts that make the big number actionable.
            if let Some(i) = peak {
                let avail = (w - left_w - 14.0).max(60.0);
                let label = widgets::elide(
                    ui,
                    &windows[i].label,
                    theme::medium(11.5),
                    dim(pal.text),
                    avail,
                );
                p.galley(
                    egui::pos2(x1 - label.rect.width(), hero.top() + 2.0),
                    label,
                    pal.text,
                );
                let reset = windows[i]
                    .resets_at
                    .map(|t| crate::types::fmt_reset(t, now))
                    .unwrap_or_else(|| "No reset time".into());
                let reset = widgets::elide(ui, &reset, theme::sans(10.5), dim(pal.muted), avail);
                p.galley(
                    egui::pos2(x1 - reset.rect.width(), hero.top() + 18.0),
                    reset,
                    pal.muted,
                );
            }
        }
        reading => {
            let (title, col) = match reading {
                Reading::NeedsAuth(_) => ("Sign-in needed", pal.warn),
                Reading::Error(_) => ("Update failed", pal.bad),
                _ => ("Not configured", pal.muted),
            };
            p.circle_filled(egui::pos2(x0 + 3.5, hero.top() + 8.0), 3.5, dim(col));
            p.text(
                egui::pos2(x0 + 14.0, hero.top() + 8.0),
                egui::Align2::LEFT_CENTER,
                title,
                theme::semibold(13.0),
                dim(col),
            );
        }
    }
    y = hero.bottom() + HERO_GAP;

    // ---- why these numbers are not current -------------------------------
    // The reading below is the last one that arrived. Say so plainly, and say
    // what went wrong, rather than letting a stale number pass for a fresh one.
    if let Some(err) = &s.fetch_error {
        let note = format!("Couldn't refresh \u{00b7} {err}");
        let g = widgets::elide(ui, &note, theme::medium(11.0), dim(pal.warn), w);
        p.galley(egui::pos2(x0, y), g, pal.warn);
        y += NOTE_H + PACE_GAP;
    }

    // ---- what the current pace means -------------------------------------
    // A percentage says how much is left; this says whether it will last, which
    // is the question actually being asked.
    if let Some(pace) = pace {
        let g = widgets::elide(ui, pace, theme::medium(11.0), dim(pal.accent), w);
        p.galley(egui::pos2(x0, y), g, pal.accent);
        y += PACE_H + PACE_GAP;
    }

    // ---- single window: one bar, no list ---------------------------------
    if let Body::Solo(used01) = body(s) {
        let heat = theme::heat(used01, pal);
        let heat = if stale {
            theme::mix(heat, pal.stale, 0.55)
        } else {
            heat
        };
        widgets::meter(
            &p,
            egui::Rect::from_min_size(egui::pos2(x0, y), Vec2::new(w, SOLO_METER_H)),
            used01,
            dim(heat),
            dim(pal.track),
            pal.glow * a,
        );
        card_footer(ui, rect, s, pal, a, now, x0, x1, w);
        return console_clicked;
    }

    // ---- rule ------------------------------------------------------------
    widgets::rule(&p, y, x0, x1, dim(pal.hairline));
    y += 1.0 + RULE_GAP;

    // ---- window list (scrolls only when the screen cannot fit it) --------
    let list_h = list_height.max(0.0);
    let list_rect = egui::Rect::from_min_size(egui::pos2(x0, y), Vec2::new(w, list_h));
    let mut list_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(list_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    list_ui.set_clip_rect(list_rect.intersect(rect));
    egui::ScrollArea::vertical()
        .id_salt(("usage-windows", &s.provider_id))
        .max_height(list_h)
        .auto_shrink([false, false])
        .show(&mut list_ui, |ui| {
            // Rows carry their own metrics; egui's default item spacing would
            // silently add ~8 px per row and push the list into scrolling when
            // the card had been sized to fit it exactly.
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let full = ui.available_width();
            match &s.reading {
                Reading::Ok { windows, .. } if !windows.is_empty() => {
                    let peak = peak_window(windows);
                    for (i, win) in windows.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(ROW_GAP);
                        }
                        let (r, _) =
                            ui.allocate_exact_size(Vec2::new(full, ROW_H), egui::Sense::hover());
                        window_row(ui, r, win, peak == Some(i), pal, a, stale, now);
                    }
                }
                Reading::NeedsAuth(m) | Reading::Error(m) => {
                    let (r, _) = ui
                        .allocate_exact_size(Vec2::new(full, STATUS_BODY_H), egui::Sense::hover());
                    let p = ui.painter();
                    let g = widgets::elide(ui, m, theme::sans(11.0), dim(pal.muted), full);
                    p.galley(egui::pos2(r.left(), r.top()), g, pal.muted);
                    let g2 = widgets::elide(
                        ui,
                        "No current usage reading",
                        theme::sans(10.0),
                        dim(pal.faint),
                        full,
                    );
                    p.galley(egui::pos2(r.left(), r.top() + 17.0), g2, pal.faint);
                }
                _ => {
                    let (r, _) = ui
                        .allocate_exact_size(Vec2::new(full, STATUS_BODY_H), egui::Sense::hover());
                    let g = widgets::elide(
                        ui,
                        "Add this provider in Settings to start tracking it.",
                        theme::sans(11.0),
                        dim(pal.faint),
                        full,
                    );
                    ui.painter()
                        .galley(egui::pos2(r.left(), r.top()), g, pal.faint);
                }
            }
        });

    card_footer(ui, rect, s, pal, a, now, x0, x1, w);
    console_clicked
}

/// Provenance line at the foot of the card: how old the reading is on the
/// left, the provider's own note (or the window count) on the right.
#[allow(clippy::too_many_arguments)]
fn card_footer(
    ui: &egui::Ui,
    rect: egui::Rect,
    s: &Snapshot,
    pal: &Palette,
    a: f32,
    now: u64,
    x0: f32,
    x1: f32,
    w: f32,
) {
    let dim = |c: Color32| c.linear_multiply(a);
    let p = ui.painter();
    let foot_rule_y = rect.bottom() - PAD_BOT - FOOT_H - FOOT_RULE_GAP - 1.0;
    widgets::rule(p, foot_rule_y, x0, x1, dim(pal.hairline));
    let foot_y = foot_rule_y + 1.0 + FOOT_RULE_GAP;
    let age = now.saturating_sub(s.fetched_at);
    let updated = p.layout_no_wrap(
        format!("Updated {} ago", fmt_countdown(age)),
        theme::sans(10.0),
        dim(pal.faint),
    );
    let updated_w = updated.rect.width();
    p.galley(egui::pos2(x0, foot_y), updated, pal.faint);
    let note = match &s.reading {
        Reading::Ok {
            detail: Some(d), ..
        } => Some(d.clone()),
        Reading::Ok { windows, .. } if windows.len() > 1 => {
            Some(format!("{} windows", windows.len()))
        }
        _ => None,
    };
    if let Some(note) = note {
        let g = widgets::elide(
            ui,
            &note,
            theme::sans(10.0),
            dim(pal.muted),
            (w - updated_w - 12.0).max(30.0),
        );
        p.galley(egui::pos2(x1 - g.rect.width(), foot_y), g, pal.muted);
    }
}

/// One window inside the card: label + share used, a slim meter, and the
/// reset/count line beneath it. The peak window is tinted rather than
/// annotated — a "most used" tag used to collide with the reset text.
#[allow(clippy::too_many_arguments)]
fn window_row(
    ui: &egui::Ui,
    r: egui::Rect,
    win: &Window,
    peak: bool,
    pal: &Palette,
    a: f32,
    stale: bool,
    now: u64,
) {
    let dim = |c: Color32| c.linear_multiply(a);
    let used01 = win
        .remaining_percent
        .map(|p| 1.0 - p / 100.0)
        .unwrap_or(0.0) as f32;
    let heat = theme::heat(used01, pal);
    let heat = if stale {
        theme::mix(heat, pal.stale, 0.55)
    } else {
        heat
    };
    let p = ui.painter();

    // label line ----------------------------------------------------------
    // Tabular digits so the column aligns, but the unit in Inter at the hero's
    // proportions — a mono `%` reads as a different, cruder typeface next to
    // the Inter labels beside it.
    let mut value = LayoutJob::default();
    match win.remaining_percent {
        Some(rem) => {
            value.append(
                &format!("{:.0}", 100.0 - rem),
                0.0,
                TextFormat::simple(theme::mono(12.5), dim(heat)),
            );
            value.append(
                "%",
                0.5,
                TextFormat::simple(theme::medium(10.5), dim(heat.gamma_multiply(0.8))),
            );
        }
        None => value.append(
            "—",
            0.0,
            TextFormat::simple(theme::mono(12.5), dim(pal.faint)),
        ),
    }
    let g_val = p.layout_job(value);
    p.galley(
        egui::pos2(r.right() - g_val.rect.width(), r.top()),
        g_val.clone(),
        heat,
    );
    let (label_font, label_col) = if peak {
        (theme::semibold(12.0), heat)
    } else {
        (theme::medium(12.0), pal.text)
    };
    let g_lab = widgets::elide(
        ui,
        &win.label,
        label_font,
        dim(label_col),
        r.width() - g_val.rect.width() - 10.0,
    );
    p.galley(egui::pos2(r.left(), r.top() + 1.0), g_lab, label_col);

    // meter ---------------------------------------------------------------
    let meter_rect = egui::Rect::from_min_size(
        egui::pos2(r.left(), r.top() + 22.0),
        Vec2::new(r.width(), 4.0),
    );
    widgets::meter(
        p,
        meter_rect,
        used01,
        dim(heat),
        dim(pal.track),
        pal.glow * a,
    );

    // meta line -----------------------------------------------------------
    let meta_y = r.top() + 31.0;
    let counts = match (win.remaining_count, win.total_count) {
        (Some(x), Some(t)) if t > 0 => Some(format!("{x}/{t}")),
        _ => None,
    };
    let counts_w = if let Some(c) = &counts {
        let g = p.layout_no_wrap(c.clone(), theme::mono(10.0), dim(pal.faint));
        let gw = g.rect.width();
        p.galley(egui::pos2(r.right() - gw, meta_y), g, pal.faint);
        gw + 10.0
    } else {
        0.0
    };
    // A window that reports no reset time says so: an empty meta line reads
    // as a rendering fault, and "when does this come back" is the second
    // question every row has to answer.
    let (reset_text, reset_col) = match win.resets_at {
        Some(t) => (crate::types::fmt_reset(t, now), pal.muted),
        None => ("No reset time".to_string(), pal.faint),
    };
    let g = widgets::elide(
        ui,
        &reset_text,
        theme::sans(10.0),
        dim(reset_col),
        r.width() - counts_w,
    );
    p.galley(egui::pos2(r.left(), meta_y), g, reset_col);
}

#[derive(Clone, Copy, Debug)]
pub struct RailCardLayout {
    pub rect: egui::Rect,
    pub list_height: f32,
}

/// Place the card beside the hovered row, inside the band the screen actually
/// allows (`host_height` is the host window, which the caller has already
/// clamped to the space below the notch's top edge).
///
/// The card hangs off its row while there is room under it. When there is not
/// — a row low on the screen — it slides up until its foot reaches the bottom
/// of the band, so it opens *upward* from the row instead of running off the
/// bottom. Only when the whole band is too short does it give up height and
/// let its window list scroll.
///
/// This replaced a binary above/below flip. The flip threw away the space on
/// the other side of the anchor: a card that needed 20 px more than the room
/// below jumped entirely above the row, even when sliding up by 20 px would
/// have done. Sliding is continuous and always keeps the card as tall as the
/// band allows.
pub fn rail_card_layout(
    s: &Snapshot,
    pace: bool,
    anchor_y: f32,
    host_height: f32,
    card_width: f32,
    margin: f32,
) -> RailCardLayout {
    // Height is never traded for position: the card is always its natural
    // size and only *where* it sits changes. The caller sizes the host to
    // hold it, so the clamp below can always place it whole.
    let card_h = rail_card_content_height(s, pace);
    let lowest = (host_height - margin - card_h).max(margin);
    let y = (anchor_y + RAIL_CARD_GAP_Y).clamp(margin, lowest);
    RailCardLayout {
        rect: egui::Rect::from_min_size(egui::pos2(0.0, y), Vec2::new(card_width, card_h)),
        list_height: card_h - CARD_FIXED_H,
    }
}
