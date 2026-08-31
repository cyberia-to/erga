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
mod pools;
mod store;
mod stats;

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Sense, Stroke, Vec2};

use balance::BalanceState;
use erga_miner::engine::Progress;
use miner::Miner;
use std::sync::Arc;

const BG: Color32 = Color32::from_rgb(3, 5, 4); // near-black with a faint green cast
const MINT: Color32 = Color32::from_rgb(125, 255, 196);
const CREAM: Color32 = Color32::from_rgb(235, 245, 240);
const MUTE: Color32 = Color32::from_rgb(90, 110, 100);
/// Every pill in the header is exactly this tall. egui sizes a ComboBox from
/// `interact_size` and a bare shape from whatever you allocate, so matching
/// the two by formula does not hold — pinning both to one number does.
const CTRL_H: f32 = 23.0;

fn main() -> eframe::Result<()> {
    // `erga mine [host] [port] [address]` mines headless — same engine, no
    // window. Everything is optional: with no arguments it uses the pool you
    // picked in the app and the wallet the app generated for you.
    //
    // Headless runs the engine in-process, which the GUI deliberately does
    // not do: eframe holds an OpenGL context, and pairing that with the
    // miner's Metal work in one process is what used to abort the app. With
    // no window there is no second graphics API, so it is safe here.
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(|s| s.as_str()) == Some("mine") {
        let pool = &pools::POOLS[pools::load_choice()];
        let host = argv.get(2).cloned().unwrap_or_else(|| pool.host.to_string());
        let port = argv.get(3).and_then(|s| s.parse().ok()).unwrap_or(pool.port);
        let address = match argv.get(4) {
            Some(a) => a.clone(),
            None => match erga_wallet::Wallet::load_or_create() {
                Ok(w) => {
                    println!("mining to your wallet: {}", w.address);
                    w.address
                }
                Err(e) => {
                    eprintln!("no address given and no wallet available: {e}");
                    std::process::exit(1);
                }
            },
        };
        println!("pool: {host}:{port}");
        erga_miner::cli::mine(host, port, address, argv.iter().any(|a| a == "--machine"));
        return Ok(());
    }

    // ERGA_WIN=1600x1000 overrides the initial window size (dev/testing).
    let size = std::env::var("ERGA_WIN")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.parse().ok()?, h.parse().ok()?])
        })
        .unwrap_or([500.0, 1000.0]);
    // The Dock tile of a *running* app comes from NSApplication's icon, not
    // from the bundle's .icns — and winit resets it when no icon is given, so
    // macOS falls back to a generated letter placeholder. Handing eframe the
    // same artwork the bundle ships keeps the icon right while we run.
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(size)
        .with_min_inner_size([440.0, 620.0])
        .with_title("erga");
    match eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(e) => eprintln!("icon: {e}"),
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
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
    pool_idx: usize,
    wallet: Result<erga_wallet::Wallet, String>,
    show_backup: bool,
    spin: f32,
    last_balance: std::time::Instant,
    sys: stats::Sys,
    store: store::Store,
    /// When the current mining session began, for the effective rate.
    session_start: Option<std::time::Instant>,
    last_save: std::time::Instant,
}

impl App {
    fn new() -> Self {
        let wallet = erga_wallet::Wallet::load_or_create();
        let balance = BalanceState::default();
        let idx = pools::load_choice();
        let pool = pool::PoolState::default();
        if let Ok(w) = &wallet {
            balance.fetch(w.address.clone()); // show balance immediately
            pool.fetch(w.address.clone(), idx); // and what the pool owes us
        }
        App {
            miner: Miner::new(),
            balance,
            pool,
            pool_idx: idx,
            wallet,
            show_backup: false,
            spin: 0.0,
            last_balance: std::time::Instant::now(),
            sys: stats::Sys::new(),
            store: store::Store::load(),
            session_start: None,
            last_save: std::time::Instant::now(),
        }
    }

    /// Begin mining, applying the solo prefix if it is on. herominers routes
    /// a `solo:` address to solo mining: you keep whole blocks and get
    /// nothing in between, so the pool's shared payout no longer applies.
    fn begin(&mut self) {
        let Some(addr) = self.address().map(|a| a.to_string()) else {
            self.miner.p.set_status("wallet unavailable");
            return;
        };
        let solo = self.solo();
        let (host, port, prefix) = pools::endpoint(self.pool_idx, solo);
        let addr = format!("{prefix}{addr}");
        store::log(&format!(
            "start pool={} {host}:{port} solo={solo}",
            pools::get(self.pool_idx).label
        ));
        self.session_start = Some(std::time::Instant::now());
        self.miner.start(addr, host, port);
    }

    /// Stop mining and fold this session's counters into the all-time totals.
    fn end(&mut self) {
        use std::sync::atomic::Ordering::Relaxed;
        let p = &self.miner.p;
        let (a, r, d, h) = (
            p.accepted.load(Relaxed),
            p.rejected.load(Relaxed),
            p.donated.load(Relaxed),
            p.hashed.load(Relaxed),
        );
        self.miner.stop();
        self.store.accepted += a;
        self.store.rejected += r;
        self.store.donated += d;
        self.store.hashed += h;
        self.store.save();
        self.session_start = None;
        store::log(&format!("stop accepted={a} rejected={r} donated={d} hashed={h}"));
    }

    /// Solo only counts where the chosen pool offers it.
    fn solo(&self) -> bool {
        self.store.solo && pools::has_solo(self.pool_idx)
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

        // Effective rate: hashes over the whole session, so the seconds spent
        // rebuilding the table each block are counted honestly.
        let eff_mhs = self.session_start.and_then(|t| {
            let secs = t.elapsed().as_secs_f64();
            let h = self.miner.p.hashed.load(std::sync::atomic::Ordering::Relaxed);
            (secs > 5.0 && h > 0).then(|| h as f64 / secs / 1e6)
        });
        let all_time = {
            use std::sync::atomic::Ordering::Relaxed;
            (
                self.store.accepted + self.miner.p.accepted.load(Relaxed),
                self.store.hashed + self.miner.p.hashed.load(Relaxed),
            )
        };
        // Checkpoint the totals every minute so a crash costs at most that.
        if running && self.last_save.elapsed().as_secs() >= 60 {
            use std::sync::atomic::Ordering::Relaxed;
            let p = &self.miner.p;
            self.store.save_with(
                p.accepted.load(Relaxed),
                p.rejected.load(Relaxed),
                p.donated.load(Relaxed),
                p.hashed.load(Relaxed),
            );
            self.last_save = std::time::Instant::now();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let avail_w = ui.available_width();
            let avail_h = ui.available_height();
            ui.add_space(16.0);

            // ── header — wordmark left, pool + badge right ────────────
            ui.horizontal(|ui| {
                ui.add_space(22.0);
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
                    // ComboBox takes its height from interact_size; the badge
                    // takes CTRL_H. Same number, so the row cannot step.
                    ui.spacing_mut().interact_size.y = CTRL_H;
                    ui.add_space(22.0);
                    badge(ui, "experimental");
                    ui.add_space(8.0);
                    let prev = self.pool_idx;
                    egui::ComboBox::from_id_source("pool")
                        .selected_text(
                            egui::RichText::new(pools::POOLS[self.pool_idx].label).size(10.5),
                        )
                        .show_ui(ui, |ui| {
                            for (i, pl) in pools::POOLS.iter().enumerate() {
                                ui.selectable_value(&mut self.pool_idx, i, pl.label);
                            }
                        });
                    if pools::has_solo(self.pool_idx) {
                        let mut solo_on = self.store.solo;
                        if pill_toggle(ui, "solo", &mut solo_on) {
                            self.store.solo = solo_on;
                            self.store.save();
                            if running {
                                self.end();
                                self.begin();
                            }
                        }
                        ui.add_space(8.0);
                    }
                    if self.pool_idx != prev {
                        pools::save_choice(self.pool_idx);
                        // the new pool keeps its own books
                        if let Some(a) = self.address().map(|a| a.to_string()) {
                            self.pool.fetch(a, self.pool_idx);
                        }
                        if running {
                            // hop pools live: land on the new door mid-flight
                            self.end();
                            self.begin();
                        }
                    }
                });
            });

            // ── one refresh, one snapshot, both layouts ───────────────
            if let Some(addr) = self.address().map(|a| a.to_string()) {
                if self.last_balance.elapsed().as_secs() >= 30 {
                    self.balance.fetch(addr.clone());
                    if pools::has_ledger(self.pool_idx) {
                        self.pool.fetch(addr, self.pool_idx);
                    }
                    self.last_balance = std::time::Instant::now();
                }
            }
            self.sys.refresh();
            let p = self.miner.p.clone();
            let mhs = p.mhs();
            let (cpu, mem, net_kbs) = (self.sys.cpu, self.sys.mem, self.sys.down_kbs);
            let on_chain = self.balance.inner.lock().unwrap().erg;
            let has_ledger = pools::has_ledger(self.pool_idx);
            let solo = self.solo();
            let addr_opt = self.address().map(|a| a.to_string());
            let mut want_backup = false;
            let mut start_stop = false;
            let pool_label = pools::POOLS[self.pool_idx].label;

            let wide = avail_w >= 1000.0;
            if wide {
                // ── the HUD: crystal sun, panels in orbit ─────────────
                let side = 40.0;
                let (lw, rw, gap) = (320.0, 380.0, 30.0);
                let cw = (avail_w - side * 2.0 - lw - rw - gap * 2.0 - 48.0).max(260.0);
                let cr = (cw * 0.34).min(avail_h * 0.22).clamp(120.0, 240.0);
                // the centre column: crystal block + the hero rate beneath it
                let centre_h = cr * 2.05 + 120.0;
                ui.add_space(((avail_h - centre_h - 210.0) / 2.0).max(10.0));

                ui.horizontal(|ui| {
                    ui.add_space(side);
                    panel_frame(ui, lw, 0.0, |ui| {
                        machine_panel(ui, &p, cpu, mem, net_kbs, mhs, running, eff_mhs, all_time);
                    });
                    ui.add_space(gap);
                    // the crystal and the hero rate under it
                    ui.vertical(|ui| {
                        ui.set_width(cw);
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(cw, cr * 2.05), Sense::click());
                        draw_crystal(ui, rect.center(), cr, running, self.spin, resp.hovered());
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            if running { "MINING" } else { "START" },
                            FontId::proportional((cr * 0.19).clamp(22.0, 38.0)),
                            if running { BG } else { MINT },
                        );
                        if resp.clicked() {
                            start_stop = true;
                        }
                        if resp.hovered() {
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        }
                        ui.add_space(10.0);
                        hero_rate(ui, mhs, running, &p, (cr * 0.44).clamp(52.0, 76.0));
                    });
                    ui.add_space(gap);
                    panel_frame(ui, rw, 0.0, |ui| {
                        let pi = self.pool.inner.lock().unwrap();
                        payout_panel(ui, &pi, has_ledger, solo, on_chain, mhs, running, &p);
                    });
                });

                ui.add_space(18.0);
                if let Some(addr) = &addr_opt {
                    wallet_strip(ui, addr, avail_w, &mut want_backup);
                }
            } else {
                // ── narrow: the same organs, stacked on one axis ──────
                let col_w = (avail_w - 44.0).min(600.0);
                let side = ((avail_w - col_w) / 2.0).max(0.0);
                let cr = (col_w * 0.30).clamp(112.0, 155.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        let (rect, resp) = ui.allocate_at_least(
                            Vec2::new(ui.available_width(), cr * 2.25),
                            Sense::click(),
                        );
                        draw_crystal(ui, rect.center(), cr, running, self.spin, resp.hovered());
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            if running { "MINING" } else { "START" },
                            FontId::proportional((cr * 0.2).clamp(20.0, 32.0)),
                            if running { BG } else { MINT },
                        );
                        if resp.clicked() {
                            start_stop = true;
                        }
                        if resp.hovered() {
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        }
                        ui.add_space(4.0);
                        ui.vertical_centered(|ui| {
                            hero_rate(ui, mhs, running, &p, 58.0);
                        });
                        ui.add_space(16.0);

                        ui.horizontal(|ui| {
                            ui.add_space(side);
                            ui.vertical(|ui| {
                                ui.set_width(col_w);
                                panel_frame(ui, col_w, 0.0, |ui| {
                                    machine_panel(ui, &p, cpu, mem, net_kbs, mhs, running, eff_mhs, all_time);
                                });
                                ui.add_space(12.0);
                                panel_frame(ui, col_w, 0.0, |ui| {
                                    let pi = self.pool.inner.lock().unwrap();
                                    payout_panel(ui, &pi, has_ledger, solo, on_chain, mhs, running, &p);
                                });
                                ui.add_space(14.0);
                                if let Some(addr) = &addr_opt {
                                    wallet_strip(ui, addr, col_w, &mut want_backup);
                                }
                                ui.add_space(14.0);
                                ui.vertical_centered(|ui| {
                                    honest_footer(ui, pool_label);
                                });
                                ui.add_space(12.0);
                            });
                        });
                    });
            }

            if want_backup {
                self.show_backup = true;
            }
            if start_stop {
                if running {
                    self.end();
                } else {
                    self.begin();
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

            // the footer rides at the bottom only in the wide HUD; in the
            // narrow layout it lives inside the scroll, after the content.
            if wide {
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    honest_footer(ui, pools::POOLS[self.pool_idx].label);
                });
            }
        });
    }
}

/// A pill that sits level with the buttons beside it. egui sizes a button as
/// max(interact_size.y, text + 2*button_padding.y); matching that formula —
/// and the button font — is what keeps the header from stepping.
/// A pill that toggles, drawn to the same metric as `badge` so the header
/// reads as one row. egui's checkbox draws a circle and a label, which is a
/// different shape language from the controls beside it.
fn pill_toggle(ui: &mut egui::Ui, text: &str, on: &mut bool) -> bool {
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

fn badge(ui: &mut egui::Ui, text: &str) {
    let galley = ui.painter().layout_no_wrap(text.into(), FontId::proportional(10.5), MINT);
    let pad = ui.spacing().button_padding;
    let size = Vec2::new(galley.size().x + pad.x * 2.0, CTRL_H);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_stroke(rect, 999.0, Stroke::new(1.0, MINT.gamma_multiply(0.5)));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, MINT);
}

/// A dashboard meter: label, a segmented fill bar (0..1), the value in bold.
/// Segments make the level readable at a glance instead of a smooth smear.
fn meter(ui: &mut egui::Ui, label: &str, frac: f32, val: &str) {
    ui.vertical(|ui| {
        caps(ui, label, 9.0, MINT.gamma_multiply(0.9));
        ui.add_space(4.0);
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 9.0), Sense::hover());
        ui.painter().rect_filled(rect, 4.5, Color32::from_rgb(24, 34, 29));
        let f = frac.clamp(0.0, 1.0);
        let col = if f > 0.9 { Color32::from_rgb(255, 180, 120) } else { MINT };
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, Vec2::new((w * f).max(2.0), 9.0)),
            4.5,
            col,
        );
        for i in 1..8 {
            let x = rect.min.x + w * i as f32 / 8.0;
            ui.painter().line_segment(
                [Pos2::new(x, rect.min.y + 1.0), Pos2::new(x, rect.max.y - 1.0)],
                Stroke::new(1.0, BG),
            );
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

/// A bordered panel. `min_h` of 0 lets the panel size to its content.
fn panel_frame(
    ui: &mut egui::Ui,
    w: f32,
    min_h: f32,
    add: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::none()
        .stroke(Stroke::new(1.0, MINT.gamma_multiply(0.22)))
        .rounding(14.0)
        .inner_margin(egui::Margin::symmetric(18.0, 16.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(w - 36.0);
                if min_h > 0.0 {
                    ui.set_min_height(min_h - 32.0);
                }
                add(ui);
            });
        });
}

/// The hero number: the live hashrate, the biggest thing on the screen
/// after the crystal itself.
fn hero_rate(ui: &mut egui::Ui, mhs: f64, running: bool, p: &Arc<Progress>, size: f32) {
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

/// THE MACHINE — what your Mac is doing right now, in meters and counts.
fn machine_panel(
    ui: &mut egui::Ui,
    p: &Arc<Progress>,
    cpu: f32,
    mem: f32,
    net_kbs: f64,
    mhs: f64,
    running: bool,
    // eff_mhs: hashes/sec over the whole session, table rebuilds included —
    // the rate the pool will agree with. all_time: accepted shares and
    // hashes across every run, ever.
    eff_mhs: Option<f64>,
    all_time: (u64, u64),
) {
    use std::sync::atomic::Ordering as O;
    caps(ui, "the machine", 10.5, MUTE);
    ui.add_space(12.0);
    // the graphic heart of the panel — four live meters
    ui.columns(2, |c| {
        meter(&mut c[0], "cpu", cpu, &format!("{:.0}%", cpu * 100.0));
        // GPU has no privilege-free utilisation read; the hashrate is its
        // honest signal, scaled against ~80 MH/s (the M4 Max ceiling).
        meter(&mut c[1], "gpu", (mhs / 80.0) as f32, &format!("{mhs:.0} MH/s"));
    });
    ui.add_space(12.0);
    ui.columns(2, |c| {
        meter(&mut c[0], "ram", mem, &format!("{:.0}%", mem * 100.0));
        meter(
            &mut c[1],
            "net",
            (net_kbs / 2048.0) as f32,
            &format!("{net_kbs:.0} KB/s"),
        );
    });
    ui.add_space(14.0);
    ui.separator();
    ui.add_space(10.0);
    let dev = p.device.lock().unwrap().clone();
    card_row(ui, "device", if dev.is_empty() { "—" } else { &dev });
    {
        let acc = p.accepted.load(O::Relaxed);
        let rej = p.rejected.load(O::Relaxed);
        card_row(
            ui,
            "shares",
            &if rej > 0 { format!("{acc}  ({rej} rejected)") } else { format!("{acc}") },
        );
    }
    {
        let h = p.height.load(O::Relaxed);
        card_row(ui, "block", &(if h > 0 { h.to_string() } else { "—".into() }));
    }
    card_row(ui, "hashed", &human(p.hashed.load(O::Relaxed)));
    if let Some(e) = eff_mhs {
        // The big number is the rate while mining; this one counts the
        // seconds spent rebuilding the table each block too.
        card_row(ui, "effective", &format!("{e:.1} MH/s"));
    }
    if all_time.0 > 0 || all_time.1 > 0 {
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);
        caps(ui, "all time", 9.5, MUTE.gamma_multiply(0.85));
        ui.add_space(4.0);
        card_row(ui, "shares", &all_time.0.to_string());
        card_row(ui, "hashed", &human(all_time.1));
    }
    if !running {
        ui.add_space(6.0);
        caps(ui, "press the crystal to begin", 9.0, MUTE.gamma_multiply(0.8));
    }
}

/// THE PAYOUT — what the work returns: the game, the ledger, the balance.
fn payout_panel(
    ui: &mut egui::Ui,
    pi: &pool::PoolInfo,
    has_ledger: bool,
    solo: bool,
    on_chain: Option<f64>,
    mhs: f64,
    running: bool,
    p: &Arc<Progress>,
) {
    ui.horizontal(|ui| {
        caps(ui, "the payout", 10.5, MUTE);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if has_ledger && pi.ok {
                caps(
                    ui,
                    &format!("sees {:.0} mh/s · 24h", pi.hashrate_24h_mhs),
                    9.0,
                    MUTE.gamma_multiply(0.85),
                );
            }
        });
    });
    ui.add_space(12.0);
    if !has_ledger {
        caps(ui, "this pool has no in-app ledger yet", 9.5, MUTE);
        ui.add_space(3.0);
        caps(ui, "track earnings on its site", 9.5, MUTE);
    } else if pi.ok {
        payout_game(ui, pi, mhs, running, solo);
    } else {
        caps(ui, "reading the pool ledger…", 9.5, MUTE);
    }

    // the development donation, always visible, never hidden
    let donated = p.donated.load(std::sync::atomic::Ordering::Relaxed);
    ui.add_space(4.0);
    card_row(ui, "to development", &format!("{donated} shares (5%)"));

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(10.0);
    caps(ui, "your balance on chain", 9.5, MUTE);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let (text, dim) = match on_chain {
            Some(e) => (format!("{e:.4}"), false),
            None => ("0.0000".to_string(), true),
        };
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &text,
            0.0,
            egui::TextFormat {
                font_id: play_bold(34.0),
                color: if dim { MINT.gamma_multiply(0.6) } else { MINT },
                ..Default::default()
            },
        );
        ui.label(job);
        ui.add_space(3.0);
        caps(ui, "erg", 11.0, MUTE);
    });
}

/// What the app is honestly doing, in two quiet lines.
fn honest_footer(ui: &mut egui::Ui, pool_label: &str) {
    caps(ui, "every share re-verified on-cpu before it is sent", 8.5, MUTE.gamma_multiply(0.8));
    ui.add_space(2.0);
    caps(
        ui,
        &format!("mines autolykos v2 to {pool_label} under your address"),
        8.5,
        MUTE.gamma_multiply(0.8),
    );
    ui.add_space(2.0);
    caps(ui, "5% of shares fund development", 8.5, MUTE.gamma_multiply(0.8));
}

/// The wallet strip — identity, present but out of the game's way.
fn wallet_strip(ui: &mut egui::Ui, addr: &str, w: f32, want_backup: &mut bool) {
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - w.min(660.0)) / 2.0).max(0.0));
        caps(ui, "wallet", 9.5, MUTE);
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new(addr)
                .monospace()
                .size(10.5)
                .color(CREAM.gamma_multiply(0.72)),
        );
        ui.add_space(8.0);
        if ui.button(egui::RichText::new("copy").size(10.0)).clicked() {
            ui.output_mut(|o| o.copied_text = addr.to_string());
        }
        if ui.button(egui::RichText::new("back up").size(10.0)).clicked() {
            *want_backup = true;
        }
        if ui.button(egui::RichText::new("logs").size(10.0)).clicked() {
            store::reveal_log();
        }
    });
}

/// ERG earned per day at the better of the pool-measured 24h rate and the
/// live local rate, against live network difficulty. Tail emission only
/// (3 ERG/block) — fees make earnings arrive sooner, never later. None
/// until the difficulty and a hashrate are both known.
fn erg_per_day(pi: &pool::PoolInfo, local_mhs: f64) -> Option<f64> {
    let rate_mhs = pi.hashrate_24h_mhs.max(local_mhs);
    if pi.difficulty <= 0.0 || rate_mhs <= 0.01 {
        return None;
    }
    let net_hs = pi.difficulty / pool::BLOCK_TIME_S;
    let blocks_per_day = 86_400.0 / pool::BLOCK_TIME_S;
    Some(rate_mhs * 1e6 / net_hs * blocks_per_day * pool::BLOCK_REWARD_ERG)
}

/// The payout game — segmented bar, pulsing tip, the score and the honest
/// countdown, then the ledger rows. One source for both layouts.
fn payout_game(
    ui: &mut egui::Ui,
    pi: &pool::PoolInfo,
    local_mhs: f64,
    running: bool,
    solo: bool,
) {
    // Solo has no shared payout to fill: you find a whole block or you find
    // nothing. The bar would be a lie, so it is replaced by the only number
    // that means anything there — how long a block takes at this rate.
    if solo {
        caps(ui, "solo — a whole block, or nothing", 10.0, MINT);
        ui.add_space(6.0);
        let rate = pi.hashrate_24h_mhs.max(local_mhs) * 1e6;
        if pi.difficulty > 0.0 && rate > 1e4 {
            let days = pi.difficulty / rate / 86_400.0;
            let mut job = egui::text::LayoutJob::default();
            job.append(
                &if days >= 1.0 { format!("{days:.0}") } else { format!("{:.1}", days * 24.0) },
                0.0,
                egui::TextFormat { font_id: play_bold(20.0), color: MINT, ..Default::default() },
            );
            ui.label(job);
            caps(
                ui,
                if days >= 1.0 { "days per block, on average" } else { "hours per block, on average" },
                9.0,
                MUTE,
            );
            ui.add_space(4.0);
            caps(ui, &format!("the block pays {:.0} erg", pool::BLOCK_REWARD_ERG), 9.0, MUTE);
        } else {
            caps(ui, "mine to see the odds at your rate", 9.0, MUTE);
        }
        ui.add_space(8.0);
        card_row(ui, "credited", &format!("{:.5} ERG", pi.balance_erg.max(0.0)));
        if pi.paid_erg > 0.0 {
            card_row(ui, "paid out", &format!("{:.5} ERG", pi.paid_erg));
        }
        return;
    }
    let earned = pi.balance_erg + pi.pending_erg;
    let toward = (earned / pi.threshold_erg) as f32;

    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 8.0), Sense::hover());
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
            [Pos2::new(x, rect.min.y + 1.5), Pos2::new(x, rect.max.y - 1.5)],
            Stroke::new(1.0, BG),
        );
    }
    let t = ui.input(|i| i.time);
    let pulse: f32 = if running { 0.55 + 0.45 * (t * 2.2).sin() as f32 } else { 0.35 };
    let tip = Pos2::new(rect.min.x + fw, rect.center().y);
    ui.painter().circle_filled(tip, 7.0, MINT.gamma_multiply(0.12 * pulse));
    ui.painter().circle_filled(tip, 4.5, MINT.gamma_multiply(0.28 * pulse));
    ui.painter().circle_filled(tip, 2.4, MINT.gamma_multiply(0.65 + 0.35 * pulse));

    ui.add_space(7.0);
    ui.horizontal(|ui| {
        let mut job = egui::text::LayoutJob::default();
        job.append(
            &format!("{:.1}%", (toward * 100.0).min(100.0)),
            0.0,
            egui::TextFormat { font_id: play_bold(20.0), color: MINT, ..Default::default() },
        );
        ui.label(job);
    });
    caps(ui, &payout_eta(pi, local_mhs, earned), 9.0, MUTE);
    ui.add_space(8.0);
    if let Some(day) = erg_per_day(pi, local_mhs) {
        let month = day * 30.0;
        let usd = if pi.price_usd > 0.0 {
            format!("  ·  ${:.2}", month * pi.price_usd)
        } else {
            String::new()
        };
        card_row(ui, "a month at this pace", &format!("≈ {month:.2} ERG{usd}"));
    }
    card_row(ui, "maturing", &format!("{:.5} ERG", pi.pending_erg.max(0.0)));
    card_row(ui, "credited", &format!("{:.5} ERG", pi.balance_erg.max(0.0)));
    if pi.paid_erg > 0.0 {
        card_row(ui, "paid out", &format!("{:.5} ERG", pi.paid_erg));
    }
}

/// The honest countdown to the first payout.
fn payout_eta(pi: &pool::PoolInfo, local_mhs: f64, earned: f64) -> String {
    let Some(per_day) = erg_per_day(pi, local_mhs) else {
        return format!("payout at {} erg — mine to fill the bar", pi.threshold_erg);
    };
    let remaining = (pi.threshold_erg - earned).max(0.0);
    if remaining <= 0.0 {
        return "payout on the next hourly run".into();
    }
    let hours = remaining / per_day * 24.0;
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
