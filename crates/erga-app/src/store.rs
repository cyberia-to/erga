//! What survives a restart: all-time counters, the log file, and the flags
//! that make the app remember what it has already told you.
//!
//! Everything lives beside the seed in the app-support directory, except
//! the log, which goes to ~/Library/Logs/erga where macOS users expect to
//! find logs (and where Console.app will show them).

use std::io::Write;
use std::path::PathBuf;

fn support_dir() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/ai.cyber.erga"))
}

pub fn log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Logs/erga/erga.log"))
}

/// Append one timestamped line to the log. Never panics, never blocks the
/// UI meaningfully — a miner that cannot write its log still mines.
pub fn log(line: &str) {
    let Some(p) = log_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // seconds since epoch is enough to correlate with a pool's timestamps
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(f, "{secs} {line}");
    }
}

/// Show the log in Finder.
pub fn reveal_log() {
    if let Some(p) = log_path() {
        let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
    }
}

/// Counters that outlive a single run, plus the one-time flags.
#[derive(Clone, Default)]
pub struct Store {
    pub accepted: u64,
    pub rejected: u64,
    pub donated: u64,
    pub hashed: u64,
    pub seen_intro: bool,
    pub solo: bool,
}

fn path() -> Option<PathBuf> {
    Some(support_dir()?.join("history.json"))
}

impl Store {
    pub fn load() -> Self {
        let Some(p) = path() else { return Self::default() };
        let Ok(text) = std::fs::read_to_string(p) else { return Self::default() };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            return Self::default();
        };
        let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
        let b = |k: &str| v.get(k).and_then(|x| x.as_bool()).unwrap_or(false);
        Store {
            accepted: n("accepted"),
            rejected: n("rejected"),
            donated: n("donated"),
            hashed: n("hashed"),
            seen_intro: b("seen_intro"),
            solo: b("solo"),
        }
    }

    /// Persist `self` plus the counters of the session in flight, so a crash
    /// costs at most the seconds since the last save — and never double
    /// counts, because the next launch starts from this total with an empty
    /// session.
    pub fn save_with(&self, s_acc: u64, s_rej: u64, s_don: u64, s_hashed: u64) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let v = serde_json::json!({
            "accepted": self.accepted + s_acc,
            "rejected": self.rejected + s_rej,
            "donated": self.donated + s_don,
            "hashed": self.hashed + s_hashed,
            "seen_intro": self.seen_intro,
            "solo": self.solo,
        });
        let _ = std::fs::write(p, v.to_string());
    }

    pub fn save(&self) {
        self.save_with(0, 0, 0, 0);
    }
}
