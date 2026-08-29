//! erga — one-button ERGO miner + wallet for Apple Silicon.
//!
//! Open it: a self-custodial Ergo wallet is generated for you. Press the
//! crystal and it mines Autolykos v2 to a pool under that address, on the
//! honeycrisp zero-copy GPU kernel (32 MH/s at 8.3 W sustained on an M4
//! Max — 2.4× an RTX 3090 per watt). Watch the hashrate, the accepted
//! shares, and your balance grow.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod balance;
mod miner;
mod stats;

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Sense, Stroke, Vec2};

use balance::BalanceState;
use miner::Miner;

const BG: Color32 = Color32::from_rgb(3, 5, 4); // near-black with a faint green cast
const MINT: Color32 = Color32::from_rgb(125, 255, 196);
const CREAM: Color32 = Color32::from_rgb(235, 245, 240);
const MUTE: Color32 = Color32::from_rgb(90, 110, 100);

fn main() -> eframe::Result<()> {
    // headless mining lives in the CLI: `erga-miner mine <host> <port> <addr>`.
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 752.0])
            .with_min_inner_size([430.0, 720.0])
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
    wallet: Result<erga_wallet::Wallet, String>,
    show_backup: bool,
    spin: f32,
    last_balance: std::time::Instant,
    sys: stats::Sys,
}

impl App {
    fn new() -> Self {
        let wallet = erga_wallet::Wallet::load_or_create();
        let balance = BalanceState::default();
        if let Ok(w) = &wallet {
            balance.fetch(w.address.clone()); // show balance immediately
        }
        App {
            miner: Miner::new(),
            balance,
            wallet,
            show_backup: false,
            spin: 0.0,
            last_balance: std::time::Instant::now(),
            sys: stats::Sys::new(),
        }
    }

    fn address(&self) -> Option<&str> {
        self.wallet.as_ref().ok().map(|w| w.address.as_str())
    }
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        [3.0 / 255.0, 5.0 / 255.0, 4.0 / 255.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let running = self.miner.is_running();
        if running {
            self.spin += 0.01;
        }
        // keep ticking so hashrate, dashboard and balance stay live
        ctx.request_repaint_after(std::time::Duration::from_millis(if running { 250 } else { 1000 }));

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
                } else if let Some(addr) = self.address().map(|a| a.to_string()) {
                    self.miner.start(addr);
                } else {
                    self.miner.p.set_status("wallet unavailable");
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

            // ── system dashboard row ──────────────────────────────────
            self.sys.refresh();
            let mhs = p.mhs();
            ui.add_space(12.0);
            ui.columns(4, |c| {
                meter(&mut c[0], "CPU", self.sys.cpu, &format!("{:.0}%", self.sys.cpu * 100.0));
                // GPU has no privilege-free utilisation read — its live signal
                // is the hashrate, scaled against ~80 MH/s (the M4 Max ceiling)
                meter(&mut c[1], "GPU", (mhs / 80.0) as f32, &format!("{mhs:.0} MH/s"));
                meter(&mut c[2], "RAM", self.sys.mem, &format!("{:.0}%", self.sys.mem * 100.0));
                let net = ((self.sys.down_kbs + self.sys.up_kbs) / 2048.0) as f32; // vs ~2 MB/s
                meter(&mut c[3], "NET", net.min(1.0), &format!("{:.0} KB/s", self.sys.down_kbs));
            });

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);

            // ── wallet / balance ──────────────────────────────────────
            let addr_opt: Option<String> = self.address().map(|a| a.to_string());
            // auto-refresh balance every 30s so earnings appear on their own
            if let Some(addr) = &addr_opt {
                if self.last_balance.elapsed().as_secs() >= 30 {
                    self.balance.fetch(addr.clone());
                    self.last_balance = std::time::Instant::now();
                }
            }
            match &addr_opt {
                Some(addr) => {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("YOUR WALLET — SHARES PAY OUT HERE").color(MUTE).size(11.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("back up").clicked() {
                                self.show_backup = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    let addr = addr.to_string();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&addr).color(CREAM.gamma_multiply(0.9)).size(11.5).monospace());
                        if ui.small_button("copy").clicked() {
                            ui.output_mut(|o| o.copied_text = addr.clone());
                        }
                    });
                    ui.add_space(6.0);
                    let b = self.balance.inner.lock().unwrap();
                    if let Some(erg) = b.erg {
                        ui.label(egui::RichText::new(format!("{erg:.4} ERG")).color(MINT).size(20.0).strong());
                    } else if b.querying {
                        ui.label(egui::RichText::new("checking balance…").color(MUTE).size(13.0));
                    } else if let Some(err) = &b.error {
                        ui.label(egui::RichText::new(err).color(Color32::from_rgb(255, 140, 140)).size(12.0));
                    } else {
                        ui.label(egui::RichText::new("0.0000 ERG").color(MINT.gamma_multiply(0.7)).size(20.0).strong());
                    }
                }
                None => {
                    ui.label(egui::RichText::new("wallet unavailable — could not read the seed file").color(Color32::from_rgb(255, 140, 140)).size(12.0));
                }
            }

            // ── backup panel (reveal the seed once, with a warning) ────
            if self.show_backup {
                let mnemonic = self.wallet.as_ref().ok().map(|w| w.mnemonic.clone());
                let mut close = false;
                egui::Window::new("back up your seed")
                    .collapsible(false)
                    .resizable(false)
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new(
                            "these 15 words ARE your wallet. anyone who sees them owns your\n\
                             coins. write them on paper, never screenshot, never share.",
                        ).color(Color32::from_rgb(255, 200, 120)).size(12.0));
                        ui.add_space(8.0);
                        if let Some(m) = &mnemonic {
                            egui::Frame::none()
                                .fill(Color32::from_rgb(4, 12, 8))
                                .inner_margin(10.0)
                                .rounding(8.0)
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(m).color(MINT).size(15.0).monospace());
                                });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui.button("copy words").clicked() {
                                    ui.output_mut(|o| o.copied_text = m.clone());
                                }
                                if ui.button("I've written them down").clicked() {
                                    close = true;
                                }
                            });
                        }
                    });
                if close {
                    self.show_backup = false;
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

/// A compact dashboard meter: label, a thin fill bar (0..1), a value.
fn meter(ui: &mut egui::Ui, label: &str, frac: f32, val: &str) {
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new(label).size(9.0).color(MINT).strong());
        ui.add_space(2.0);
        let w = ui.available_width().min(90.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 5.0), Sense::hover());
        let bg = egui::Rect::from_min_size(rect.min, Vec2::new(w, 5.0));
        ui.painter().rect_filled(bg, 2.5, Color32::from_rgb(30, 40, 34));
        let f = frac.clamp(0.0, 1.0);
        let fill = egui::Rect::from_min_size(rect.min, Vec2::new(w * f, 5.0));
        let col = if f > 0.9 { Color32::from_rgb(255, 180, 120) } else { MINT };
        ui.painter().rect_filled(fill, 2.5, col);
        ui.add_space(2.0);
        ui.label(egui::RichText::new(val).size(10.0).color(CREAM.gamma_multiply(0.85)));
    });
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
