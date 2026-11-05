use std::collections::HashMap;
use std::sync::Mutex;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::{missing_default, set_in_login};

const SERVICE: &str = "app.nicedb.desktop";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "no default keyring: create one in Seahorse (GNOME Keyring) or KWallet, then try again"
    )]
    NoKeyring,
    #[error("keychain: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

fn other(e: impl std::fmt::Display) -> Error {
    Error::Other(e.to_string())
}

#[cfg(not(target_os = "linux"))]
fn missing_default(_: &keyring::Error) -> bool {
    false
}

#[cfg(not(target_os = "linux"))]
fn set_in_login(_: &str, _: &str, _: &str) -> Result<()> {
    Err(Error::NoKeyring)
}

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

pub struct Keychain;

impl Keychain {
    fn entry(key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, key).map_err(other)
    }
}

impl SecretStore for Keychain {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match Self::entry(key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) if missing_default(&e) => Ok(None),
            Err(e) => Err(other(e)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        match Self::entry(key)?.set_password(value) {
            Ok(()) => Ok(()),
            Err(e) if missing_default(&e) => set_in_login(SERVICE, key, value),
            Err(e) => Err(other(e)),
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) if missing_default(&e) => Ok(()),
            Err(e) => Err(other(e)),
        }
    }
}

#[derive(Default)]
pub struct Memory(Mutex<HashMap<String, String>>);

impl SecretStore for Memory {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| other("poisoned"))?
            .get(key)
            .cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| other("poisoned"))?
            .insert(key.into(), value.into());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.0.lock().map_err(|_| other("poisoned"))?.remove(key);
        Ok(())
    }
}
