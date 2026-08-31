//! The two panels: what mining costs on the left, what it returns on the
//! right. They are the same object seen twice, which is why they are the same
//! size — and why the projections live here rather than beside the engine.

use eframe::egui;
use egui::{Color32, Pos2, Sense, Stroke, Vec2};
use erga_miner::engine::Progress;
use std::sync::Arc;

use crate::theme::{caps, play_bold};
use crate::widgets::{card_row, card_row_tinted, human, meter};
use crate::{pool, AMBER, BG, CORAL, MINT, MUTE, SKY};

/// THE MACHINE — what your Mac is doing right now, in meters and counts.
pub fn machine_panel(
    ui: &mut egui::Ui,
    p: &Arc<Progress>,
    cpu: f32,
    mem: f32,
    miner_cpu: f32,
    miner_mem: f32,
    net_kbs: f64,
    mhs: f64,
    running: bool,
    // eff_mhs: hashes/sec over the whole session, table rebuilds included —
    // the rate the pool will agree with.
    eff_mhs: Option<f64>,
) {
    let _ = running;
    use std::sync::atomic::Ordering as O;
    caps(ui, "the machine", 10.5, MUTE);
    ui.add_space(12.0);
    // the graphic heart of the panel — four live meters
    ui.columns(2, |c| {
        meter(&mut c[0], "cpu", miner_cpu, cpu, &format!("{:.0}%", cpu * 100.0), AMBER);
        // GPU has no privilege-free utilisation read; the hashrate is its
        // honest signal, scaled against ~80 MH/s (the M4 Max ceiling).
        // the GPU is all ours while mining, and it is production, not cost
        meter(&mut c[1], "gpu", (mhs / 80.0) as f32, (mhs / 80.0) as f32, &format!("{mhs:.0} MH/s"), MINT);
    });
    ui.add_space(12.0);
    ui.columns(2, |c| {
        meter(&mut c[0], "ram", miner_mem, mem, &format!("{:.0}%", mem * 100.0), AMBER);
        meter(
            &mut c[1],
            "net",
            0.0,
            (net_kbs / 2048.0) as f32,
            &format!("{net_kbs:.0} KB/s"),
            AMBER,
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
        if rej > 0 {
            card_row_tinted(ui, "shares", &format!("{acc}  ({rej} rejected)"), CORAL);
        } else {
            card_row(ui, "shares", &format!("{acc}"));
        }
    }
    {
        let h = p.height.load(O::Relaxed);
        card_row_tinted(ui, "block", &(if h > 0 { h.to_string() } else { "—".into() }), SKY);
    }
    card_row(ui, "hashed", &human(p.hashed.load(O::Relaxed)));
    if let Some(e) = eff_mhs {
        // The big number is the rate while mining; this one counts the
        // seconds spent rebuilding the table each block too.
        card_row(ui, "effective", &format!("{e:.1} MH/s"));
    }
}

/// THE PAYOUT — what the work returns: the game, the ledger, the balance.
pub fn payout_panel(
    ui: &mut egui::Ui,
    pi: &pool::PoolInfo,
    has_ledger: bool,
    solo: bool,
    mhs: f64,
    running: bool,
    // all_time: accepted shares and hashes across every run, ever.
    all_time: (u64, u64),
    // target_h: the height this panel must come out at. 0 = content decides.
    target_h: f32,
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

    // What you have earned across every run belongs with earnings, not with
    // the machine's vital signs.
    if all_time.0 > 0 || all_time.1 > 0 {
        ui.add_space(14.0);
        ui.separator();
        ui.add_space(9.0);
        caps(ui, "all time", 9.5, MUTE.gamma_multiply(0.85));
        ui.add_space(5.0);
        card_row(ui, "shares", &all_time.0.to_string());
        card_row(ui, "hashed", &human(all_time.1));
    }
    let _ = target_h;
}

/// The payout game — segmented bar, pulsing tip, the score and the honest
/// countdown, then the ledger rows. One source for both layouts.
pub fn payout_game(
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

/// ERG earned per day at the better of the pool-measured 24h rate and the
/// live local rate, against live network difficulty. Tail emission only
/// (3 ERG/block) — fees make earnings arrive sooner, never later. None
/// until the difficulty and a hashrate are both known.
pub fn erg_per_day(pi: &pool::PoolInfo, local_mhs: f64) -> Option<f64> {
    let rate_mhs = pi.hashrate_24h_mhs.max(local_mhs);
    if pi.difficulty <= 0.0 || rate_mhs <= 0.01 {
        return None;
    }
    let net_hs = pi.difficulty / pool::BLOCK_TIME_S;
    let blocks_per_day = 86_400.0 / pool::BLOCK_TIME_S;
    Some(rate_mhs * 1e6 / net_hs * blocks_per_day * pool::BLOCK_REWARD_ERG)
}

/// The honest countdown to the first payout.
pub fn payout_eta(pi: &pool::PoolInfo, local_mhs: f64, earned: f64) -> String {
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
