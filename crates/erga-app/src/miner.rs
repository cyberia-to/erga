//! Thin GUI-side wrapper over the erga-miner pool engine. The engine runs on
//! its own thread and reports through the shared `Progress`; the UI reads it.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;

use erga_miner::engine::{self, PoolCfg, Progress};

const POOL_HOST: &str = "ergo.herominers.com";
const POOL_PORT: u16 = 1180;

pub struct Miner {
    pub p: Arc<Progress>,
    handle: Option<JoinHandle<()>>,
}

impl Miner {
    pub fn new() -> Self {
        Miner { p: Progress::new(), handle: None }
    }

    pub fn is_running(&self) -> bool {
        self.p.running.load(Ordering::Relaxed)
    }

    /// Start mining to the pool under `address`.
    pub fn start(&mut self, address: String) {
        if self.is_running() {
            return;
        }
        self.p.stop.store(false, Ordering::Relaxed);
        self.p.running.store(true, Ordering::Relaxed);
        let p = self.p.clone();
        let cfg = PoolCfg { host: POOL_HOST.into(), port: POOL_PORT, address };
        self.handle = Some(std::thread::spawn(move || engine::run(cfg, p)));
    }

    pub fn stop(&mut self) {
        self.p.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        self.p.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for Miner {
    fn drop(&mut self) {
        self.stop();
    }
}
