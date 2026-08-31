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

/// The last `n` lines of the log — enough to see what just went wrong
/// without pasting a novel into an issue.
pub fn log_tail(n: usize) -> String {
    let Some(p) = log_path() else { return String::new() };
    let Ok(text) = std::fs::read_to_string(p) else { return String::new() };
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

fn shell(cmd: &str, args: &[&str]) -> String {
    std::process::Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Percent-encode for a URL query value. Small and explicit beats a
/// dependency for one string.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The issue body: the question, the machine, the live state, the log tail.
/// Separate from `report_bug` so it can be tested without opening a browser —
/// source indentation once leaked in here and turned the table into a code
/// block, which is invisible until someone files a real issue.
fn report_body(os: &str, chip: &str, mem_gb: &str, state: &str, log: &str) -> String {
    format!(
        "**What happened?**\n\n\n\
         **What did you expect?**\n\n\n\
         ---\n\n\
         | | |\n\
         |---|---|\n\
         | erga | {} |\n\
         | macOS | {os} |\n\
         | chip | {chip} |\n\
         | memory | {mem_gb} |\n\n\
         **State**\n\n```\n{state}\n```\n\n\
         **Recent log**\n\n```\n{log}\n```\n",
        env!("CARGO_PKG_VERSION"),
    )
}

/// Open a prefilled GitHub issue: the facts that are always asked for are
/// already in it, so a report costs one click instead of an interrogation.
/// GitHub cannot take a file attachment through a URL, so the log tail is
/// pasted inline — and the full log stays one Finder click away.
pub fn report_bug(state: &str) {
    let os = shell("sw_vers", &["-productVersion"]);
    let chip = shell("sysctl", &["-n", "machdep.cpu.brand_string"]);
    let mem_gb = shell("sysctl", &["-n", "hw.memsize"])
        .parse::<u64>()
        .map(|b| format!("{:.0} GB", b as f64 / (1u64 << 30) as f64))
        .unwrap_or_default();
    let body = report_body(&os, &chip, &mem_gb, state, &log_tail(40));
    // Keep well under what browsers and GitHub accept for a GET.
    let body: String = body.chars().take(5000).collect();
    let url = format!(
        "https://github.com/cyberia-to/erga/issues/new?title={}&body={}",
        encode(&format!("[{}] ", env!("CARGO_PKG_VERSION"))),
        encode(&body)
    );
    let _ = std::process::Command::new("open").arg(url).spawn();
    // GitHub takes no attachment through a URL, so the tail is inline above.
    // Reveal the full log too: attaching it is then one drag into the issue.
    reveal_log();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Markdown turns any line indented four spaces into a code block. Source
    /// indentation leaked into this template once and silently flattened the
    /// environment table in every issue filed.
    #[test]
    fn report_body_has_no_indented_lines() {
        let body = report_body("15.0", "Apple M4 Max", "48 GB", "mining", "a log line");
        for line in body.lines() {
            assert!(
                !line.starts_with("    "),
                "indented line would render as a code block: {line:?}"
            );
        }
    }

    /// The table must survive as a table: header, delimiter, then one row per
    /// fact, each starting at column zero.
    #[test]
    fn report_body_keeps_the_table() {
        let body = report_body("15.0", "Apple M4 Max", "48 GB", "mining", "log");
        assert!(body.contains("\n|---|---|\n"), "delimiter row missing");
        for row in ["| macOS | 15.0 |", "| chip | Apple M4 Max |", "| memory | 48 GB |"] {
            assert!(body.contains(&format!("\n{row}\n")), "row missing: {row}");
        }
    }
}
