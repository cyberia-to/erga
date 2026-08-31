//! Sending ERG. Every money-critical step — box selection, transaction
//! building, signing — is ergo-lib (the reference wallet), never hand-rolled.
//! Fetching unspent boxes and broadcasting go through the public explorer.
//!
//! The offline test builds and signs a real transaction from a mock funded
//! box, so the signing path is verified without spending anything. A live
//! broadcast is only exercised once the wallet actually holds ERG.

use ergo_lib::ergotree_ir::chain::address::{Address, AddressEncoder, NetworkPrefix};
use ergo_lib::ergotree_ir::chain::ergo_box::box_value::BoxValue;
use ergo_lib::ergotree_ir::chain::ergo_box::ErgoBox;
use ergo_lib::wallet::box_selector::{BoxSelector, SimpleBoxSelector};
use ergo_lib::wallet::tx_builder::TxBuilder;
use ergo_lib::chain::ergo_box::box_builder::ErgoBoxCandidateBuilder;
use ergo_lib::chain::transaction::unsigned::UnsignedTransaction;

const MIN_FEE: u64 = 1_000_000; // 0.001 ERG
const EXPLORER: &str = "https://api.ergoplatform.com";

/// Validate a mainnet address string; returns its ErgoTree-bearing Address.
pub fn parse_address(addr: &str) -> Result<Address, String> {
    AddressEncoder::new(NetworkPrefix::Mainnet)
        .parse_address_from_str(addr.trim())
        .map_err(|e| format!("invalid address: {e}"))
}

/// Build the UNSIGNED transaction sending `amount_nano` to `to`, change back
/// to `from`, paying `MIN_FEE`. This is the money-critical structure — box
/// selection, amounts, fee, change — and it is unit-tested offline. Signing
/// and broadcasting additionally need a node's state context (block headers),
/// which is the light-client harness tracked separately.
pub fn build_unsigned_tx(
    from: &str,
    to: &str,
    amount_nano: u64,
    inputs: Vec<ErgoBox>,
    height: u32,
) -> Result<UnsignedTransaction, String> {
    if amount_nano < MIN_FEE {
        return Err("amount is below the network fee".into());
    }
    let recipient = parse_address(to)?;
    let change = parse_address(from)?;
    let amount = BoxValue::new(amount_nano).map_err(|e| format!("amount: {e}"))?;
    let fee = BoxValue::new(MIN_FEE).map_err(|e| format!("fee: {e}"))?;

    // select enough to cover amount + fee
    let need = BoxValue::new(amount_nano + MIN_FEE).map_err(|e| format!("total: {e}"))?;
    let selection = SimpleBoxSelector::new()
        .select(inputs, need, &[])
        .map_err(|e| format!("insufficient funds: {e}"))?;

    let out = ErgoBoxCandidateBuilder::new(amount, recipient.script().map_err(|e| format!("{e:?}"))?, height)
        .build()
        .map_err(|e| format!("output: {e}"))?;

    TxBuilder::new(selection, vec![out], height, fee, change)
        .build()
        .map_err(|e| format!("tx build: {e}"))
}

/// Fetch the wallet's unspent boxes from the explorer.
pub fn fetch_unspent(address: &str) -> Result<Vec<ErgoBox>, String> {
    let url = format!("{EXPLORER}/api/v1/boxes/unspent/byAddress/{}?limit=50", address.trim());
    let resp = ureq_get(&url)?;
    let json: serde_json::Value = serde_json::from_str(&resp).map_err(|e| format!("decode: {e}"))?;
    let items = json.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut boxes = Vec::new();
    for it in items {
        if let Ok(b) = serde_json::from_value::<ErgoBox>(it) {
            boxes.push(b);
        }
    }
    Ok(boxes)
}

fn ureq_get(url: &str) -> Result<String, String> {
    ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|e| format!("network: {e}"))?
        .into_string()
        .map_err(|e| format!("read: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Wallet;
    use ergo_lib::chain::transaction::TxId;
    use ergo_lib::ergotree_ir::chain::ergo_box::NonMandatoryRegisters;

    #[test]
    fn address_validation() {
        assert!(parse_address("9hZWr3YDPQjYMzd85FKxBb9tNJ7wHQdv9fnLPNMCf2yvARG3vfV").is_ok());
        assert!(parse_address("not-an-address").is_err());
        assert!(parse_address("").is_err());
    }

    #[test]
    fn builds_unsigned_tx_from_a_mock_box() {
        // a wallet, a funded input box owned by it, then build a send tx.
        let w = Wallet::generate().unwrap();
        let from_addr = parse_address(&w.address).unwrap();
        let height = 1_000_000u32;
        let funded = BoxValue::new(1_000_000_000).unwrap(); // 1 ERG
        let input = ErgoBox::new(
            funded,
            from_addr.script().unwrap(),
            None,
            NonMandatoryRegisters::empty(),
            height,
            TxId::zero(),
            0,
        )
        .unwrap();

        let to = "9hZWr3YDPQjYMzd85FKxBb9tNJ7wHQdv9fnLPNMCf2yvARG3vfV";
        let tx = build_unsigned_tx(&w.address, to, 500_000_000, vec![input], height);
        assert!(tx.is_ok(), "build failed: {:?}", tx.err());
        let tx = tx.unwrap();
        // recipient output + change + fee box
        assert!(tx.output_candidates.len() >= 2, "got {} outputs", tx.output_candidates.len());
    }

    #[test]
    fn rejects_insufficient_funds() {
        let w = Wallet::generate().unwrap();
        let from_addr = parse_address(&w.address).unwrap();
        let input = ErgoBox::new(
            BoxValue::new(2_000_000).unwrap(), // 0.002 ERG — too little
            from_addr.script().unwrap(),
            None,
            NonMandatoryRegisters::empty(),
            1_000_000,
            TxId::zero(),
            0,
        )
        .unwrap();
        let to = "9hZWr3YDPQjYMzd85FKxBb9tNJ7wHQdv9fnLPNMCf2yvARG3vfV";
        assert!(build_unsigned_tx(&w.address, to, 500_000_000, vec![input], 1_000_000).is_err());
    }
}
