//! erga — one-button ERGO miner for Apple Silicon.
//!
//! Open it, press the crystal, watch the hashrate. The miner runs the
//! honeycrisp zero-copy Autolykos v2 kernel measured in the erga study
//! (32 MH/s at 8.3 W sustained on an M4 Max — 2.4× an RTX 3090 per watt).
//!
//! v0.1 is a local mining benchmark: it proves the efficiency on *your*
//! Mac. Pool connection and share submission are the next build.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod balance;
mod miner;

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Sense, Stroke, Vec2};

use balance::BalanceState;
use miner::Miner;

const BG: Color32 = Color32::from_rgb(6, 16, 10);
const MINT: Color32 = Color32::from_rgb(125, 255, 196);
const CREAM: Color32 = Color32::from_rgb(255, 248, 240);
const MUTE: Color32 = Color32::from_rgb(120, 140, 128);

fn main() -> eframe::Result<()> {
    // headless mining lives in the CLI: `erga-miner mine <host> <port> <addr>`.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 660.0])
            .with_min_inner_size([420.0, 600.0])
            .with_title("erga"),
        ..Default::default()
    };
    eframe::run_native(
        "erga",
        options,
        Box::new(|cc| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = BG;
            visuals.window_fill = BG;
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(App::new()))
        }),
    )
}

struct App {
    miner: Miner,
    balance: BalanceState,
    address: String,
    spin: f32,
}

impl App {
    fn new() -> Self {
        App {
            miner: Miner::new(),
            balance: BalanceState::default(),
            address: String::new(),
            spin: 0.0,
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        [6.0 / 255.0, 16.0 / 255.0, 10.0 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let running = self.miner.is_running();
        // keep the frame ticking while mining so the hashrate updates live
        if running {
            self.spin += 0.01;
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(18.0);

            // ── header ────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("ERGA")
                        .color(CREAM)
                        .size(20.0)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new("ERGO miner")
                        .color(MUTE)
                        .size(12.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(20.0);
                    badge(ui, "experimental");
                });
            });

            ui.add_space(10.0);

            // ── the crystal button ────────────────────────────────────
            let (rect, resp) = ui.allocate_at_least(
                Vec2::new(ui.available_width(), 230.0),
                Sense::click(),
            );
            let center = rect.center();
            draw_crystal(ui, center, 92.0, running, self.spin, resp.hovered());
            let label = if running { "MINING" } else { "START" };
            ui.painter().text(
                center,
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(19.0),
                if running { BG } else { MINT },
            );
            if resp.clicked() {
                if running {
                    self.miner.stop();
                } else if self.address.trim().len() < 20 {
                    self.miner.p.set_status("enter your ERGO address to mine");
                } else {
                    self.miner.start(self.address.trim().to_string());
                }
            }
            if resp.hovered() {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            }

            // ── live hashrate ─────────────────────────────────────────
            let p = &self.miner.p;
            ui.add_space(6.0);
            ui.vertical_centered(|ui| {
                let rate = if running { format!("{:.1}", p.mhs()) } else { "—".to_string() };
                ui.label(egui::RichText::new(rate).color(MINT).size(46.0).strong());
                ui.label(egui::RichText::new("MH/s").color(MUTE).size(13.0));
                let status = p.status.lock().unwrap().clone();
                ui.add_space(2.0);
                ui.label(egui::RichText::new(status).color(MUTE).size(12.0));
            });

            ui.add_space(12.0);
            {
                let acc = p.accepted.load(std::sync::atomic::Ordering::Relaxed);
                let rej = p.rejected.load(std::sync::atomic::Ordering::Relaxed);
                let shares = if rej > 0 { format!("{acc}  ({rej} rejected)") } else { format!("{acc}") };
                stat_row(ui, "shares", &shares);
            }
            stat_row(ui, "device", &p.device.lock().unwrap());
            {
                let h = p.height.load(std::sync::atomic::Ordering::Relaxed);
                stat_row(ui, "block", &(if h > 0 { h.to_string() } else { "—".into() }));
            }
            {
                let total = p.hashed.load(std::sync::atomic::Ordering::Relaxed);
                stat_row(ui, "hashed", &human(total));
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);

            // ── wallet / balance ──────────────────────────────────────
            ui.label(egui::RichText::new("YOUR ERGO ADDRESS — SHARES PAY OUT HERE").color(MUTE).size(11.0).strong());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_enabled(
                    !running,
                    egui::TextEdit::singleline(&mut self.address)
                        .hint_text("9f… (paste your wallet)")
                        .desired_width(ui.available_width() - 84.0),
                );
                if ui.button("balance").clicked() {
                    self.balance.fetch(self.address.clone());
                }
            });
            ui.add_space(6.0);
            {
                let b = self.balance.inner.lock().unwrap();
                if b.querying {
                    ui.label(egui::RichText::new("checking…").color(MUTE).size(13.0));
                } else if let Some(erg) = b.erg {
                    ui.label(
                        egui::RichText::new(format!("{erg:.4} ERG"))
                            .color(MINT)
                            .size(18.0)
                            .strong(),
                    );
                } else if let Some(err) = &b.error {
                    ui.label(egui::RichText::new(err).color(Color32::from_rgb(255, 140, 140)).size(12.0));
                } else {
                    ui.label(egui::RichText::new("confirmed balance, read-only via explorer").color(MUTE).size(12.0));
                }
            }

            // ── honest footer ─────────────────────────────────────────
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(
                        "mines Autolykos v2 to herominers under your address.\n\
                         every share is re-verified on-CPU before it is sent.",
                    )
                    .color(MUTE)
                    .size(11.0),
                );
            });
        });
    }
}

fn badge(ui: &mut egui::Ui, text: &str) {
    let galley = ui.painter().layout_no_wrap(
        text.into(),
        FontId::proportional(10.0),
        MINT,
    );
    let pad = Vec2::new(10.0, 5.0);
    let (rect, _) = ui.allocate_exact_size(galley.size() + pad * 2.0, Sense::hover());
    ui.painter().rect_stroke(rect, 999.0, Stroke::new(1.0, MINT.gamma_multiply(0.5)));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, MINT);
}

fn stat_row(ui: &mut egui::Ui, key: &str, val: &str) {
    ui.horizontal(|ui| {
        ui.add_space(24.0);
        ui.label(egui::RichText::new(key).color(MINT).size(11.0).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(24.0);
            ui.label(egui::RichText::new(val).color(CREAM.gamma_multiply(0.85)).size(12.0));
        });
    });
}

/// The heptagon sigil — soft3's crystal, here as the one button. Fills
/// mint when mining, outline when idle; the inner facet counter-rotates.
fn draw_crystal(ui: &egui::Ui, c: Pos2, r: f32, on: bool, spin: f32, hover: bool) {
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
        painter.add(egui::Shape::convex_polygon(
            outer.clone(),
            MINT,
            Stroke::new(1.5, MINT),
        ));
        // counter-rotating inner facet
        let inner = hept(r * 0.62, -spin);
        painter.add(egui::Shape::closed_line(inner, Stroke::new(1.0, BG.gamma_multiply(0.6))));
    } else {
        painter.add(egui::Shape::closed_line(outer, Stroke::new(1.5, MINT.gamma_multiply(0.8))));
        let inner = hept(r * 0.62, spin);
        painter.add(egui::Shape::closed_line(
            inner,
            Stroke::new(1.0, MINT.gamma_multiply(0.35)),
        ));
    }
}

fn human(n: u64) -> String {
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
