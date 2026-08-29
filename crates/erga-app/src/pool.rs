//! Pool-side truth: what herominers has actually credited to our address.
//! The local hashrate says what we *do*; this says what we've *earned* —
//! pending rewards still maturing (72 confirmations), the credited balance
//! waiting for the payout threshold, and payouts already sent.
//!
//! One GET against the pool's public API, off-thread, same shape as the
//! explorer balance fetch.

use std::sync::{Arc, Mutex};

const API: &str = "https://ergo.herominers.com/api/stats_address";
const NANO: f64 = 1e9;
/// herominers pays out at 0.5 ERG (minPaymentThreshold, verified live).
pub const PAYOUT_ERG: f64 = 0.5;

#[derive(Clone, Default)]
pub struct PoolState {
    pub inner: Arc<Mutex<PoolInfo>>,
}

#[derive(Default)]
pub struct PoolInfo {
    pub querying: bool,
    pub ok: bool,
    pub balance_erg: f64,      // credited, unpaid — counts toward the threshold
    pub pending_erg: f64,      // block rewards still maturing
    pub paid_erg: f64,         // lifetime payouts
    pub hashrate_24h_mhs: f64, // our rate as the POOL measures it
    pub error: Option<String>,
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
            let result = query(&address);
            let mut p = inner.lock().unwrap();
            p.querying = false;
            match result {
                Ok(info) => {
                    p.ok = true;
                    p.balance_erg = info.0;
                    p.pending_erg = info.1;
                    p.paid_erg = info.2;
                    p.hashrate_24h_mhs = info.3;
                }
                Err(e) => p.error = Some(e),
            }
        });
    }
}

/// (balance, pending, paid, hashrate_24h MH/s)
fn query(address: &str) -> Result<(f64, f64, f64, f64), String> {
    let url = format!("{API}?address={}&longpoll=false", address.trim());
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| format!("pool: {e}"))?;
    let json: serde_json::Value = resp.into_json().map_err(|e| format!("pool decode: {e}"))?;

    let stats = json.get("stats").cloned().unwrap_or_default();
    // big numbers arrive as strings; small ones as numbers — accept both
    let num = |v: &serde_json::Value| -> f64 {
        v.as_f64()
            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            .unwrap_or(0.0)
    };

    let balance = num(stats.get("balance").unwrap_or(&serde_json::Value::Null)) / NANO;
    let paid = num(stats.get("paid").unwrap_or(&serde_json::Value::Null)) / NANO;
    let hashrate_24h = num(stats.get("hashrate_24h").unwrap_or(&serde_json::Value::Null)) / 1e6;

    // pending = our slice of every block still maturing
    let pending = json
        .get("unconfirmed")
        .and_then(|v| v.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .map(|b| num(b.get("reward").unwrap_or(&serde_json::Value::Null)))
                .sum::<f64>()
        })
        .unwrap_or(0.0)
        / NANO;

    Ok((balance, pending, paid, hashrate_24h))
}
