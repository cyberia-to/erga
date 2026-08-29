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
mod pool;
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
            .with_inner_size([460.0, 872.0])
            .with_min_inner_size([430.0, 830.0])
            .with_title("erga"),
        ..Default::default()
    };
    eframe::run_native(
        "erga",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            setup_style(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}

/// Play — the typeface of old.cyb.ai — carries the whole app. Regular for
/// text, Bold for the numbers that matter.
fn setup_fonts(ctx: &egui::Context) {
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

fn play_bold(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("play-bold".into()))
}

/// The cyber look, applied once: black panels, mint pill buttons with a thin
/// stroke instead of egui's grey slabs, mint-tinted separators and windows.
fn setup_style(ctx: &egui::Context) {
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
fn caps(ui: &mut egui::Ui, text: &str, size: f32, color: Color32) {
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

struct App {
    miner: Miner,
    balance: BalanceState,
    pool: pool::PoolState,
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
        let pool = pool::PoolState::default();
        if let Ok(w) = &wallet {
            balance.fetch(w.address.clone()); // show balance immediately
            pool.fetch(w.address.clone()); // and what the pool owes us
        }
        App {
            miner: Miner::new(),
            balance,
            pool,
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
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    "ERGA",
                    0.0,
                    egui::TextFormat {
                        font_id: play_bold(21.0),
                        color: CREAM,
                        extra_letter_spacing: 3.0,
                        ..Default::default()
                    },
                );
                ui.label(job);
                ui.add_space(4.0);
                caps(ui, "ergo miner", 10.5, MUTE);
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

            ui.add_space(16.0);

            // ── wallet card — bordered panel, the old.cyb.ai voice ────
            let addr_opt: Option<String> = self.address().map(|a| a.to_string());
            // auto-refresh balance + pool ledger every 30s
            if let Some(addr) = &addr_opt {
                if self.last_balance.elapsed().as_secs() >= 30 {
                    self.balance.fetch(addr.clone());
                    self.pool.fetch(addr.clone());
                    self.last_balance = std::time::Instant::now();
                }
            }
            egui::Frame::none()
                .stroke(Stroke::new(1.0, MINT.gamma_multiply(0.24)))
                .rounding(12.0)
                .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                .outer_margin(egui::Margin::symmetric(20.0, 0.0))
                .show(ui, |ui| {
                    match &addr_opt {
                        Some(addr) => {
                            ui.horizontal(|ui| {
                                caps(ui, "your wallet", 10.0, MUTE);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button(egui::RichText::new("back up").size(10.5)).clicked() {
                                        self.show_backup = true;
                                    }
                                });
                            });
                            ui.add_space(7.0);
                            let addr_s = addr.to_string();
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(&addr_s)
                                        .color(CREAM.gamma_multiply(0.72))
                                        .size(10.8)
                                        .monospace(),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button(egui::RichText::new("copy").size(10.5)).clicked() {
                                        ui.output_mut(|o| o.copied_text = addr_s.clone());
                                    }
                                });
                            });
                            ui.add_space(8.0);
                            // the balance — the number that matters, in Play Bold
                            let b = self.balance.inner.lock().unwrap();
                            let (amount, dim) = match (b.erg, b.querying, &b.error) {
                                (Some(erg), _, _) => (format!("{erg:.4}"), false),
                                (None, true, _) => ("…".to_string(), true),
                                (None, _, Some(_)) => ("—".to_string(), true),
                                _ => ("0.0000".to_string(), true),
                            };
                            let err = b.error.clone();
                            drop(b);
                            ui.horizontal(|ui| {
                                let mut job = egui::text::LayoutJob::default();
                                job.append(
                                    &amount,
                                    0.0,
                                    egui::TextFormat {
                                        font_id: play_bold(30.0),
                                        color: if dim { MINT.gamma_multiply(0.65) } else { MINT },
                                        ..Default::default()
                                    },
                                );
                                ui.label(job);
                                ui.add_space(2.0);
                                caps(ui, "erg", 11.0, MUTE);
                            });
                            if let Some(e) = err {
                                ui.label(egui::RichText::new(e).color(Color32::from_rgb(255, 140, 140)).size(10.5));
                            }

                            // ── the payout — the one bar the player fills ──
                            let pi = self.pool.inner.lock().unwrap();
                            if pi.ok {
                                ui.add_space(10.0);
                                ui.separator();
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    caps(ui, "at the pool", 10.0, MUTE);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        caps(
                                            ui,
                                            &format!("sees {:.0} mh/s · 24h", pi.hashrate_24h_mhs),
                                            9.0,
                                            MUTE.gamma_multiply(0.85),
                                        );
                                    });
                                });
                                ui.add_space(9.0);

                                let earned = pi.balance_erg + pi.pending_erg;
                                let toward = (earned / pi.threshold_erg) as f32;

                                // the payout bar — segmented in tens, a pulsing
                                // tip marks where the earning happens
                                let w = ui.available_width();
                                let (rect, _) =
                                    ui.allocate_exact_size(Vec2::new(w, 8.0), Sense::hover());
                                ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(22, 32, 27));
                                let f = toward.clamp(0.0, 1.0).max(0.008);
                                let fw = w * f;
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(rect.min, Vec2::new(fw, 8.0)),
                                    4.0,
                                    MINT.gamma_multiply(0.92),
                                );
                                for i in 1..10 {
                                    let x = rect.min.x + w * i as f32 / 10.0;
                                    ui.painter().line_segment(
                                        [
                                            Pos2::new(x, rect.min.y + 1.5),
                                            Pos2::new(x, rect.max.y - 1.5),
                                        ],
                                        Stroke::new(1.0, BG),
                                    );
                                }
                                let t = ui.input(|i| i.time);
                                let pulse: f32 =
                                    if running { 0.55 + 0.45 * (t * 2.2).sin() as f32 } else { 0.35 };
                                let tip = Pos2::new(rect.min.x + fw, rect.center().y);
                                ui.painter().circle_filled(tip, 7.0, MINT.gamma_multiply(0.12 * pulse));
                                ui.painter().circle_filled(tip, 4.5, MINT.gamma_multiply(0.28 * pulse));
                                ui.painter()
                                    .circle_filled(tip, 2.4, MINT.gamma_multiply(0.65 + 0.35 * pulse));

                                ui.add_space(7.0);
                                // the score line: the percentage, and an honest ETA
                                ui.horizontal(|ui| {
                                    let mut job = egui::text::LayoutJob::default();
                                    job.append(
                                        &format!("{:.1}%", (toward * 100.0).min(100.0)),
                                        0.0,
                                        egui::TextFormat {
                                            font_id: play_bold(20.0),
                                            color: MINT,
                                            ..Default::default()
                                        },
                                    );
                                    ui.label(job);
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        caps(ui, &payout_eta(&pi, mhs, earned), 9.5, MUTE);
                                    });
                                });
                                ui.add_space(7.0);
                                card_row(ui, "maturing", &format!("{:.5} ERG", pi.pending_erg));
                                card_row(ui, "credited", &format!("{:.5} ERG", pi.balance_erg));
                                if pi.paid_erg > 0.0 {
                                    card_row(ui, "paid out", &format!("{:.5} ERG", pi.paid_erg));
                                }
                            }
                        }
                        None => {
                            ui.label(
                                egui::RichText::new("wallet unavailable — could not read the seed file")
                                    .color(Color32::from_rgb(255, 140, 140))
                                    .size(12.0),
                            );
                        }
                    }
                });

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
                caps(ui, "every share re-verified on-cpu before it is sent", 8.5, MUTE.gamma_multiply(0.8));
                ui.add_space(2.0);
                caps(ui, "mines autolykos v2 to herominers under your address", 8.5, MUTE.gamma_multiply(0.8));
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

/// The honest countdown to the first payout: remaining ERG at the better of
/// the pool-measured 24h rate and the live local rate, against live network
/// difficulty. Tail emission only (3 ERG/block) — fees make it arrive sooner,
/// never later.
fn payout_eta(pi: &pool::PoolInfo, local_mhs: f64, earned: f64) -> String {
    let rate_mhs = pi.hashrate_24h_mhs.max(local_mhs);
    if pi.difficulty <= 0.0 || rate_mhs <= 0.01 {
        return format!("payout at {} erg — mine to fill the bar", pi.threshold_erg);
    }
    let remaining = (pi.threshold_erg - earned).max(0.0);
    if remaining <= 0.0 {
        return "payout on the next hourly run".into();
    }
    let net_hs = pi.difficulty / pool::BLOCK_TIME_S;
    let blocks_per_day = 86_400.0 / pool::BLOCK_TIME_S;
    let erg_per_day = rate_mhs * 1e6 / net_hs * blocks_per_day * pool::BLOCK_REWARD_ERG;
    let hours = remaining / erg_per_day * 24.0;
    let human = if hours < 1.0 {
        format!("{:.0} min", (hours * 60.0).max(1.0))
    } else if hours < 48.0 {
        format!("{:.0} h", hours)
    } else {
        format!("{:.1} d", hours / 24.0)
    };
    format!("≈ {human} to the {} erg payout", pi.threshold_erg)
}

/// A key/value row inside a card (no outer margins — the card supplies them).
fn card_row(ui: &mut egui::Ui, key: &str, val: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(key).color(MINT.gamma_multiply(0.85)).size(11.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(val).color(CREAM.gamma_multiply(0.88)).size(11.5).monospace());
        });
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
