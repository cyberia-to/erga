//! A minimal self-custodial Ergo wallet: generate a BIP39 mnemonic, derive
//! the mainnet P2PK address via ergo-lib (the reference implementation — no
//! hand-rolled crypto), and keep the seed phrase in the macOS Keychain.

use ergo_lib::ergotree_ir::chain::address::{Address, AddressEncoder, NetworkPrefix};
use ergo_lib::wallet::derivation_path::{ChildIndexHardened, ChildIndexNormal, DerivationPath};
use ergo_lib::wallet::ext_secret_key::ExtSecretKey;
use ergo_lib::wallet::mnemonic::Mnemonic as ErgoMnemonic;

const SERVICE: &str = "ai.cyber.erga";
const ACCOUNT: &str = "ergo-mnemonic";

/// A wallet: the mnemonic and the address derived from it.
pub struct Wallet {
    pub mnemonic: String,
    pub address: String,
}

impl Wallet {
    /// Create a fresh 15-word wallet.
    pub fn generate() -> Result<Wallet, String> {
        let m = bip39::Mnemonic::generate(15).map_err(|e| format!("mnemonic gen: {e}"))?;
        Wallet::from_phrase(&m.to_string())
    }

    /// Rebuild a wallet from an existing phrase.
    pub fn from_phrase(phrase: &str) -> Result<Wallet, String> {
        let address = derive_p2pk_address(phrase)?;
        Ok(Wallet { mnemonic: phrase.to_string(), address })
    }

    /// Load the stored wallet, or generate + store a new one on first run.
    pub fn load_or_create() -> Result<Wallet, String> {
        if let Some(phrase) = keychain_get()? {
            return Wallet::from_phrase(&phrase);
        }
        let w = Wallet::generate()?;
        keychain_set(&w.mnemonic)?;
        Ok(w)
    }

    pub fn forget() -> Result<(), String> {
        let e = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())?;
        let _ = e.delete_credential();
        Ok(())
    }
}

/// m/44'/429'/0'/0/0 → P2PK mainnet address (EIP-3).
fn derive_p2pk_address(phrase: &str) -> Result<String, String> {
    let seed = ErgoMnemonic::to_seed(phrase, "");
    let master = ExtSecretKey::derive_master(seed).map_err(|e| format!("master: {e:?}"))?;
    let path = DerivationPath::new(
        ChildIndexHardened::from_31_bit(0).map_err(|e| format!("{e:?}"))?,
        vec![
            ChildIndexNormal::normal(0).map_err(|e| format!("{e:?}"))?,
            ChildIndexNormal::normal(0).map_err(|e| format!("{e:?}"))?,
        ],
    );
    let key = master.derive(path).map_err(|e| format!("derive: {e:?}"))?;
    let ext_pub = key.public_key().map_err(|e| format!("pubkey: {e:?}"))?;
    let address: Address = ext_pub.into();
    Ok(AddressEncoder::new(NetworkPrefix::Mainnet).address_to_str(&address))
}

fn keychain_get() -> Result<Option<String>, String> {
    let e = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())?;
    match e.get_password() {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn keychain_set(phrase: &str) -> Result<(), String> {
    let e = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())?;
    e.set_password(phrase).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_address_is_mainnet_p2pk() {
        let w = Wallet::generate().unwrap();
        assert_eq!(w.mnemonic.split_whitespace().count(), 15);
        // mainnet P2PK addresses begin with '9'
        assert!(w.address.starts_with('9'), "got {}", w.address);
        // deterministic: same phrase → same address
        let again = Wallet::from_phrase(&w.mnemonic).unwrap();
        assert_eq!(w.address, again.address);
    }
}
