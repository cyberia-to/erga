//! The pools erga will mine to, and how to read what each one owes you.
//!
//! A pool earns a place here only after this client has held a real
//! conversation with it — a job parsed, and where possible a share accepted.
//! Guessing at endpoints from a website is how you ship a menu entry that
//! silently mines nothing.
//!
//! Regions are deliberately absent. Every pool here resolves to its own
//! nearest server, and latency only ever costs a stale share; picking a
//! continent by hand is a decision without a payoff.

use std::path::PathBuf;

/// Which public API tells us what this pool owes an address. Every pool
/// reports the same five facts under different names and, worse, different
/// units — k1pool answers in ERG where the others answer in nanoERG.
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)] // K1Pool is wired and waiting — see the note in POOLS
pub enum Ledger {
    Herominers,
    TwoMiners,
    K1Pool,
    None,
}

pub struct Pool {
    pub label: &'static str,
    pub host: &'static str,
    pub port: u16,
    pub ledger: Ledger,
    /// The pool's minimum payout in ERG, for the times its API will not say.
    pub payout_erg: f64,
    /// How this pool does solo, if it does at all.
    pub solo: Option<Solo>,
}

/// Solo *at a pool* is not solo with your own node. The pool still builds the
/// block candidate, still runs the stratum, still takes its fee — only the
/// accounting changes: whoever solves the block keeps it instead of sharing
/// it. (Real solo means running an Ergo node and mining its own candidate;
/// erga does not do that yet.) Pools implement the switch differently, and
/// that difference is nobody's business but this module's.
#[derive(Clone, Copy)]
pub enum Solo {
    /// Routed by prefixing the address — herominers.
    Prefix(&'static str),
    /// Routed to a separate server — 2miners.
    Endpoint(&'static str, u16),
}

pub const POOLS: &[Pool] = &[
    Pool {
        label: "herominers",
        host: "ergo.herominers.com",
        port: 1180,
        ledger: Ledger::Herominers,
        payout_erg: 0.5,
        solo: Some(Solo::Prefix("solo:")),
    },
    Pool {
        label: "2miners",
        host: "erg.2miners.com",
        port: 8888,
        ledger: Ledger::TwoMiners,
        payout_erg: 1.0,
        solo: Some(Solo::Endpoint("solo-erg.2miners.com", 8888)),
    },
    // k1pool (eu.erg.k1pool.com:3746) is deliberately absent. It speaks the
    // dialect — it sends jobs this client parses — but after minutes of
    // mining its API still reported `workers: 0` and no shares, so nothing
    // we sent was ever credited. Its ledger adapter is written and waiting
    // in pool.rs; the entry goes back the day a share of ours lands there.
];

/// Whether this pool can show you a ledger inside the app.
pub fn has_ledger(idx: usize) -> bool {
    POOLS.get(idx).map(|p| p.ledger != Ledger::None).unwrap_or(false)
}

pub fn get(idx: usize) -> &'static Pool {
    POOLS.get(idx).unwrap_or(&POOLS[0])
}

/// Where to connect and what to call yourself, for this pool in this mode.
pub fn endpoint(idx: usize, solo: bool) -> (&'static str, u16, &'static str) {
    let p = get(idx);
    match (solo, p.solo) {
        (true, Some(Solo::Prefix(pre))) => (p.host, p.port, pre),
        (true, Some(Solo::Endpoint(h, port))) => (h, port, ""),
        _ => (p.host, p.port, ""),
    }
}

/// Can this pool mine solo at all?
pub fn has_solo(idx: usize) -> bool {
    get(idx).solo.is_some()
}

fn config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Application Support/ai.cyber.erga/pool"))
}

/// The chosen pool, by the label stored last. Falls back to the default when
/// the stored name no longer exists — the list is allowed to change.
pub fn load_choice() -> usize {
    let Some(p) = config_path() else { return 0 };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| {
            let want = s.trim().to_string();
            POOLS.iter().position(|p| p.label == want)
        })
        .unwrap_or(0)
}

pub fn save_choice(idx: usize) {
    let Some(p) = config_path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Some(pool) = POOLS.get(idx) {
        let _ = std::fs::write(p, pool.label);
    }
}
