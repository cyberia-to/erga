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

// ─── how hard to push ────────────────────────────────────────────────────

/// How much of the machine mining is allowed to take.
///
/// The GPU has no throttle of its own, so this is a duty cycle: dispatch a
/// batch, then stand aside for a proportional rest. Wall-clock share is the
/// honest unit — it is what the fans, the battery and every other app on the
/// machine actually feel, and the reported hashrate falls with it because the
/// hashes really are not being done.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Intensity {
    /// Everything the chip will give.
    Max,
    /// A quarter of it — the machine stays entirely usable.
    Eco,
    /// A tenth. Nominal mining: enough to keep an address funded, little
    /// enough to forget it is running.
    Min,
}

impl Intensity {
    /// The share of wall-clock the GPU may spend mining.
    fn duty(self) -> f64 {
        match self {
            Intensity::Max => 1.0,
            Intensity::Eco => 0.25,
            Intensity::Min => 0.10,
        }
    }

    /// Nonces per dispatch. A smaller batch at low duty spreads the same work
    /// over more, shorter interruptions, which is what "usable machine" means
    /// in practice; at full tilt the largest batch amortizes best.
    fn batch(self) -> u32 {
        let base = match self {
            Intensity::Max => 8_388_608,
            Intensity::Eco => 4_194_304,
            Intensity::Min => 2_097_152,
        };
        std::env::var("ERGA_BATCH")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| *v >= 65_536)
            .unwrap_or(base)
    }

    pub fn parse(s: &str) -> Intensity {
        match s.trim().to_ascii_lowercase().as_str() {
            "min" => Intensity::Min,
            "eco" => Intensity::Eco,
            _ => Intensity::Max,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Intensity::Max => "max",
            Intensity::Eco => "eco",
            Intensity::Min => "min",
        }
    }
}

/// Where the window leaves the chosen intensity for the miner to read.
fn intensity_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        std::path::PathBuf::from(home)
            .join("Library/Application Support/ai.cyber.erga")
            .join("intensity"),
    )
}

/// The intensity in force, re-read from disk at most twice a second.
///
/// Polling a four-byte file beats restarting the miner: a restart would throw
/// away the epoch table and charge another build for what should be a switch
/// you feel immediately.
fn current_intensity(last: &mut std::time::Instant, cached: &mut Intensity) -> Intensity {
    if last.elapsed().as_millis() >= 500 {
        *last = std::time::Instant::now();
        if let Some(s) = intensity_path().and_then(|p| std::fs::read_to_string(p).ok()) {
            *cached = Intensity::parse(&s);
        }
    }
    *cached
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

// ─── the next table, built while this one mines ─────────────────────────
//
// Mining is memory-bound and the table build is ALU-bound (the V7 diagnostic
// measured compute running 10× spare during scans), so the two share the GPU
// well: the build rides in the compute the scans cannot use, on its own
// queue, and the pause at each block edge disappears instead of shrinking.
//
// The price is a second table in memory while both exist. That is checked
// against what is *available right now*, every block, not against the machine's
// total at startup — another app may have taken 20 GB since the last block.

/// Sentinel for `Progress::next_pct`: no background build exists.
pub const NO_PREFETCH: u64 = 200;

/// A miner is Metal objects, which Apple documents as thread-safe to share;
/// only command encoders are not, and each thread here encodes on its own
/// queue. Rust cannot see that, hence the wrapper.
struct SendMiner(ScanMiner);
unsafe impl Send for SendMiner {}

/// The epoch tables a session mines with: the one in use, and the next one
/// forming in the background. They travel together because they are one
/// resource — the answer to "what does block H need".
#[derive(Default)]
struct Tables {
    current: Option<(u32, ScanMiner)>,
    next: Option<Prefetch>,
}

struct Prefetch {
    height: u32,
    rx: std::sync::mpsc::Receiver<Result<SendMiner, String>>,
    /// Set when the block arrived before the build finished: drop the pacing
    /// and finish flat out — someone is waiting now.
    urgent: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}

impl Drop for Prefetch {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Whether a background build may start, given what the machine has free.
///
/// The headroom scales with the intensity setting because both answer the
/// same question — how much of the machine mining may take. `min` never
/// prefetches: nominal mining hoards nothing.
fn prefetch_allowed(intensity: Intensity, available: u64, need: u64) -> bool {
    const GIB: u64 = 1 << 30;
    let headroom = match intensity {
        Intensity::Max => 3 * GIB,
        Intensity::Eco => 8 * GIB,
        Intensity::Min => return false,
    };
    available > need.saturating_add(headroom)
}

/// Bytes the system can hand out without swapping, asked fresh each time.
fn available_memory() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.available_memory()
}

/// Begin building `height`'s table in the background, if memory and the
/// intensity setting allow. The build is paced — one piece, then a rest — to
/// stay out of the scans' way; a block arriving early flips it to urgent.
fn start_prefetch(
    height: u32,
    version: u8,
    m: &[u8],
    p: &Arc<Progress>,
    intensity: Intensity,
) -> Option<Prefetch> {
    let n = autolykos::calc_big_n(version, height);
    let need = n as u64 * 32;
    if !prefetch_allowed(intensity, available_memory(), need) {
        return None;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let urgent = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(AtomicBool::new(false));
    let (u, c, pp, mm) = (urgent.clone(), cancel.clone(), p.clone(), m.to_vec());
    std::thread::spawn(move || {
        let result = Gpu::open()
            .map_err(|e| format!("{e:?}"))
            .and_then(|gpu| {
                ScanMiner::new_gpu_built(gpu, n, height, &mm, &|f| {
                    pp.next_pct.store((f * 100.0) as u64, Ordering::Relaxed);
                    if c.load(Ordering::Relaxed) {
                        return false;
                    }
                    // Pace: rest ~6 s between pieces so the whole build takes
                    // about a minute of a ~112 s average block, in slices so
                    // urgency is felt within 200 ms.
                    if f < 1.0 && !u.load(Ordering::Relaxed) {
                        for _ in 0..30 {
                            if u.load(Ordering::Relaxed) || c.load(Ordering::Relaxed) {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(200));
                        }
                    }
                    !c.load(Ordering::Relaxed)
                })
            })
            .map(SendMiner);
        if result.is_err() {
            pp.next_pct.store(NO_PREFETCH, Ordering::Relaxed);
        }
        let _ = tx.send(result);
    });
    Some(Prefetch { height, rx, urgent, cancel })
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
    /// How far the epoch table has been built, 0..100. Real progress: the
    /// build is dispatched in pieces so this can be reported rather than
    /// guessed from a stopwatch.
    pub build_pct: AtomicU64,
    /// The NEXT block's table, built in the background while this one mines.
    /// 0..=100 while it exists (100 = ready and waiting); 200 = none.
    pub next_pct: AtomicU64,
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
            build_pct: AtomicU64::new(0),
            next_pct: AtomicU64::new(NO_PREFETCH),
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
    let mut tables = Tables::default();

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
                let end = mine_session(s, &addr, &p, &m, &mut tables, &mut quota, donating);
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
    tables: &mut Tables,
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
    // Re-read at most twice a second, so moving the control is felt at once.
    let mut intensity = Intensity::parse(&std::env::var("ERGA_INTENSITY").unwrap_or_default());
    let mut intensity_checked = std::time::Instant::now();
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
            let (table, prefetch) = (&mut tables.current, &mut tables.next);
            if table.as_ref().map(|(h, _)| *h) != Some(j.height) {
                p.height.store(j.height as u64, Ordering::Relaxed);

                // The background build, if it was for this very height, is
                // the fast path: take it ready, or hurry it and wait.
                let mut swapped = false;
                if prefetch.as_ref().is_some_and(|pf| pf.height == j.height) {
                    let pf = prefetch.take().unwrap();
                    pf.urgent.store(true, Ordering::Relaxed);
                    p.set_status("building table…");
                    loop {
                        match pf.rx.try_recv() {
                            Ok(Ok(mn)) => {
                                *table = Some((j.height, mn.0));
                                swapped = true;
                                break;
                            }
                            Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                // the blocking battery shows the same figure
                                let f = p.next_pct.load(Ordering::Relaxed).min(100);
                                p.build_pct.store(f, Ordering::Relaxed);
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                        }
                    }
                } else {
                    // A build for some other height is only in the way.
                    *prefetch = None;
                }
                p.next_pct.store(NO_PREFETCH, Ordering::Relaxed);

                if !swapped {
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
                    // Free the previous epoch's table BEFORE allocating the
                    // next. The table is ~N*32 bytes (6.8 GiB at height 1.86M,
                    // growing 5% every 51200 blocks), so holding both across a
                    // rebuild doubles peak memory and pushes smaller Macs into
                    // swap. Mining on a stale-height table would be invalid
                    // anyway, so there is nothing to lose by dropping it first.
                    *table = None;
                    match ScanMiner::new_gpu_built(gpu, n, j.height, m, &|f| {
                        p.build_pct.store((f * 100.0) as u64, Ordering::Relaxed);
                        true
                    }) {
                        Ok(mn) => *table = Some((j.height, mn)),
                        Err(e) => {
                            // transient: drop this table, wait for the next job
                            p.set_status(format!("table build retry: {e}"));
                            *table = None;
                            continue;
                        }
                    }
                }

                // Mining is about to resume on j.height — begin the next
                // block's table in the spare compute, memory permitting.
                *prefetch =
                    start_prefetch(j.height + 1, j.version, m, p, intensity);

                cursor = 0;
                window_start = std::time::Instant::now();
                window_hashed = 0;
            }
            p.height.store(j.height as u64, Ordering::Relaxed);
            p.set_status(if donating { "mining · development share" } else { "mining" });
            job = Some(j);
        }

        let (Some((_, mn)), Some(j)) = (&tables.current, &job) else {
            std::thread::sleep(std::time::Duration::from_millis(80));
            continue;
        };
        let mut msg = [0u8; 32];
        if j.msg.len() == 32 {
            msg.copy_from_slice(&j.msg);
        }
        let target = left_pad_32(&j.target_b.to_bytes_be());

        intensity = current_intensity(&mut intensity_checked, &mut intensity);
        let batch = intensity.batch();
        let nonce_base = en1_prefix | (cursor & tail_mask);
        let dispatch_started = std::time::Instant::now();
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

        // Stand aside for the rest of the duty cycle. Measured against the
        // dispatch that just ran, so the share holds whatever the machine's
        // speed is: a slower chip rests less, not more.
        let duty = intensity.duty();
        if duty < 1.0 {
            let worked = dispatch_started.elapsed();
            let rest = worked.mul_f64((1.0 / duty) - 1.0);
            // A cap keeps one slow dispatch from parking the miner for a
            // minute; it re-measures on the next pass instead.
            std::thread::sleep(rest.min(std::time::Duration::from_secs(2)));
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The duty is the promise the control makes; the batch is how smoothly
    /// it is kept. Both must fall as the setting eases off, or "eco" and
    /// "min" would differ only in name.
    #[test]
    fn intensity_eases_off_monotonically() {
        let (max, eco, min) = (Intensity::Max, Intensity::Eco, Intensity::Min);
        assert!(max.duty() > eco.duty() && eco.duty() > min.duty());
        assert!(max.batch() >= eco.batch() && eco.batch() >= min.batch());
        assert_eq!(max.duty(), 1.0, "max must not throttle itself");
    }

    /// Anything unrecognised means full tilt: a typo in the file must not
    /// silently drop someone to a tenth of their hashrate.
    #[test]
    fn intensity_parses_and_round_trips() {
        for i in [Intensity::Max, Intensity::Eco, Intensity::Min] {
            assert_eq!(Intensity::parse(i.as_str()), i);
        }
        assert_eq!(Intensity::parse("  ECO \n"), Intensity::Eco);
        for junk in ["", "fast", "0", "maximum"] {
            assert_eq!(Intensity::parse(junk), Intensity::Max, "{junk:?}");
        }
    }
}
