//! Pool-side truth: what the pool you chose has actually credited to your
//! address. The local hashrate says what the machine *does*; this says what
//! it has *earned*.
//!
//! Every pool reports the same handful of facts under different names, and
//! k1pool reports them in ERG where the others use nanoERG — so each one
//! gets its own small adapter and they all fill the same struct.
//!
//! Network difficulty and the ERG price are properties of the chain, not of
//! a pool, so they are always read from one place regardless of where you
//! are mining.

use crate::pools::{self, Ledger};
use std::sync::{Arc, Mutex};

const NETWORK_STATS: &str = "https://ergo.herominers.com/api/stats";
const NANO: f64 = 1e9;
/// Ergo tail emission (EIP-27): 3 ERG a block, steady for years — the
/// conservative base for every projection.
pub const BLOCK_REWARD_ERG: f64 = 3.0;
/// Ergo targets a block every two minutes.
pub const BLOCK_TIME_S: f64 = 120.0;

#[derive(Clone, Default)]
pub struct PoolState {
    pub inner: Arc<Mutex<PoolInfo>>,
}

pub struct PoolInfo {
    pub querying: bool,
    pub ok: bool,
    pub balance_erg: f64,      // credited, unpaid — counts toward the threshold
    pub pending_erg: f64,      // block rewards still maturing
    pub paid_erg: f64,         // lifetime payouts
    pub hashrate_24h_mhs: f64, // our rate as the POOL measures it
    pub threshold_erg: f64,    // this pool's minimum payout
    pub difficulty: f64,       // network difficulty (0 = unknown)
    pub price_usd: f64,        // ERG spot (0 = unknown)
    pub error: Option<String>,
}

impl Default for PoolInfo {
    fn default() -> Self {
        PoolInfo {
            querying: false,
            ok: false,
            balance_erg: 0.0,
            pending_erg: 0.0,
            paid_erg: 0.0,
            hashrate_24h_mhs: 0.0,
            threshold_erg: 0.5,
            difficulty: 0.0,
            price_usd: 0.0,
            error: None,
        }
    }
}

impl PoolState {
    /// Fetch the ledger for `address` at pool `idx`, off-thread.
    pub fn fetch(&self, address: String, idx: usize) {
        {
            let mut p = self.inner.lock().unwrap();
            p.querying = true;
            p.error = None;
        }
        let inner = self.inner.clone();
        std::thread::spawn(move || {
            let pool = pools::get(idx);
            let ledger = match pool.ledger {
                Ledger::Herominers => herominers(&address),
                Ledger::TwoMiners => two_miners(&address),
                Ledger::K1Pool => k1pool(&address),
                Ledger::None => Err("this pool has no in-app ledger".into()),
            };
            let net = network(); // best-effort; projections degrade gracefully
            let mut p = inner.lock().unwrap();
            p.querying = false;
            match ledger {
                Ok(l) => {
                    p.ok = true;
                    p.balance_erg = l.balance;
                    p.pending_erg = l.pending;
                    p.paid_erg = l.paid;
                    p.hashrate_24h_mhs = l.hashrate_mhs;
                    p.threshold_erg =
                        if l.threshold > 0.0 { l.threshold } else { pool.payout_erg };
                }
                Err(e) => {
                    p.ok = false;
                    p.error = Some(e);
                    p.threshold_erg = pool.payout_erg;
                }
            }
            if let Ok((difficulty, price)) = net {
                p.difficulty = difficulty;
                if price > 0.0 {
                    p.price_usd = price;
                }
            }
        });
    }
}

struct LedgerRead {
    balance: f64,
    pending: f64,
    paid: f64,
    hashrate_mhs: f64,
    threshold: f64,
}

// big numbers arrive as strings, small ones as numbers — accept both
fn num(v: Option<&serde_json::Value>) -> f64 {
    v.and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .unwrap_or(0.0)
}

fn herominers(address: &str) -> Result<LedgerRead, String> {
    let url = format!(
        "https://ergo.herominers.com/api/stats_address?address={}&longpoll=false",
        address.trim()
    );
    let json = get_json(&url)?;
    let stats = json.get("stats").cloned().unwrap_or_default();
    // pending = our slice of every block still maturing
    let pending = json
        .get("unconfirmed")
        .and_then(|v| v.as_array())
        .map(|bs| bs.iter().map(|b| num(b.get("reward"))).sum::<f64>())
        .unwrap_or(0.0)
        / NANO;
    Ok(LedgerRead {
        balance: num(stats.get("balance")) / NANO,
        pending,
        paid: num(stats.get("paid")) / NANO,
        hashrate_mhs: num(stats.get("hashrate_24h")) / 1e6,
        threshold: 0.0, // taken from the pool's own config below
    })
}

fn two_miners(address: &str) -> Result<LedgerRead, String> {
    let url = format!("https://erg.2miners.com/api/accounts/{}", address.trim());
    let json = get_json(&url)?;
    let stats = json.get("stats").cloned().unwrap_or_default();
    Ok(LedgerRead {
        balance: num(stats.get("balance")) / NANO,
        pending: num(stats.get("immature")) / NANO,
        paid: 0.0, // the account API does not report a lifetime paid total
        hashrate_mhs: num(json.get("hashrate")) / 1e6,
        threshold: num(json.get("config").and_then(|c| c.get("minPayout"))) / NANO,
    })
}

fn k1pool(address: &str) -> Result<LedgerRead, String> {
    let url = format!("https://k1pool.com/api/miner/erg/{}", address.trim());
    let json = get_json(&url)?;
    let m = json.get("miner").cloned().unwrap_or_default();
    // k1pool answers in ERG, not nanoERG — its payoutThreshold reads 5 for a
    // pool that advertises a 5 ERG minimum. Dividing here would be a
    // billion-fold error, so nothing is scaled.
    Ok(LedgerRead {
        balance: num(m.get("pendingBalance")),
        pending: num(m.get("immatureBalance")),
        paid: num(m.get("paidBalance")),
        hashrate_mhs: num(m.get("dayHashrate")) / 1e6,
        threshold: num(m.get("payoutThreshold")),
    })
}

/// (network difficulty, ERG price USD) — chain facts, one source.
fn network() -> Result<(f64, f64), String> {
    let json = get_json(NETWORK_STATS)?;
    let difficulty = num(json.get("network").and_then(|n| n.get("difficulty")));
    let price = num(
        json.get("pool")
            .and_then(|p| p.get("price"))
            .and_then(|p| p.get("usd")),
    );
    Ok((difficulty, price))
}

fn get_json(url: &str) -> Result<serde_json::Value, String> {
    ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| format!("pool: {e}"))?
        .into_json()
        .map_err(|e| format!("pool decode: {e}"))
}
