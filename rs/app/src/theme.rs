//! The look: the palette's carriers, the typeface, and the primitives every
//! other module draws with. One height for every control in the header,
//! because two that differ by two pixels read as a mistake, not a hierarchy.

use eframe::egui;
use egui::{Color32, FontId, Sense, Stroke, Vec2};

use crate::{BG, CREAM, CTRL_H, MINT};

/// Play — the typeface of old.cyb.ai — carries the whole app. Regular for
/// text, Bold for the numbers that matter.
pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "play".into(),
        egui::FontData::from_static(include_bytes!("../assets/Play-Regular.ttf")),
    );
    fonts.font_data.insert(
        "play-bold".into(),
        egui::FontData::from_static(include_bytes!("../assets/Play-Bold.ttf")),
    );
    fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap()
        .insert(0, "play".into());
    fonts.families.insert(
        egui::FontFamily::Name("play-bold".into()),
        vec!["play-bold".into(), "play".into()],
    );
    ctx.set_fonts(fonts);
}

pub fn play_bold(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("play-bold".into()))
}

/// The cyber look, applied once: black panels, mint pill buttons with a thin
/// stroke instead of egui's grey slabs, mint-tinted separators and windows.
pub fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = BG;
    v.window_stroke = Stroke::new(1.0, MINT.gamma_multiply(0.30));
    v.window_rounding = 12.0.into();
    v.override_text_color = None;

    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, MINT.gamma_multiply(0.14)); // separators
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, CREAM.gamma_multiply(0.88));

    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, MINT.gamma_multiply(0.38));
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, MINT.gamma_multiply(0.92));
    v.widgets.inactive.rounding = 999.0.into();

    v.widgets.hovered.bg_fill = MINT.gamma_multiply(0.10);
    v.widgets.hovered.weak_bg_fill = MINT.gamma_multiply(0.10);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, MINT.gamma_multiply(0.9));
    v.widgets.hovered.fg_stroke = Stroke::new(1.5, MINT);
    v.widgets.hovered.rounding = 999.0.into();

    v.widgets.active.bg_fill = MINT.gamma_multiply(0.22);
    v.widgets.active.weak_bg_fill = MINT.gamma_multiply(0.22);
    v.widgets.active.bg_stroke = Stroke::new(1.0, MINT);
    v.widgets.active.fg_stroke = Stroke::new(1.5, MINT);
    v.widgets.active.rounding = 999.0.into();

    v.selection.bg_fill = MINT.gamma_multiply(0.25);
    ctx.set_visuals(v);

    let mut style = (*ctx.style()).clone();
    style.spacing.button_padding = Vec2::new(11.0, 4.0);
    ctx.set_style(style);
}

/// Uppercase micro-heading with letter-spacing — the old.cyb.ai label voice.
pub fn caps(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &text.to_uppercase(),
        0.0,
        egui::TextFormat {
            font_id: FontId::proportional(size),
            color,
            extra_letter_spacing: 1.6,
            ..Default::default()
        },
    );
    ui.label(job);
}

pub fn badge(ui: &mut egui::Ui, text: &str, tint: Color32) {
    let galley = ui.painter().layout_no_wrap(text.into(), FontId::proportional(10.5), tint);
    let pad = ui.spacing().button_padding;
    let size = Vec2::new(galley.size().x + pad.x * 2.0, CTRL_H);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_stroke(rect, 999.0, Stroke::new(1.0, tint.gamma_multiply(0.6)));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, tint);
}

/// A pill that sits level with the buttons beside it. egui sizes a button as
/// max(interact_size.y, text + 2*button_padding.y); matching that formula —
/// and the button font — is what keeps the header from stepping.
/// A pill that toggles, drawn to the same metric as `badge` so the header
/// reads as one row. egui's checkbox draws a circle and a label, which is a
/// different shape language from the controls beside it.
pub fn pill_toggle(ui: &mut egui::Ui, text: &str, on: &mut bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        text.into(),
        FontId::proportional(10.5),
        if *on { BG } else { MINT },
    );
    let pad = ui.spacing().button_padding;
    let size = Vec2::new(galley.size().x + pad.x * 2.0, CTRL_H);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hot = resp.hovered();
    if *on {
        ui.painter().rect_filled(rect, 999.0, MINT.gamma_multiply(if hot { 1.0 } else { 0.88 }));
    } else if hot {
        ui.painter().rect_filled(rect, 999.0, MINT.gamma_multiply(0.10));
    }
    ui.painter().rect_stroke(
        rect,
        999.0,
        Stroke::new(1.0, MINT.gamma_multiply(if *on || hot { 0.95 } else { 0.45 })),
    );
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, MINT);
    if hot {
        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
    }
    if resp.clicked() {
        *on = !*on;
        return true;
    }
    false
}

/// The app icon, as a texture, decoded once and kept. Same bytes the bundle
/// ships — the window and the Dock cannot drift apart.
pub fn load_icon(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).ok()?;
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    Some(ctx.load_texture("erga-icon", image, egui::TextureOptions::LINEAR))
}
