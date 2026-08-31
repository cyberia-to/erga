//! The headless mining front-end, shared by every binary that offers it.
//!
//! `engine::run` does the work on its own thread; this loop is only the
//! reporter. `--machine` emits parseable DEVICE/STAT lines for a GUI to
//! consume; without it, one human-readable line per change.

use crate::engine::{self, PoolCfg, Progress};
use std::io::Write;
use std::sync::atomic::Ordering;

/// Mine to `host:port` under `address` until the process is killed.
pub fn mine(host: String, port: u16, address: String, machine: bool) {
    let p = Progress::new();
    let pc = p.clone();
    let h = std::thread::spawn(move || engine::run(PoolCfg { host, port, address }, pc));

    let mut last = String::new();
    let mut device_sent = false;
    while !h.is_finished() {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let st = p.status.lock().unwrap().clone();
        let acc = p.accepted.load(Ordering::Relaxed);
        let rej = p.rejected.load(Ordering::Relaxed);
        if machine {
            if !device_sent {
                let dev = p.device.lock().unwrap().clone();
                if !dev.is_empty() {
                    println!("DEVICE {dev}");
                    device_sent = true;
                }
            }
            // status may contain spaces; keep it last, one field per token
            println!(
                "STAT {} {} {acc} {rej} {} {} {} {} {st}",
                p.rate_khs.load(Ordering::Relaxed),
                p.height.load(Ordering::Relaxed),
                p.hashed.load(Ordering::Relaxed),
                p.donated.load(Ordering::Relaxed),
                p.build_pct.load(Ordering::Relaxed),
                p.next_pct.load(Ordering::Relaxed),
            );
            let _ = std::io::stdout().flush();
        } else {
            // The build is the one phase long enough to wonder about, so the
            // terminal says how far along it is as well as the window.
            let pct = p.build_pct.load(Ordering::Relaxed);
            let st = if st.starts_with("building table") && pct < 100 {
                format!("{st} {pct}%")
            } else {
                st
            };
            let next = p.next_pct.load(Ordering::Relaxed);
            let st = if next <= 100 && st.starts_with("mining") {
                format!("{st} · next table {}", if next == 100 { "ready".into() } else { format!("{next}%") })
            } else {
                st
            };
            let line = format!(
                "{:>6.1} MH/s | height {} | accepted {acc} rejected {rej} | donated {} | {st}",
                p.mhs(),
                p.height.load(Ordering::Relaxed),
                p.donated.load(Ordering::Relaxed)
            );
            if line != last {
                println!("{line}");
                last = line;
            }
        }
    }
    let _ = h.join();
}
