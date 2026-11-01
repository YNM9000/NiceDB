use std::collections::HashMap;
use std::sync::Mutex;

const SERVICE: &str = "app.nicedb.desktop";

#[derive(Debug, thiserror::Error)]
#[error("keychain: {0}")]
pub struct Error(String);

pub type Result<T> = std::result::Result<T, Error>;

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

pub struct Keychain;

impl Keychain {
    fn entry(key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, key).map_err(|e| Error(e.to_string()))
    }
}

impl SecretStore for Keychain {
    fn get(&self, key: &str) -> Result<Option<String>> {
        match Self::entry(key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error(e.to_string())),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        Self::entry(key)?
            .set_password(value)
            .map_err(|e| Error(e.to_string()))
    }

    fn delete(&self, key: &str) -> Result<()> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error(e.to_string())),
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
            .map_err(|_| Error("poisoned".into()))?
            .get(key)
            .cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| Error("poisoned".into()))?
            .insert(key.into(), value.into());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        self.0
            .lock()
            .map_err(|_| Error("poisoned".into()))?
            .remove(key);
        Ok(())
    }
}
