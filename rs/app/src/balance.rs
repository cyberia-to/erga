//! Lightweight, read-only balance check against the public Ergo explorer.
//! No node, no sync of the full chain — one HTTPS GET returns the confirmed
//! balance for an address. Runs off-thread so the UI never blocks.

use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct BalanceState {
    pub inner: Arc<Mutex<Balance>>,
}

#[derive(Default)]
pub struct Balance {
    pub querying: bool,
    pub erg: Option<f64>,
    pub error: Option<String>,
}

impl BalanceState {
    /// Fire a background fetch for `address`. Result lands in `inner`.
    pub fn fetch(&self, address: String) {
        {
            let mut b = self.inner.lock().unwrap();
            b.querying = true;
            b.error = None;
        }
        let inner = self.inner.clone();
        std::thread::spawn(move || {
            let result = query_confirmed(&address);
            let mut b = inner.lock().unwrap();
            b.querying = false;
            match result {
                Ok(nanoerg) => b.erg = Some(nanoerg as f64 / 1e9),
                Err(e) => b.error = Some(e),
            }
        });
    }
}

fn query_confirmed(address: &str) -> Result<u64, String> {
    let a = address.trim();
    if a.is_empty() {
        return Err("enter an address".into());
    }
    let url = format!(
        "https://api.ergoplatform.com/api/v1/addresses/{a}/balance/confirmed"
    );
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| format!("network: {e}"))?;
    let json: serde_json::Value = resp.into_json().map_err(|e| format!("decode: {e}"))?;
    json.get("nanoErgs")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| "unexpected response (is the address valid?)".into())
}
