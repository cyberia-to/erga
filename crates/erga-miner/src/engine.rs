//! The pool-mining engine, driving progress through a shared `Progress` so
//! any front-end (CLI or GUI) can read it. Connect a pool, build the epoch
//! table on the GPU, scan, re-verify each candidate on the chain-verified
//! CPU reference, submit. Nothing invalid is ever sent.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::gpu::ScanMiner;
use aruminium::Gpu;
use erga_pool::stratum::{Job, PoolEvent, Stratum};

pub struct Progress {
    pub running: AtomicBool,
    pub stop: AtomicBool,
    pub rate_khs: AtomicU64, // MH/s × 1000
    pub accepted: AtomicU64,
    pub rejected: AtomicU64,
    pub height: AtomicU64,
    pub hashed: AtomicU64,
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

    if let Ok(g) = Gpu::open() {
        *p.device.lock().unwrap() = g.name();
    }

    p.set_status("connecting…");
    let mut s = match Stratum::connect(&cfg.host, cfg.port, &cfg.address, "erga") {
        Ok(s) => s,
        Err(e) => {
            p.set_status(format!("connect failed: {e}"));
            p.running.store(false, Ordering::Relaxed);
            return;
        }
    };

    let en1 = s.extranonce1.clone();
    let en1_bits = en1.len() as u32 * 8;
    let search_bits = 64 - en1_bits;
    let en1_val: u64 = en1.iter().fold(0u64, |a, &b| (a << 8) | b as u64);
    let en1_prefix = if en1_bits == 0 { 0 } else { en1_val << search_bits };
    let tail_mask: u64 = if search_bits >= 64 { u64::MAX } else { (1u64 << search_bits) - 1 };

    let m = autolykos::big_m();
    let mut miner: Option<ScanMiner> = None;
    let mut cur_height = 0u32;
    let mut job: Option<Job> = None;
    let mut cursor: u64 = 0;
    let batch: u32 = 8_388_608; // 8M nonces per dispatch
    let mut window_start = std::time::Instant::now();
    let mut window_hashed: u64 = 0;

    p.set_status("waiting for work…");
    while !p.stop.load(Ordering::Relaxed) {
        while let Ok(ev) = s.events.try_recv() {
            match ev {
                PoolEvent::Job(j) => {
                    if j.height != cur_height {
                        cur_height = j.height;
                        p.height.store(j.height as u64, Ordering::Relaxed);
                        let n = autolykos::calc_big_n(j.version, j.height);
                        p.set_status("building table…");
                        p.rate_khs.store(0, Ordering::Relaxed);
                        match ScanMiner::new_gpu_built(Gpu::open().expect("gpu"), n, j.height, &m) {
                            Ok(mn) => miner = Some(mn),
                            Err(e) => {
                                p.set_status(format!("GPU table build failed: {e}"));
                                p.running.store(false, Ordering::Relaxed);
                                return;
                            }
                        }
                        cursor = 0;
                        window_start = std::time::Instant::now();
                        window_hashed = 0;
                        p.set_status("mining");
                    }
                    job = Some(j);
                }
                PoolEvent::Difficulty(_) => {}
                PoolEvent::SubmitResult { accepted, .. } => {
                    if accepted {
                        p.accepted.fetch_add(1, Ordering::Relaxed);
                    } else {
                        p.rejected.fetch_add(1, Ordering::Relaxed);
                    }
                }
                PoolEvent::Closed => {
                    p.set_status("pool disconnected");
                    p.running.store(false, Ordering::Relaxed);
                    return;
                }
            }
        }

        let (Some(mn), Some(j)) = (&miner, &job) else {
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
            let hit = autolykos::pow_hit(&msg, &nb, &j.height.to_be_bytes(), mn.n, &m);
            if hit < j.target_b {
                let nonce_hex = hex(&nb);
                let en2_hex = hex(&nb[en1.len()..]);
                let _ = s.submit(&cfg.address, "erga", &j.job_id, &en2_hex, &nonce_hex, "");
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

    p.set_status("idle");
    p.rate_khs.store(0, Ordering::Relaxed);
    p.running.store(false, Ordering::Relaxed);
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
