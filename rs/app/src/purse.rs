//! The wallet's face: the address, the action bar at the foot of the window,
//! and the seed screen — which takes the whole window rather than hovering
//! over the app as a dialog would.
//!
//! Rows of buttons are centred from a *measured* width. egui lays a button out
//! as its text plus twice the button padding, with item spacing between
//! siblings; guessing that total is what leaves a row looking a few pixels off
//! centre, which the eye notices even when it cannot say why.

use eframe::egui;
use egui::{Color32, FontId, Sense, Stroke, Vec2};

use crate::theme::caps;

/// The exact width a row of buttons will occupy. Measure the spacing you are
/// about to use *after* setting it, or the answer describes a different row.
fn row_width(ui: &egui::Ui, labels: &[&str], size: f32) -> f32 {
    let pad = ui.spacing().button_padding.x;
    let gap = ui.spacing().item_spacing.x;
    labels
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let g = ui
                .painter()
                .layout_no_wrap((*l).to_string(), egui::FontId::proportional(size), MINT);
            g.size().x + pad * 2.0 + if i > 0 { gap } else { 0.0 }
        })
        .sum()
}
use crate::{AMBER, CORAL, CREAM, CTRL_H, MINT};

/// The wallet strip — identity, present but out of the game's way.
/// The wallet, directly under the button that fills it: the address large
/// enough to read across a room, and the two things you actually do with it.
pub fn wallet_block(ui: &mut egui::Ui, addr: Option<&str>) {
    let Some(addr) = addr else {
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("wallet unavailable — could not read the seed file")
                    .color(CORAL)
                    .size(12.0),
            );
        });
        return;
    };
    ui.vertical_centered(|ui| {
        // An Ergo address under a mining crystal is self-evidently your wallet.
        ui.label(
            egui::RichText::new(addr)
                .monospace()
                .size(13.0)
                .color(CREAM.gamma_multiply(0.9)),
        );
    });
}

/// The action bar: the only things you do here besides press the crystal,
/// so they sit at the foot of the window where a hand expects them, sized to
/// be hit rather than squinted at.
pub fn action_bar(
    ui: &mut egui::Ui,
    addr: Option<&str>,
    want_backup: &mut bool,
    want_report: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().button_padding = Vec2::new(24.0, 12.0);
        ui.spacing_mut().item_spacing.x = 14.0;
        let row = row_width(ui, &["copy address", "back up", "report a bug"], 14.0);
        ui.add_space(((ui.available_width() - row) / 2.0).max(0.0));
        let has = addr.is_some();
        if ui
            .add_enabled(has, egui::Button::new(egui::RichText::new("copy address").size(14.0)))
            .clicked()
        {
            if let Some(a) = addr {
                ui.output_mut(|o| o.copied_text = a.to_string());
            }
        }
        if ui
            .add_enabled(has, egui::Button::new(egui::RichText::new("back up").size(14.0)))
            .clicked()
        {
            *want_backup = true;
        }
        if ui
            .button(egui::RichText::new("report a bug").size(14.0))
            .on_hover_text(
                "opens a GitHub issue with your machine, the app state and the recent log already filled in",
            )
            .clicked()
        {
            *want_report = true;
        }
    });
}

/// The seed, alone on screen. Returns true when the user dismisses it.
///
/// Not a modal: a modal invites you to keep working with your wallet's keys
/// sitting behind a dialog. This takes the window, says the words are on the
/// clipboard, and stays until you dismiss it.
pub fn backup_screen(ui: &mut egui::Ui, seed: Option<&str>) -> bool {
    let mut done = false;
    ui.add_space(26.0);
    // the notice, at the top, where a notification belongs
    ui.vertical_centered(|ui| {
        let pad = ui.spacing().button_padding;
        let g = ui.painter().layout_no_wrap(
            "copied to your clipboard".into(),
            FontId::proportional(11.5),
            MINT,
        );
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(g.size().x + pad.x * 2.5, CTRL_H + 4.0),
            Sense::hover(),
        );
        ui.painter().rect_filled(rect, 999.0, MINT.gamma_multiply(0.14));
        ui.painter()
            .rect_stroke(rect, 999.0, Stroke::new(1.0, MINT.gamma_multiply(0.55)));
        ui.painter().galley(rect.center() - g.size() / 2.0, g, MINT);
    });

    let h = ui.available_height();
    ui.add_space((h * 0.16).max(18.0));
    ui.vertical_centered(|ui| {
        caps(ui, "back up your seed", 13.0, CREAM);
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(
                "these fifteen words ARE your wallet. anyone who sees them owns your coins.\n                 write them on paper. the clipboard is not a backup — it will be overwritten.",
            )
            .color(AMBER)
            .size(13.0),
        );
        ui.add_space(26.0);
        match seed {
            Some(m) => {
                let w = (ui.available_width() * 0.66).clamp(360.0, 720.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(5, 14, 10))
                    .stroke(Stroke::new(1.0, MINT.gamma_multiply(0.28)))
                    .inner_margin(egui::Margin::symmetric(26.0, 22.0))
                    .rounding(12.0)
                    .show(ui, |ui| {
                        ui.set_width(w);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(m)
                                    .color(MINT)
                                    .size(20.0)
                                    .monospace(),
                            );
                        });
                    });
            }
            None => {
                ui.label(egui::RichText::new("no seed to show").color(CORAL).size(13.0));
            }
        }
    });
    // The way out sits where every other action in this app sits: the foot of
    // the window, at the same height as the action bar it replaces. A button
    // that moves when the screen changes makes you hunt for it.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = Vec2::new(24.0, 12.0);
            let row = row_width(ui, &["I've written them down"], 14.0);
            ui.add_space(((ui.available_width() - row) / 2.0).max(0.0));
            if ui
                .button(egui::RichText::new("I've written them down").size(14.0))
                .clicked()
            {
                done = true;
            }
        });
    });
    done
}
