//! Where the Tangle access token lives: the macOS Keychain. Never logged.

use anyhow::Context;

const SERVICE: &str = "flowtag-tangle";
const ACCOUNT: &str = "pat";

pub trait TokenStore: Send {
    fn get(&self) -> Option<String>;
    fn set(&self, token: &str) -> anyhow::Result<()>;
    fn delete(&self) -> anyhow::Result<()>;
}

pub struct KeychainToken;

impl KeychainToken {
    fn entry() -> anyhow::Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, ACCOUNT).context("opening Keychain entry")
    }
}

impl TokenStore for KeychainToken {
    /// `None` when absent or when the Keychain cannot be read (the error is logged, never the value).
    fn get(&self) -> Option<String> {
        match Self::entry().and_then(|e| e.get_password().map_err(Into::into)) {
            Ok(token) => Some(token),
            Err(e) => {
                if !matches!(e.downcast_ref::<keyring::Error>(), Some(keyring::Error::NoEntry)) {
                    log::warn!("Keychain read failed: {e}");
                }
                None
            }
        }
    }

    fn set(&self, token: &str) -> anyhow::Result<()> {
        Self::entry()?.set_password(token).context("writing token to Keychain")
    }

    fn delete(&self) -> anyhow::Result<()> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e).context("deleting token from Keychain"),
        }
    }
}
