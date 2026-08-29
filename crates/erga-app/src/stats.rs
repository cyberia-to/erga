//! Lightweight system telemetry for the dashboard row — CPU load, memory
//! use, and network throughput, all readable without elevated privileges.
//! GPU utilisation and power need entitlements macOS won't grant a plain
//! app, so the GPU's signal here is the hashrate itself, shown elsewhere.

use std::time::Instant;
use sysinfo::{Networks, System};

pub struct Sys {
    sys: System,
    nets: Networks,
    last: Instant,
    pub cpu: f32,      // 0..1
    pub mem: f32,      // 0..1
    pub down_kbs: f64, // KiB/s
    pub up_kbs: f64,   // KiB/s
}

impl Sys {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Sys {
            sys,
            nets: Networks::new_with_refreshed_list(),
            last: Instant::now(),
            cpu: 0.0,
            mem: 0.0,
            down_kbs: 0.0,
            up_kbs: 0.0,
        }
    }

    /// Refresh at most ~1×/second (self-throttled; cheap to call every frame).
    pub fn refresh(&mut self) {
        let dt = self.last.elapsed().as_secs_f64();
        if dt < 1.0 {
            return;
        }
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.cpu = (self.sys.global_cpu_usage() / 100.0).clamp(0.0, 1.0);
        let total = self.sys.total_memory().max(1) as f32;
        self.mem = (self.sys.used_memory() as f32 / total).clamp(0.0, 1.0);

        self.nets.refresh(true);
        let (mut rx, mut tx) = (0u64, 0u64);
        for (_, d) in self.nets.iter() {
            rx += d.received();
            tx += d.transmitted();
        }
        self.down_kbs = rx as f64 / 1024.0 / dt;
        self.up_kbs = tx as f64 / 1024.0 / dt;
        self.last = Instant::now();
    }
}
