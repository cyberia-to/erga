//! erga — one-button ERGO miner + wallet for Apple Silicon.
//!
//! Open it: a self-custodial Ergo wallet is generated for you. Press the
//! crystal and it mines Autolykos v2 to a pool under that address, on the
//! honeycrisp zero-copy GPU kernel (32 MH/s at 8.3 W sustained on an M4
//! Max — 2.4× an RTX 3090 per watt). Watch the hashrate, the accepted
//! shares, and your balance grow.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod balance;
mod chime;
mod miner;
mod panels;
mod purse;
mod theme;
mod widgets;
mod pool;
mod pools;
mod store;
mod stats;

use eframe::egui;
use egui::{Align2, Color32, FontId, Sense, Vec2};

use balance::BalanceState;
use panels::{machine_panel, payout_panel};
use purse::{action_bar, backup_screen, wallet_block};
use theme::{badge, caps, load_icon, pill_toggle, play_bold, setup_fonts, setup_style};
use widgets::{big_balance, draw_crystal, hero_rate, human, panel_frame, start_hint};
use miner::Miner;

// Colour is a language here, not decoration. Nothing is given a colour it
// has not earned:
//   MINT   what you gain — earnings, hashrate, the crystal while it runs
//   AMBER  what it costs — cpu, memory, network: the machine working for you
//   CORAL  what is wrong — rejected shares, failures
//   SKY    what the chain says — height, difficulty: facts you do not own
pub const BG: Color32 = Color32::from_rgb(3, 5, 4); // near-black with a faint green cast
pub const MINT: Color32 = Color32::from_rgb(125, 255, 196);
pub const AMBER: Color32 = Color32::from_rgb(255, 186, 92);
pub const CORAL: Color32 = Color32::from_rgb(255, 122, 122);
pub const SKY: Color32 = Color32::from_rgb(126, 197, 255);
pub const CREAM: Color32 = Color32::from_rgb(235, 245, 240);
pub const MUTE: Color32 = Color32::from_rgb(90, 110, 100);
/// Every pill in the header is exactly this tall. egui sizes a ComboBox from
/// `interact_size` and a bare shape from whatever you allocate, so matching
/// the two by formula does not hold — pinning both to one number does.
pub const CTRL_H: f32 = 23.0;
/// The height the start hint occupies, reserved even when it is not drawn.
pub const HINT_H: f32 = 16.0;

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






struct App {
    miner: Miner,
    balance: BalanceState,
    pool: pool::PoolState,
    pool_idx: usize,
    wallet: Result<erga_wallet::Wallet, String>,
    /// When the crystal was last pressed, for the press animation.
    pressed_at: Option<std::time::Instant>,
    /// Accepted shares already announced, so each is celebrated exactly once.
    heard_shares: u64,
    /// Whether the seed is on screen. It waits to be dismissed: a timer suits
    /// something that appeared uninvited, but this one was opened on purpose
    /// and is being copied onto paper, and paper is slow.
    show_seed: bool,
    spin: f32,
    last_balance: std::time::Instant,
    sys: stats::Sys,
    store: store::Store,
    /// Loaded on first paint; `Some(None)` means the artwork would not decode.
    icon: Option<Option<egui::TextureHandle>>,
    /// When the current mining session began, for the effective rate.
    session_start: Option<std::time::Instant>,
    last_save: std::time::Instant,
}

impl App {
    fn new() -> Self {
        chime::ensure();
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
            pressed_at: None,
            heard_shares: 0,
            show_seed: false,
            spin: 0.0,
            last_balance: std::time::Instant::now(),
            sys: stats::Sys::new(),
            store: store::Store::load(),
            icon: None,
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

    /// Start mining as soon as the window is up. Set ERGA_AUTOSTART=1 for a
    /// login item, or for a machine whose only job is to mine.
    fn autostart(&self) -> bool {
        std::env::var("ERGA_AUTOSTART").map(|v| v != "0").unwrap_or(false)
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
        if self.autostart() && self.session_start.is_none() && !self.miner.is_running() {
            self.begin();
        }
        let running = self.miner.is_running();
        // A share is the only thing here that is genuinely good news, so it is
        // the only thing that makes a sound on its own.
        let accepted = self.miner.p.accepted.load(std::sync::atomic::Ordering::Relaxed);
        if accepted > self.heard_shares {
            if self.heard_shares > 0 || accepted == 1 {
                chime::share();
            }
            self.heard_shares = accepted;
        } else if accepted < self.heard_shares {
            self.heard_shares = accepted; // a new session restarted the count
        }
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
            // The seed takes the whole window while it is up: it is not a
            // dialog over the app, it is the only thing that should be on
            // screen — and it puts itself away.
            if self.show_seed {
                let seed = self.wallet.as_ref().ok().map(|w| w.mnemonic.clone());
                if backup_screen(ui, seed.as_deref()) {
                    self.show_seed = false;
                }
                return;
            }
            // A press dips the crystal and lets it spring back over ~220 ms.
            // Small enough to feel like contact rather than like a transition.
            let press = self
                .pressed_at
                .map(|t| t.elapsed().as_secs_f32())
                .filter(|e| *e < 0.22)
                .map(|e| {
                    let k = e / 0.22;
                    1.0 - 0.06 * (1.0 - k) * (k * 18.0).cos().abs()
                })
                .unwrap_or(1.0);
            if self.pressed_at.is_some() && press >= 1.0 {
                self.pressed_at = None;
            }
            let avail_w = ui.available_width();
            ui.add_space(16.0);

            // ── header — wordmark left, pool + badge right ────────────
            ui.horizontal(|ui| {
                ui.add_space(22.0);
                if let Some(tex) = self.icon.get_or_insert_with(|| load_icon(ui.ctx())).clone() {
                    ui.add(
                        egui::Image::new(egui::load::SizedTexture::new(
                            tex.id(),
                            Vec2::splat(26.0),
                        ))
                        .fit_to_exact_size(Vec2::splat(26.0)),
                    );
                    ui.add_space(9.0);
                }
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    "Erga",
                    0.0,
                    egui::TextFormat {
                        font_id: play_bold(21.0),
                        color: CREAM,
                        extra_letter_spacing: 0.6,
                        ..Default::default()
                    },
                );
                ui.label(job);
                ui.add_space(8.0);
                caps(ui, &format!("v{}", env!("CARGO_PKG_VERSION")), 9.5, MUTE.gamma_multiply(0.85));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // ComboBox takes its height from interact_size; the badge
                    // takes CTRL_H. Same number, so the row cannot step.
                    ui.spacing_mut().interact_size.y = CTRL_H;
                    ui.add_space(22.0);
                    badge(ui, "beta", AMBER);
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
            self.sys.refresh(self.miner.pid());
            let p = self.miner.p.clone();
            let mhs = p.mhs();
            let (cpu, mem, net_kbs) = (self.sys.cpu, self.sys.mem, self.sys.down_kbs);
            let (miner_cpu, miner_mem) = (self.sys.miner_cpu, self.sys.miner_mem);
            let on_chain = self.balance.inner.lock().unwrap().erg;
            let has_ledger = pools::has_ledger(self.pool_idx);
            let solo = self.solo();
            let addr_opt = self.address().map(|a| a.to_string());
            let mut want_backup = false;
            let mut want_report = false;
            let mut start_stop = false;

            let wide = avail_w >= 1000.0;
            // The header has drawn by now, so ask again: the value taken
            // before it is larger than what is actually left, and every
            // centring below is computed from this number.
            let avail_h = ui.available_height();
            if wide {
                // ── the HUD ───────────────────────────────────────────
                // Read top to bottom: the balance is what this is for, the
                // crystal is how you get it, the wallet is where it lands.
                // So the balance opens and the actions close, and the crystal
                // sits dead centre of the window with the two panels — cost
                // on the left, return on the right — level with it.
                let side = 40.0;
                let (pw, gap) = (330.0, 30.0);
                let cw = (avail_w - side * 2.0 - pw * 2.0 - gap * 2.0).max(260.0);
                // the actions own the foot of the window
                // measured against what the foot actually holds: the bar, the
                // address, and the air around them. Under-reserving it is what
                // pressed the status line against the wallet label.
                let foot_h = 186.0;
                let band_h = (avail_h - foot_h - 16.0).max(340.0);
                // For the crystal's centre to land on band_h/2 while the rate
                // still fits beneath it, the radius cannot exceed half the
                // band less what sits below. Anything larger pushes the wallet
                // out of the foot and over the status line.
                let head_h = 132.0; // caps + the number + the gap under it
                // Only the status line sits under the crystal now — the rate
                // moved inside it. A giant number below competed with the
                // balance above and, kept centred, left no room for either.
                let rate_h = 54.0;
                let cr = (cw * 0.34)
                    .min(band_h / 2.0 - rate_h)
                    .clamp(110.0, 250.0);
                // sized to their content, not to the window: a panel that
                // stretches is mostly empty
                let panel_h = (band_h - 40.0).clamp(300.0, 430.0);

                // The hint's row is reserved whether or not it is drawn:
                // otherwise the whole window jumps upward the moment mining
                // starts, which reads as a glitch rather than as feedback.
                ui.add_space(2.0);
                if running {
                    ui.add_space(HINT_H);
                } else {
                    start_hint(ui);
                }
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.add_space(side);
                    // panels are centred on the crystal, not hung from the top
                    ui.vertical(|ui| {
                        ui.set_width(pw);
                        ui.add_space(((band_h - panel_h) / 2.0).max(0.0));
                        panel_frame(ui, pw, panel_h, |ui| {
                            machine_panel(ui, &p, cpu, mem, miner_cpu, miner_mem, net_kbs, mhs, running, eff_mhs);
                        });
                    });
                    ui.add_space(gap);
                    ui.vertical(|ui| {
                        ui.set_width(cw);
                        // Everything above the crystal is sized so that the
                        // crystal's own centre lands on band_h/2 exactly.
                        ui.add_space(((band_h / 2.0) - cr - head_h).max(4.0));
                        big_balance(ui, on_chain, cw);
                        ui.add_space(34.0);
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(cw, cr * 2.0), Sense::click());
                        draw_crystal(ui, rect.center(), cr * press, running, self.spin, resp.hovered());
                        // Mining, the crystal *is* the readout: the rate lives
                        // where the label was, because a filled crystal has
                        // already said "mining".
                        if running {
                            let c = rect.center();
                            ui.painter().text(
                                c - Vec2::new(0.0, cr * 0.06),
                                Align2::CENTER_CENTER,
                                format!("{mhs:.1}"),
                                play_bold((cr * 0.42).clamp(34.0, 88.0)),
                                BG,
                            );
                            ui.painter().text(
                                c + Vec2::new(0.0, cr * 0.30),
                                Align2::CENTER_CENTER,
                                "MH/S",
                                FontId::proportional((cr * 0.10).clamp(11.0, 18.0)),
                                BG.gamma_multiply(0.75),
                            );
                        } else {
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                "START",
                                FontId::proportional((cr * 0.19).clamp(22.0, 38.0)),
                                MINT,
                            );
                        }
                        if resp.clicked() {
                            start_stop = true;
                        }
                        if resp.hovered() {
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                        }
                        ui.add_space(12.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(p.status.lock().unwrap().clone())
                                    .color(MUTE)
                                    .size(12.5),
                            );
                        });
                    });
                    ui.add_space(gap);
                    ui.vertical(|ui| {
                        ui.set_width(pw);
                        ui.add_space(((band_h - panel_h) / 2.0).max(0.0));
                        panel_frame(ui, pw, panel_h, |ui| {
                            let pi = self.pool.inner.lock().unwrap();
                            payout_panel(ui, &pi, has_ledger, solo, mhs, running, all_time, panel_h - 32.0);
                        });
                    });
                });

                // the address, then the bar hard against the bottom edge
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(20.0);
                    action_bar(ui, addr_opt.as_deref(), &mut want_backup, &mut want_report);
                    ui.add_space(16.0);
                    wallet_block(ui, addr_opt.as_deref());
                    ui.add_space(34.0); // clear of the status line above
                });
            } else {
                // ── narrow: the same organs, stacked on one axis ──────
                let col_w = (avail_w - 44.0).min(600.0);
                let side = ((avail_w - col_w) / 2.0).max(0.0);
                let cr = (col_w * 0.30).clamp(112.0, 155.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        if !running {
                            ui.add_space(6.0);
                            start_hint(ui);
                        }
                        ui.add_space(12.0);
                        big_balance(ui, on_chain, col_w);
                        ui.add_space(26.0);
                        let (rect, resp) = ui.allocate_at_least(
                            Vec2::new(ui.available_width(), cr * 2.25),
                            Sense::click(),
                        );
                        draw_crystal(ui, rect.center(), cr * press, running, self.spin, resp.hovered());
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
                        wallet_block(ui, addr_opt.as_deref());
                        ui.add_space(14.0);
                        action_bar(ui, addr_opt.as_deref(), &mut want_backup, &mut want_report);
                        ui.add_space(18.0);

                        ui.horizontal(|ui| {
                            ui.add_space(side);
                            ui.vertical(|ui| {
                                ui.set_width(col_w);
                                panel_frame(ui, col_w, 0.0, |ui| {
                                    machine_panel(ui, &p, cpu, mem, miner_cpu, miner_mem, net_kbs, mhs, running, eff_mhs);
                                });
                                ui.add_space(12.0);
                                panel_frame(ui, col_w, 0.0, |ui| {
                                    let pi = self.pool.inner.lock().unwrap();
                                    payout_panel(ui, &pi, has_ledger, solo, mhs, running, all_time, 0.0);
                                });
                                ui.add_space(14.0);
                            });
                        });
                    });
            }

            if want_report {
                let pl = pools::get(self.pool_idx);
                let p = &self.miner.p;
                use std::sync::atomic::Ordering::Relaxed;
                let state = format!(
                    "pool: {} ({}:{}) solo={}\nstatus: {}\ndevice: {}\nrate: {:.1} MH/s\n\
                     height: {}\nshares: {} accepted, {} rejected, {} to development\n\
                     hashed this run: {}\nall time: {} shares, {} hashes",
                    pl.label,
                    pl.host,
                    pl.port,
                    self.solo(),
                    p.status.lock().unwrap(),
                    p.device.lock().unwrap(),
                    p.mhs(),
                    p.height.load(Relaxed),
                    p.accepted.load(Relaxed),
                    p.rejected.load(Relaxed),
                    p.donated.load(Relaxed),
                    human(p.hashed.load(Relaxed)),
                    self.store.accepted + p.accepted.load(Relaxed),
                    human(self.store.hashed + p.hashed.load(Relaxed)),
                );
                store::report_bug(&state);
            }
            if want_backup {
                // Copy on open: the words are wanted *somewhere else*, and a
                // person retyping fifteen of them from a screen makes mistakes.
                if let Ok(w) = &self.wallet {
                    ui.output_mut(|o| o.copied_text = w.mnemonic.clone());
                }
                self.show_seed = true;
            }
            if start_stop {
                chime::press();
                self.pressed_at = Some(std::time::Instant::now());
                if running {
                    self.end();
                } else {
                    self.begin();
                }
            }


            // the footer rides at the bottom only in the wide HUD; in the
            // narrow layout it lives inside the scroll, after the content.
            if wide {
            }
        });
    }
}





















