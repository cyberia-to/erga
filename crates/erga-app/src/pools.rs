//! The pool chooser. herominers is the default — lowest verified payout
//! floor (0.5 ERG against 1.0 at 2miners and woolypooly), a public per-
//! address ledger, and the stratum dialect this client is chain-verified
//! against. The regions are the same pool behind closer doors; latency
//! only affects stale shares, never the protocol.
//!
//! Foreign pools join this list only after a live accepted share through
//! our stratum client. 2miners passed (accepted share, 2026-08-29);
//! woolypooly's advertised endpoint does not resolve — excluded.

use std::path::PathBuf;

pub struct Pool {
    pub label: &'static str,
    pub host: &'static str,
    pub port: u16,
}

pub const POOLS: &[Pool] = &[
    Pool { label: "herominers · auto", host: "ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · germany", host: "de.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · finland", host: "fi.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · france", host: "fr.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · usa east", host: "us.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · usa west", host: "us2.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · canada", host: "ca.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · brazil", host: "br.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · turkey", host: "tr.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · india", host: "in.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · singapore", host: "sg.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · hong kong", host: "hk.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · korea", host: "kr.ergo.herominers.com", port: 1180 },
    Pool { label: "herominers · australia", host: "au.ergo.herominers.com", port: 1180 },
    Pool { label: "2miners · 1 erg min", host: "erg.2miners.com", port: 8888 },
];

/// Whether this pool's public ledger API is the herominers one the app reads.
pub fn has_ledger(idx: usize) -> bool {
    POOLS
        .get(idx)
        .map(|p| p.host.ends_with("herominers.com"))
        .unwrap_or(false)
}

fn config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library/Application Support/ai.cyber.erga/pool"),
    )
}

/// The chosen pool index, herominers·auto unless a valid choice is stored.
pub fn load_choice() -> usize {
    let Some(p) = config_path() else { return 0 };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| {
            let host = s.trim();
            POOLS.iter().position(|p| p.host == host)
        })
        .unwrap_or(0)
}

pub fn save_choice(idx: usize) {
    let Some(p) = config_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Some(pool) = POOLS.get(idx) {
        let _ = std::fs::write(p, pool.host);
    }
}
