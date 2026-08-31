//! Lightweight system telemetry for the dashboard row — CPU load, memory
//! use, and network throughput, all readable without elevated privileges.
//! GPU utilisation and power need entitlements macOS won't grant a plain
//! app, so the GPU's signal here is the hashrate itself, shown elsewhere.

use std::time::Instant;
use sysinfo::{Networks, Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// The miner's memory as macOS itself accounts it: `ri_phys_footprint`, the
/// same figure `footprint` and Activity Monitor report.
///
/// Resident size — what sysinfo reads — misses the one thing that matters
/// here: the epoch table lives in a Metal buffer, which the kernel books
/// under IOAccelerator, not under the process's resident pages. Measured on
/// a live miner: footprint 7105 MB, resident 30 MB. A meter fed the second
/// number says the miner costs nothing, which is exactly wrong.
fn phys_footprint(pid: u32) -> Option<u64> {
    let mut info: libc::rusage_info_v4 = unsafe { std::mem::zeroed() };
    let r = unsafe {
        libc::proc_pid_rusage(
            pid as libc::c_int,
            libc::RUSAGE_INFO_V4,
            &mut info as *mut _ as *mut libc::rusage_info_t,
        )
    };
    (r == 0).then_some(info.ri_phys_footprint)
}

pub struct Sys {
    sys: System,
    nets: Networks,
    last: Instant,
    pub cpu: f32,      // 0..1, whole machine
    pub mem: f32,      // 0..1, whole machine
    /// The miner's own share of each, so the meters can say what erga costs
    /// rather than what the machine happens to be doing.
    pub miner_cpu: f32,
    pub miner_mem: f32,
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
            miner_cpu: 0.0,
            miner_mem: 0.0,
            down_kbs: 0.0,
            up_kbs: 0.0,
        }
    }

    /// Refresh at most ~1×/second (self-throttled; cheap to call every frame).
    /// `miner_pid` is the child doing the mining, if it is running — reading a
    /// process we spawned ourselves needs no privileges.
    pub fn refresh(&mut self, miner_pid: Option<u32>) {
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

        // What the miner itself costs. CPU is reported per core by sysinfo,
        // so divide by the core count to land on the same 0..1 scale as the
        // machine-wide figure.
        self.miner_cpu = 0.0;
        self.miner_mem = 0.0;
        if let Some(pid) = miner_pid {
            let pid = Pid::from_u32(pid);
            self.sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );
            if let Some(proc) = self.sys.process(pid) {
                let cores = self.sys.cpus().len().max(1) as f32;
                self.miner_cpu = (proc.cpu_usage() / 100.0 / cores).clamp(0.0, 1.0);
                let total = self.sys.total_memory().max(1) as f32;
                // Footprint first — it is the only figure that sees the epoch
                // table. Resident size is the fallback, not the answer.
                let bytes = phys_footprint(miner_pid.unwrap_or(0)).unwrap_or(proc.memory());
                self.miner_mem = (bytes as f32 / total).clamp(0.0, 1.0);
            }
        }
        self.last = Instant::now();
    }
}
