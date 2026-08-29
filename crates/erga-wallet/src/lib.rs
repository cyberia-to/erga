//! A minimal self-custodial Ergo wallet: generate a BIP39 mnemonic, derive
//! the mainnet P2PK address via ergo-lib (the reference implementation — no
//! hand-rolled crypto), and keep the seed phrase in the macOS Keychain.

use ergo_lib::ergotree_ir::chain::address::{Address, AddressEncoder, NetworkPrefix};
use ergo_lib::wallet::derivation_path::{ChildIndexHardened, ChildIndexNormal, DerivationPath};
use ergo_lib::wallet::ext_secret_key::ExtSecretKey;
use ergo_lib::wallet::mnemonic::Mnemonic as ErgoMnemonic;
use std::path::PathBuf;

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
    ///
    /// The seed lives in a `0600` file under Application Support. A macOS
    /// Keychain item would be encrypted at rest but prompts for the login
    /// password whenever the app's code signature changes — hostile to a
    /// one-click miner. The `back up your seed` flow makes the real backup
    /// the user's paper copy; this file is just so the app remembers itself.
    pub fn load_or_create() -> Result<Wallet, String> {
        if let Some(phrase) = seed_read()? {
            return Wallet::from_phrase(phrase.trim());
        }
        let w = Wallet::generate()?;
        seed_write(&w.mnemonic)?;
        Ok(w)
    }
}

fn seed_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|_| "no HOME".to_string())?;
    Ok(PathBuf::from(home).join("Library/Application Support/ai.cyber.erga/seed"))
}

fn seed_read() -> Result<Option<String>, String> {
    let p = seed_path()?;
    match std::fs::read_to_string(&p) {
        Ok(s) if !s.trim().is_empty() => Ok(Some(s)),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read seed: {e}")),
    }
}

fn seed_write(phrase: &str) -> Result<(), String> {
    let p = seed_path()?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir: {e}"))?;
    }
    std::fs::write(&p, phrase).map_err(|e| format!("write seed: {e}"))?;
    // owner-only permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
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
