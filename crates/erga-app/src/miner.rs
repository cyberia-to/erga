//! The mining engine, isolated on its own thread. All Metal objects
//! (Gpu, GpuMiner, RTable) live and die inside `run` — none cross the
//! thread boundary. The GUI reads progress through the atomics in `Shared`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use aruminium::Gpu;
use mine_bench::{gpu_mine_for, zero_acc32, GpuMiner, MineVariant};
use rtable_bench::RTable;

/// A fixed test header. The mining *rate* is independent of the header
/// bytes — the per-nonce work and memory-access pattern are identical
/// whatever `m` holds — so a benchmark header measures the true hashrate.
const M_HEADER: [u8; 32] = [0x5au8; 32];

/// Progress shared with the UI. Rate is stored in kH/s (hashes/1000) so a
/// single `AtomicU64` carries three decimal places of MH/s without floats.
pub struct Shared {
    pub running: AtomicBool,
    pub stop: AtomicBool,
    pub rate_khs: AtomicU64,
    pub total_nonces: AtomicU64,
    pub log2n: AtomicU64,
    pub device: Mutex<String>,
    pub status: Mutex<String>,
}

impl Shared {
    fn new() -> Arc<Self> {
        Arc::new(Shared {
            running: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            rate_khs: AtomicU64::new(0),
            total_nonces: AtomicU64::new(0),
            log2n: AtomicU64::new(0),
            device: Mutex::new(String::new()),
            status: Mutex::new("idle".into()),
        })
    }
    pub fn mhs(&self) -> f64 {
        self.rate_khs.load(Ordering::Relaxed) as f64 / 1000.0
    }
    fn set_status(&self, s: impl Into<String>) {
        *self.status.lock().unwrap() = s.into();
    }
}

pub struct Miner {
    pub shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
}

impl Miner {
    pub fn new() -> Self {
        Miner { shared: Shared::new(), handle: None }
    }

    pub fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Relaxed)
    }

    pub fn start(&mut self) {
        if self.is_running() {
            return;
        }
        let shared = self.shared.clone();
        shared.stop.store(false, Ordering::Relaxed);
        shared.running.store(true, Ordering::Relaxed);
        shared.rate_khs.store(0, Ordering::Relaxed);
        shared.total_nonces.store(0, Ordering::Relaxed);
        shared.set_status("opening GPU…");
        self.handle = Some(std::thread::spawn(move || run(shared)));
    }

    pub fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.shared.running.store(false, Ordering::Relaxed);
        self.shared.rate_khs.store(0, Ordering::Relaxed);
        self.shared.set_status("idle");
    }
}

impl Drop for Miner {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Owns every Metal object for the lifetime of one mining session.
fn run(shared: Arc<Shared>) {
    let gpu = match Gpu::open() {
        Ok(g) => g,
        Err(e) => {
            shared.set_status(format!("no Metal GPU: {e:?}"));
            shared.running.store(false, Ordering::Relaxed);
            return;
        }
    };
    *shared.device.lock().unwrap() = gpu.name();

    // Build the R-table. Try a mainnet-representative 2 GiB (2^26) first;
    // fall back on smaller machines so the button never dead-ends.
    shared.set_status("building table…");
    let mut table = None;
    let mut chosen_log2n = 0u32;
    for log2n in [26u32, 25, 24, 23] {
        let n = 1u64 << log2n;
        match RTable::open(&gpu, n) {
            Ok(t) => {
                let threads = std::thread::available_parallelism().map(|p| p.get()).unwrap_or(8);
                t.build_parallel(0, threads); // h=0 benchmark table
                table = Some(t);
                chosen_log2n = log2n;
                break;
            }
            Err(_) => continue,
        }
    }
    let table = match table {
        Some(t) => t,
        None => {
            shared.set_status("could not allocate the mining table");
            shared.running.store(false, Ordering::Relaxed);
            return;
        }
    };
    shared.log2n.store(chosen_log2n as u64, Ordering::Relaxed);

    let miner = match GpuMiner::open(gpu, MineVariant::V1Single) {
        Ok(m) => m,
        Err(e) => {
            shared.set_status(format!("kernel compile failed: {e:?}"));
            shared.running.store(false, Ordering::Relaxed);
            return;
        }
    };
    let acc_buf = match miner.gpu.buffer(32) {
        Ok(b) => b,
        Err(e) => {
            shared.set_status(format!("buffer alloc failed: {e:?}"));
            shared.running.store(false, Ordering::Relaxed);
            return;
        }
    };

    shared.set_status("mining");
    let batch: u32 = 4_194_304; // 4M nonces per GPU dispatch
    let tg: usize = 64;
    let window = 0.5_f64; // seconds per hashrate sample
    let mut nonce: u64 = 0;

    while !shared.stop.load(Ordering::Relaxed) {
        zero_acc32(&acc_buf);
        let t0 = std::time::Instant::now();
        let n = gpu_mine_for(&miner, &table, &acc_buf, &M_HEADER, nonce, window, batch, tg);
        let dt = t0.elapsed().as_secs_f64();
        nonce = nonce.wrapping_add(n);
        let mhs = (n as f64) / dt / 1e6;
        shared.rate_khs.store((mhs * 1000.0) as u64, Ordering::Relaxed);
        shared.total_nonces.fetch_add(n, Ordering::Relaxed);
    }

    shared.running.store(false, Ordering::Relaxed);
}
