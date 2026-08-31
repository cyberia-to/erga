//! The parts the window is assembled from: meters that separate what erga
//! costs from what the machine was doing anyway, key/value rows, the panel
//! frame, the crystal, and the two numbers that carry the story.

use eframe::egui;
use egui::{Color32, Pos2, Sense, Stroke, Vec2};
use erga_miner::engine::Progress;
use std::sync::Arc;

use crate::theme::{caps, play_bold};
use crate::{BG, CREAM, MINT, MUTE};

/// A meter that separates *what erga costs* from what the machine was doing
/// anyway: the solid segment is the miner, the dim one behind it is
/// everything else. A bar showing only the total answers the wrong question —
/// from it you cannot tell whether erga is the reason the machine is busy.
pub fn meter(ui: &mut egui::Ui, label: &str, mine: f32, total: f32, val: &str, tint: Color32) {
    ui.vertical(|ui| {
        caps(ui, label, 9.0, tint.gamma_multiply(0.9));
        ui.add_space(4.0);
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 9.0), Sense::hover());
        ui.painter().rect_filled(rect, 4.5, Color32::from_rgb(24, 34, 29));
        let total = total.clamp(0.0, 1.0);
        let mine = mine.clamp(0.0, total);
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, Vec2::new(w * total, 9.0)),
            4.5,
            tint.gamma_multiply(0.26),
        );
        if mine > 0.0005 {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(rect.min, Vec2::new((w * mine).max(3.0), 9.0)),
                4.5,
                tint,
            );
        }
        for i in 1..8 {
            let x = rect.min.x + w * i as f32 / 8.0;
            ui.painter().line_segment(
                [Pos2::new(x, rect.min.y + 1.0), Pos2::new(x, rect.max.y - 1.0)],
                Stroke::new(1.0, BG),
            );
        }
        if resp.hovered() {
            resp.on_hover_text(format!(
                "erga {:.0}%  ·  everything else {:.0}%",
                mine * 100.0,
                (total - mine).max(0.0) * 100.0
            ));
        }
        ui.add_space(4.0);
        let mut job = egui::text::LayoutJob::default();
        job.append(
            val,
            0.0,
            egui::TextFormat {
                font_id: play_bold(14.0),
                color: CREAM.gamma_multiply(0.92),
                ..Default::default()
            },
        );
        ui.label(job);
    });
}

/// A key/value row inside a card (no outer margins — the card supplies them).
/// Like `card_row`, but the value wears a colour that means something.
pub fn card_row_tinted(ui: &mut egui::Ui, key: &str, val: &str, tint: Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).color(MINT.gamma_multiply(0.85)).size(11.5));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(val).color(tint).size(12.0).monospace());
        });
    });
}

pub fn card_row(ui: &mut egui::Ui, key: &str, val: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).color(MINT.gamma_multiply(0.85)).size(11.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(val).color(CREAM.gamma_multiply(0.88)).size(11.5).monospace());
        });
    });
}

/// A bordered panel. When `h` is given the content area is allocated at
/// exactly that size — not merely *at least* it — because a minimum only
/// stops a panel shrinking, and the one on the right grows from the bottom.
/// Two panels that must look like one object have to be told the same size,
/// A bordered panel. With a height given, the rectangle is allocated and
/// stroked directly and the content is drawn inside it, clipped. Asking a
/// layout for a size is a request; taking the rectangle is a guarantee —
/// and two panels that read as one object must be the same size exactly,
/// A bordered panel. With a height given the content ui is *fixed* to it —
/// `set_height`, not `set_min_height` — which does two things at once: the
/// two panels end up identical, and `available_height` inside becomes a
/// finite number, so a bottom-anchored child can pin itself without any
/// spacer arithmetic to overshoot.
pub fn panel_frame(ui: &mut egui::Ui, w: f32, h: f32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::none()
        .stroke(Stroke::new(1.0, MINT.gamma_multiply(0.22)))
        .rounding(14.0)
        .inner_margin(egui::Margin::symmetric(18.0, 16.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(w - 36.0);
                if h > 0.0 {
                    ui.set_height(h - 32.0);
                }
                add(ui);
            });
        });
}

/// The heptagon sigil — soft3's crystal, here as the one button. Fills
/// mint when mining, outline when idle; the inner facet counter-rotates.
pub fn draw_crystal(ui: &egui::Ui, c: Pos2, r: f32, on: bool, spin: f32, hover: bool) {
    let painter = ui.painter();
    let hept = |radius: f32, phase: f32| -> Vec<Pos2> {
        (0..7)
            .map(|i| {
                let a = phase - std::f32::consts::FRAC_PI_2
                    + i as f32 * std::f32::consts::TAU / 7.0;
                Pos2::new(c.x + radius * a.cos(), c.y + radius * a.sin())
            })
            .collect()
    };
    let outer = hept(r * if hover && !on { 1.05 } else { 1.0 }, 0.0);
    if on {
        // soft outer glow — a few expanding rings with falling alpha
        for i in 1..=4 {
            let k = i as f32;
            painter.add(egui::Shape::closed_line(
                hept(r * (1.0 + k * 0.06), 0.0),
                Stroke::new(2.0, MINT.gamma_multiply(0.10 / k)),
            ));
        }
        painter.add(egui::Shape::convex_polygon(outer.clone(), MINT, Stroke::new(1.5, MINT)));
        let inner = hept(r * 0.62, -spin);
        painter.add(egui::Shape::closed_line(inner, Stroke::new(1.0, BG.gamma_multiply(0.6))));
    } else {
        for i in 1..=3 {
            let k = i as f32;
            painter.add(egui::Shape::closed_line(
                hept(r * (1.0 + k * 0.05), 0.0),
                Stroke::new(1.5, MINT.gamma_multiply(0.06 / k)),
            ));
        }
        painter.add(egui::Shape::closed_line(outer, Stroke::new(1.5, MINT.gamma_multiply(0.85))));
        let inner = hept(r * 0.62, spin);
        painter.add(egui::Shape::closed_line(inner, Stroke::new(1.0, MINT.gamma_multiply(0.35))));
    }
}

/// The balance, at the top of the window: the number everything else exists
/// to move. It is the one figure here that can grow without bound, so it is
/// also the one allowed to shrink its own type rather than spill.
pub fn big_balance(ui: &mut egui::Ui, on_chain: Option<f64>, w: f32) {
    let text = match on_chain {
        Some(e) => format!("{e:.4}"),
        None => "0.0000".to_string(),
    };
    // Always full strength. Dimming it for being zero made the hashrate the
    // brightest thing on screen, which inverts what this app is for.
    let colour = MINT;
    let avail = (w - 90.0).max(80.0);
    let mut size = 60.0f32;
    while size > 18.0 {
        let g = ui.painter().layout_no_wrap(text.clone(), play_bold(size), colour);
        if g.size().x <= avail {
            break;
        }
        size -= 2.0;
    }
    ui.vertical_centered(|ui| {
        caps(ui, "your balance on chain", 9.5, MUTE);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let g = ui.painter().layout_no_wrap(text.clone(), play_bold(size), colour);
            let row = g.size().x + 46.0;
            ui.add_space(((ui.available_width() - row) / 2.0).max(0.0));
            let mut job = egui::text::LayoutJob::default();
            job.append(
                &text,
                0.0,
                egui::TextFormat { font_id: play_bold(size), color: colour, ..Default::default() },
            );
            ui.label(job);
            ui.add_space(7.0);
            caps(ui, "erg", 12.0, MUTE);
        });
    });
}

/// The hero number: the live hashrate, the biggest thing on the screen
/// after the crystal itself.
pub fn hero_rate(ui: &mut egui::Ui, mhs: f64, running: bool, p: &Arc<Progress>, size: f32) {
    ui.vertical_centered(|ui| {
        let text = if running { format!("{mhs:.1}") } else { "—".to_string() };
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &text,
            0.0,
            egui::TextFormat { font_id: play_bold(size), color: MINT, ..Default::default() },
        );
        ui.label(job);
        caps(ui, "mh/s", 11.0, MUTE);
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(p.status.lock().unwrap().clone())
                .color(MUTE)
                .size(11.5),
        );
    });
}

/// The invitation to start, at the top of the window. It breathes rather
/// than blinks: a slow sine on the alpha, so it draws the eye without
/// nagging it.
pub fn start_hint(ui: &mut egui::Ui) {
    let t = ui.input(|i| i.time);
    // ~4.5 s per breath, never fully out
    let a = 0.42 + 0.58 * (0.5 - 0.5 * ((t * 1.4).cos() as f32));
    ui.vertical_centered(|ui| {
        caps(ui, "press the crystal to begin", 10.5, MINT.gamma_multiply(a));
    });
}

pub fn human(n: u64) -> String {
    if n >= 1_000_000_000_000 {
        format!("{:.2} T", n as f64 / 1e12)
    } else if n >= 1_000_000_000 {
        format!("{:.2} B", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.1} M", n as f64 / 1e6)
    } else {
        format!("{n}")
    }
}
