//! GUI-side controller for the miner. The heavy GPU work runs in a *separate*
//! process — the `erga-miner` CLI — not in this one. That isolation is
//! deliberate: eframe holds an OpenGL (glow) context on the UI thread, while
//! the miner drives Metal through honeycrisp. Two graphics APIs sharing one
//! process is fragile; a bad interaction there aborts the whole app. Running
//! the miner as a child process gives it its own clean GPU context, and if it
//! ever dies the UI survives and simply reports it.
//!
//! We reuse `engine::Progress` purely as the shared read-model the UI already
//! knows how to render; here it is populated by parsing the child's stdout.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;

use erga_miner::engine::Progress;

pub struct Miner {
    pub p: Arc<Progress>,
    child: Option<Child>,
    reader: Option<JoinHandle<()>>,
}

impl Miner {
    pub fn new() -> Self {
        Miner { p: Progress::new(), child: None, reader: None }
    }

    pub fn is_running(&self) -> bool {
        self.p.running.load(Ordering::Relaxed)
    }

    /// Locate the bundled `erga-miner` binary. In a packaged .app it sits next
    /// to this executable in `Contents/MacOS`; in a dev build it is the sibling
    /// artifact in the same `target/<profile>` directory.
    fn miner_bin() -> std::path::PathBuf {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let cand = dir.join("erga-miner");
                if cand.exists() {
                    return cand;
                }
            }
        }
        // last resort: rely on PATH
        std::path::PathBuf::from("erga-miner")
    }

    /// Start mining to the chosen pool under `address`.
    pub fn start(&mut self, address: String, host: &str, port: u16) {
        if self.is_running() {
            return;
        }
        self.p.stop.store(false, Ordering::Relaxed);

        // reset counters the UI reads
        self.p.accepted.store(0, Ordering::Relaxed);
        self.p.rejected.store(0, Ordering::Relaxed);
        self.p.hashed.store(0, Ordering::Relaxed);
        self.p.rate_khs.store(0, Ordering::Relaxed);
        self.p.set_status("starting…");

        let bin = Self::miner_bin();
        let mut cmd = Command::new(&bin);
        cmd.arg("mine")
            .arg(host)
            .arg(port.to_string())
            .arg(&address)
            .arg("--machine")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.p.set_status(format!("cannot launch miner: {e}"));
                return;
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                self.p.set_status("miner produced no output");
                let _ = child.kill();
                return;
            }
        };

        // Keep the Mac awake while mining — a sleeping machine mines nothing,
        // and lost nights dominate every other optimisation. caffeinate ties
        // itself to the miner's pid (-w) and exits with it; the display may
        // still sleep, the machine may not (-i idle, -s system-on-AC).
        let _ = Command::new("caffeinate")
            .args(["-is", "-w", &child.id().to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        self.p.running.store(true, Ordering::Relaxed);
        let p = self.p.clone();
        self.reader = Some(std::thread::spawn(move || {
            let rdr = BufReader::new(stdout);
            for line in rdr.lines() {
                let Ok(line) = line else { break };
                parse_line(&p, &line);
            }
            // stdout closed → the child exited. If we did not ask it to stop,
            // it crashed or the pool dropped us for good; reflect that.
            if !p.stop.load(Ordering::Relaxed) {
                p.set_status("miner stopped — press START to retry");
                p.rate_khs.store(0, Ordering::Relaxed);
            }
            p.running.store(false, Ordering::Relaxed);
        }));
        self.child = Some(child);
    }

    pub fn stop(&mut self) {
        self.p.stop.store(true, Ordering::Relaxed);
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(h) = self.reader.take() {
            let _ = h.join();
        }
        self.p.rate_khs.store(0, Ordering::Relaxed);
        self.p.running.store(false, Ordering::Relaxed);
        self.p.set_status("idle");
    }
}

/// Parse one line of the child's `--machine` output into the shared progress.
///   `DEVICE <name…>`
///   `STAT <rate_khs> <height> <accepted> <rejected> <hashed> <donated> <status…>`
fn parse_line(p: &Arc<Progress>, line: &str) {
    let mut it = line.split_whitespace();
    match it.next() {
        Some("DEVICE") => {
            let name = line["DEVICE".len()..].trim().to_string();
            if !name.is_empty() {
                *p.device.lock().unwrap() = name;
            }
        }
        Some("STAT") => {
            let rate = it.next().and_then(|s| s.parse::<u64>().ok());
            let height = it.next().and_then(|s| s.parse::<u64>().ok());
            let acc = it.next().and_then(|s| s.parse::<u64>().ok());
            let rej = it.next().and_then(|s| s.parse::<u64>().ok());
            let hashed = it.next().and_then(|s| s.parse::<u64>().ok());
            let donated = it.next().and_then(|s| s.parse::<u64>().ok());
            if let (Some(rate), Some(height), Some(acc), Some(rej), Some(hashed), Some(donated)) =
                (rate, height, acc, rej, hashed, donated)
            {
                p.rate_khs.store(rate, Ordering::Relaxed);
                p.height.store(height, Ordering::Relaxed);
                p.accepted.store(acc, Ordering::Relaxed);
                p.rejected.store(rej, Ordering::Relaxed);
                p.hashed.store(hashed, Ordering::Relaxed);
                p.donated.store(donated, Ordering::Relaxed);
                // the rest of the line is the status text
                let status: String = it.collect::<Vec<_>>().join(" ");
                if !status.is_empty() {
                    p.set_status(status);
                }
            }
        }
        _ => {}
    }
}

impl Drop for Miner {
    fn drop(&mut self) {
        self.stop();
    }
}
