//! The pool-mining engine, driving progress through a shared `Progress` so
//! any front-end (CLI or GUI) can read it. Connect a pool, build the epoch
//! table on the GPU, scan, re-verify each candidate on the chain-verified
//! CPU reference, submit. Nothing invalid is ever sent.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::gpu::ScanMiner;
use aruminium::Gpu;
use erga_pool::stratum::{Job, PoolEvent, Stratum};

// ─── development donation ────────────────────────────────────────────────
//
// One share in every `DONATION_EVERY_NTH` (20 → 5%) is submitted under the
// address below instead of yours. The pool credits whoever the submitted
// share names, so this is a real 5% of your mining, and nothing else about
// your mining changes — no extra connection, no hidden traffic, no idle
// time. It funds erga's development.
//
// You own this software and this choice. To change it, edit these two
// constants and rebuild — or set the environment variables at run time:
//
//   ERGA_DONATION=off              turn it off entirely
//   ERGA_DONATION=<your address>   send that 5% wherever you like
//   ERGA_DONATION_EVERY=50         donate 1 share in 50 (2%) instead
//
// The app always shows how many shares went to development, so the number
// is never hidden from you.
pub const DONATION_ADDRESS: &str = "9f8DEbXprAnTS4yhPjp9BEgqnzThVzBPggw5184RureWaRcoGYM";
pub const DONATION_EVERY_NTH: u64 = 20; // 20 → 5%; 0 disables the donation

/// The donation setting for this run, after the environment has its say.
/// Returns None when donation is off.
fn donation() -> Option<(String, u64)> {
    let addr = match std::env::var("ERGA_DONATION") {
        Ok(v) if v.eq_ignore_ascii_case("off") || v.is_empty() => return None,
        Ok(v) => v,
        Err(_) => DONATION_ADDRESS.to_string(),
    };
    let every = std::env::var("ERGA_DONATION_EVERY")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DONATION_EVERY_NTH);
    if every == 0 || addr.is_empty() {
        return None;
    }
    Some((addr, every))
}

/// Where the share cadence is remembered between runs.
fn cadence_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library/Application Support/ai.cyber.erga")
            .join("cadence"),
    )
}

/// Shares still owed to you before the next development share.
///
/// This must survive process restarts. The miner is spawned afresh every time
/// mining starts, so a counter that reset each run would mean a user who mines
/// in bursts shorter than `every` shares never donates at all — the stated 1
/// in 20 would quietly become 1 in never.
fn load_owed(every: u64) -> u64 {
    cadence_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(every - 1)
        .min(every - 1)
}

fn save_owed(owed: u64) {
    let Some(p) = cadence_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, owed.to_string());
}

pub struct Progress {
    pub running: AtomicBool,
    pub stop: AtomicBool,
    pub rate_khs: AtomicU64, // MH/s × 1000
    pub accepted: AtomicU64,
    pub rejected: AtomicU64,
    pub height: AtomicU64,
    pub hashed: AtomicU64,
    pub submitted: AtomicU64, // shares sent this run (drives the donation cadence)
    pub donated: AtomicU64,   // of those, how many funded development
    pub device: Mutex<String>,
    pub status: Mutex<String>,
}

impl Progress {
    pub fn new() -> Arc<Self> {
        Arc::new(Progress {
            running: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            rate_khs: AtomicU64::new(0),
            accepted: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            height: AtomicU64::new(0),
            hashed: AtomicU64::new(0),
            submitted: AtomicU64::new(0),
            donated: AtomicU64::new(0),
            device: Mutex::new(String::new()),
            status: Mutex::new("idle".into()),
        })
    }
    pub fn mhs(&self) -> f64 {
        self.rate_khs.load(Ordering::Relaxed) as f64 / 1000.0
    }
    pub fn set_status(&self, s: impl Into<String>) {
        *self.status.lock().unwrap() = s.into();
    }
}

pub struct PoolCfg {
    pub host: String,
    pub port: u16,
    pub address: String,
}

/// Run the miner until `p.stop` is set. Reports through `p`.
pub fn run(cfg: PoolCfg, p: Arc<Progress>) {
    p.running.store(true, Ordering::Relaxed);
    p.accepted.store(0, Ordering::Relaxed);
    p.rejected.store(0, Ordering::Relaxed);
    p.hashed.store(0, Ordering::Relaxed);
    p.submitted.store(0, Ordering::Relaxed);
    p.donated.store(0, Ordering::Relaxed);

    match Gpu::open() {
        Ok(g) => *p.device.lock().unwrap() = g.name(),
        Err(e) => {
            p.set_status(format!("no Metal GPU: {e:?}"));
            p.running.store(false, Ordering::Relaxed);
            return;
        }
    }

    let m = autolykos::big_m();
    let don = donation();

    // The epoch table lives out here so it survives session switches: it
    // depends only on the block height, never on which address we mine for.
    let mut table: Option<(u32, ScanMiner)> = None;

    // Shares owed to you before the next development share. The donation is
    // a *separate authorized session* — a pool binds each connection to the
    // address that authorized it, so a share can only be credited to the
    // session that found it. We therefore alternate sessions rather than
    // relabel shares: 19 for you, 1 for development, and so on.
    // Resumed from disk, so bursts shorter than `every` still add up.
    let mut owed_to_you: u64 = don.as_ref().map(|(_, every)| load_owed(*every)).unwrap_or(u64::MAX);
    let mut donating = don.is_some() && owed_to_you == 0;

    while !p.stop.load(Ordering::Relaxed) {
        let (addr, mut quota) = match (&don, donating) {
            (Some((a, _)), true) => (a.clone(), 1u64),
            _ => (cfg.address.clone(), owed_to_you.max(1)),
        };
        p.set_status(if donating { "connecting… (development share)" } else { "connecting…" });
        match Stratum::connect(&cfg.host, cfg.port, &addr, "erga") {
            Ok(s) => {
                let end = mine_session(s, &addr, &p, &m, &mut table, &mut quota, donating);
                match end {
                    SessionEnd::Stopped => break,
                    SessionEnd::QuotaMet => {
                        if donating {
                            donating = false;
                            owed_to_you = don.as_ref().map(|(_, e)| e - 1).unwrap_or(u64::MAX);
                        } else if don.is_some() {
                            donating = true;
                            owed_to_you = 0;
                        }
                        if don.is_some() {
                            save_owed(owed_to_you);
                        }
                        continue; // switch immediately, no backoff
                    }
                    SessionEnd::Closed => {
                        // keep the phase; remember what is still owed
                        if !donating {
                            owed_to_you = quota;
                            if don.is_some() {
                                save_owed(owed_to_you);
                            }
                        }
                        p.set_status("pool disconnected — reconnecting…");
                    }
                }
            }
            Err(e) => p.set_status(format!("connect failed ({e}) — retrying…")),
        }
        if p.stop.load(Ordering::Relaxed) {
            break;
        }
        p.rate_khs.store(0, Ordering::Relaxed);
        for _ in 0..30 {
            if p.stop.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    p.set_status("idle");
    p.rate_khs.store(0, Ordering::Relaxed);
    p.running.store(false, Ordering::Relaxed);
}

/// Why a mining session ended.
enum SessionEnd {
    /// Submitted everything asked of it — the caller switches phase.
    QuotaMet,
    /// The pool hung up; the caller reconnects.
    Closed,
    /// The user pressed stop.
    Stopped,
}

/// One connected mining session. Returns when the pool disconnects or `stop`
/// is set; the caller reconnects if appropriate.
fn mine_session(
    mut s: Stratum,
    address: &str,
    p: &Arc<Progress>,
    m: &[u8],
    table: &mut Option<(u32, ScanMiner)>,
    quota: &mut u64,
    donating: bool,
) -> SessionEnd {
    let en1 = s.extranonce1.clone();
    let en1_bits = en1.len() as u32 * 8;
    let search_bits = 64 - en1_bits;
    let en1_val: u64 = en1.iter().fold(0u64, |a, &b| (a << 8) | b as u64);
    let en1_prefix = if en1_bits == 0 { 0 } else { en1_val << search_bits };
    let tail_mask: u64 = if search_bits >= 64 { u64::MAX } else { (1u64 << search_bits) - 1 };

    let mut job: Option<Job> = None;
    let mut cursor: u64 = 0;
    let batch: u32 = 8_388_608; // 8M nonces per dispatch
    let mut window_start = std::time::Instant::now();
    let mut window_hashed: u64 = 0;

    p.set_status("waiting for work…");
    while !p.stop.load(Ordering::Relaxed) {
        // Drain all pending events, coalescing to the LATEST job. At startup
        // the pool sends a burst of heights; rebuilding the table for each is
        // wasteful (13s apiece) — we rebuild once, for the newest height.
        let mut latest_job: Option<Job> = None;
        while let Ok(ev) = s.events.try_recv() {
            match ev {
                PoolEvent::Job(j) => latest_job = Some(j),
                PoolEvent::Difficulty(_) => {}
                PoolEvent::SubmitResult { accepted, .. } => {
                    if accepted {
                        p.accepted.fetch_add(1, Ordering::Relaxed);
                    } else {
                        p.rejected.fetch_add(1, Ordering::Relaxed);
                    }
                }
                PoolEvent::Closed => return SessionEnd::Closed,
            }
        }
        if let Some(j) = latest_job {
            // Rebuild only when the height actually changed — a table built
            // by the previous session is still exactly right for this one.
            if table.as_ref().map(|(h, _)| *h) != Some(j.height) {
                p.height.store(j.height as u64, Ordering::Relaxed);
                let n = autolykos::calc_big_n(j.version, j.height);
                p.set_status("building table…");
                p.rate_khs.store(0, Ordering::Relaxed);
                let gpu = match Gpu::open() {
                    Ok(g) => g,
                    Err(e) => {
                        p.set_status(format!("GPU open failed: {e:?}"));
                        return SessionEnd::Closed;
                    }
                };
                // Free the previous epoch's table BEFORE allocating the next.
                // The table is ~N*32 bytes (6.8 GiB at height 1.86M and it
                // grows 5% every 51200 blocks), so holding both across a
                // rebuild doubles peak memory and pushes smaller Macs into
                // swap. Mining on a stale-height table would be invalid
                // anyway, so there is nothing to lose by dropping it first.
                *table = None;
                match ScanMiner::new_gpu_built(gpu, n, j.height, m) {
                    Ok(mn) => *table = Some((j.height, mn)),
                    Err(e) => {
                        // transient build error: drop this table, wait for next job
                        p.set_status(format!("table build retry: {e}"));
                        *table = None;
                        continue;
                    }
                }
                cursor = 0;
                window_start = std::time::Instant::now();
                window_hashed = 0;
            }
            p.height.store(j.height as u64, Ordering::Relaxed);
            p.set_status(if donating { "mining · development share" } else { "mining" });
            job = Some(j);
        }

        let (Some((_, mn)), Some(j)) = (&*table, &job) else {
            std::thread::sleep(std::time::Duration::from_millis(80));
            continue;
        };
        let mut msg = [0u8; 32];
        if j.msg.len() == 32 {
            msg.copy_from_slice(&j.msg);
        }
        let target = left_pad_32(&j.target_b.to_bytes_be());

        let nonce_base = en1_prefix | (cursor & tail_mask);
        if let Some(nonce) = mn.scan(&msg, &target, nonce_base, batch) {
            let nb = nonce.to_be_bytes();
            let hit = autolykos::pow_hit(&msg, &nb, &j.height.to_be_bytes(), mn.n, m);
            if hit < j.target_b {
                let nonce_hex = hex(&nb);
                let en2_hex = hex(&nb[en1.len()..]);
                // The pool credits the address this session authorized with,
                // so the share simply goes to whoever this session is for.
                let _ = s.submit(address, "erga", &j.job_id, &en2_hex, &nonce_hex, "");
                p.submitted.fetch_add(1, Ordering::Relaxed);
                if donating {
                    p.donated.fetch_add(1, Ordering::Relaxed);
                }
                *quota = quota.saturating_sub(1);
                // Checkpoint the cadence the moment it moves. The app can be
                // quit or killed between shares, and a forgotten count is a
                // burst that silently never donates.
                if !donating && *quota != u64::MAX {
                    save_owed(*quota);
                }
                if *quota == 0 {
                    // give the pool a breath to answer before we switch away
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    drain_results(&mut s, p);
                    return SessionEnd::QuotaMet;
                }
            }
        }
        cursor = cursor.wrapping_add(batch as u64);
        p.hashed.fetch_add(batch as u64, Ordering::Relaxed);
        window_hashed += batch as u64;

        let dt = window_start.elapsed().as_secs_f64();
        if dt > 0.5 {
            let mhs = window_hashed as f64 / dt / 1e6;
            p.rate_khs.store((mhs * 1000.0) as u64, Ordering::Relaxed);
            window_start = std::time::Instant::now();
            window_hashed = 0;
        }
    }
    SessionEnd::Stopped
}

/// Read whatever the pool has already said about our submissions, so an
/// accepted/rejected verdict is not lost when a session is switched away.
fn drain_results(s: &mut Stratum, p: &Arc<Progress>) {
    while let Ok(ev) = s.events.try_recv() {
        if let PoolEvent::SubmitResult { accepted, .. } = ev {
            if accepted {
                p.accepted.fetch_add(1, Ordering::Relaxed);
            } else {
                p.rejected.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

fn left_pad_32(b: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let n = b.len().min(32);
    out[32 - n..].copy_from_slice(&b[b.len() - n..]);
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
