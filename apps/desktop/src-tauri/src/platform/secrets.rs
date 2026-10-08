//! OS credential store (Windows Credential Manager / macOS Keychain / Secret Service).
//!
//! Holds the sync session token and the D1 API token (spec §5.2): never written to SQLite and
//! never synced.

use hatoba_core::platform::SecretStore;
use zeroize::Zeroizing;

const SERVICE: &str = "app.hatoba.desktop";

#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringStore;

fn store_error(e: keyring::Error) -> hatoba_core::Error {
    hatoba_core::Error::Io(std::io::Error::other(format!("credential store: {e}")))
}

fn entry(key: &str) -> hatoba_core::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, key).map_err(store_error)
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> hatoba_core::Result<Option<Zeroizing<String>>> {
        match entry(key)?.get_password() {
            Ok(v) => Ok(Some(Zeroizing::new(v))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(store_error(e)),
        }
    }

    fn set(&self, key: &str, value: &str) -> hatoba_core::Result<()> {
        entry(key)?.set_password(value).map_err(store_error)
    }

    fn delete(&self, key: &str) -> hatoba_core::Result<()> {
        match entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(store_error(e)),
        }
    }
}
