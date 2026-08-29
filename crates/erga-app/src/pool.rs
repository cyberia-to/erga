//! Pool-side truth: what herominers has actually credited to our address.
//! The local hashrate says what we *do*; this says what we've *earned* —
//! pending rewards still maturing (72 confirmations), the credited balance
//! waiting for the payout threshold, and payouts already sent.
//!
//! Two GETs against the pool's public API, off-thread: the address ledger
//! and the pool stats (network difficulty + the payout threshold), which
//! together give an honest ETA to the first payout.

use std::sync::{Arc, Mutex};

const API_ADDR: &str = "https://ergo.herominers.com/api/stats_address";
const API_STATS: &str = "https://ergo.herominers.com/api/stats";
const NANO: f64 = 1e9;
/// Ergo tail emission (EIP-27): 3 ERG per block, steady for years — the
/// conservative base for the ETA (tx fees on top are ignored).
pub const BLOCK_REWARD_ERG: f64 = 3.0;
/// Ergo targets a block every 2 minutes.
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
    pub threshold_erg: f64,    // the pool's minimum payout
    pub difficulty: f64,       // network difficulty (0 = unknown)
    pub price_usd: f64,        // ERG spot price per the pool (0 = unknown)
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
            threshold_erg: 0.5, // herominers' floor, until the pool confirms
            difficulty: 0.0,
            price_usd: 0.0,
            error: None,
        }
    }
}

impl PoolState {
    /// Fire a background fetch for `address`. Result lands in `inner`.
    pub fn fetch(&self, address: String) {
        {
            let mut p = self.inner.lock().unwrap();
            p.querying = true;
            p.error = None;
        }
        let inner = self.inner.clone();
        std::thread::spawn(move || {
            let ledger = query_ledger(&address);
            let net = query_network(); // best-effort; ETA degrades gracefully
            let mut p = inner.lock().unwrap();
            p.querying = false;
            match ledger {
                Ok((balance, pending, paid, hashrate_24h)) => {
                    p.ok = true;
                    p.balance_erg = balance;
                    p.pending_erg = pending;
                    p.paid_erg = paid;
                    p.hashrate_24h_mhs = hashrate_24h;
                }
                Err(e) => p.error = Some(e),
            }
            if let Ok((difficulty, threshold, price)) = net {
                p.difficulty = difficulty;
                if threshold > 0.0 {
                    p.threshold_erg = threshold;
                }
                if price > 0.0 {
                    p.price_usd = price;
                }
            }
        });
    }
}

// big numbers arrive as strings; small ones as numbers — accept both
fn num(v: &serde_json::Value) -> f64 {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0.0)
}

/// (balance, pending, paid, hashrate_24h MH/s)
fn query_ledger(address: &str) -> Result<(f64, f64, f64, f64), String> {
    let url = format!("{API_ADDR}?address={}&longpoll=false", address.trim());
    let json = get_json(&url)?;

    let stats = json.get("stats").cloned().unwrap_or_default();
    let null = serde_json::Value::Null;
    let balance = num(stats.get("balance").unwrap_or(&null)) / NANO;
    let paid = num(stats.get("paid").unwrap_or(&null)) / NANO;
    let hashrate_24h = num(stats.get("hashrate_24h").unwrap_or(&null)) / 1e6;

    // pending = our slice of every block still maturing
    let pending = json
        .get("unconfirmed")
        .and_then(|v| v.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .map(|b| num(b.get("reward").unwrap_or(&null)))
                .sum::<f64>()
        })
        .unwrap_or(0.0)
        / NANO;

    Ok((balance, pending, paid, hashrate_24h))
}

/// (network difficulty, payout threshold ERG, ERG price USD)
fn query_network() -> Result<(f64, f64, f64), String> {
    let json = get_json(API_STATS)?;
    let null = serde_json::Value::Null;
    let difficulty = num(
        json.get("network")
            .and_then(|n| n.get("difficulty"))
            .unwrap_or(&null),
    );
    let threshold = num(
        json.get("config")
            .and_then(|c| c.get("minPaymentThreshold"))
            .unwrap_or(&null),
    ) / NANO;
    let price = num(
        json.get("pool")
            .and_then(|p| p.get("price"))
            .and_then(|p| p.get("usd"))
            .unwrap_or(&null),
    );
    Ok((difficulty, threshold, price))
}

fn get_json(url: &str) -> Result<serde_json::Value, String> {
    ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| format!("pool: {e}"))?
        .into_json()
        .map_err(|e| format!("pool decode: {e}"))
}
