//! API keys, kept in the OS keychain (macOS Keychain, Windows Credential
//! Manager, Secret Service on Linux). They are read only to make a request.

use std::collections::HashMap;
use std::sync::Mutex;

#[cfg(feature = "keychain")]
use crate::Error;
use crate::Result;

const SERVICE: &str = "app.nought.llm";

pub trait KeyStore: Send + Sync {
    fn set(&self, id: &str, key: &str) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<String>>;
    fn delete(&self, id: &str) -> Result<()>;
}

/// Keys held in memory only, lost on quit (tests, and machines without a keychain).
#[derive(Default)]
pub struct MemoryKeys(Mutex<HashMap<String, String>>);

impl KeyStore for MemoryKeys {
    fn set(&self, id: &str, key: &str) -> Result<()> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id.into(), key.into());
        Ok(())
    }
    fn get(&self, id: &str) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).get(id).cloned())
    }
    fn delete(&self, id: &str) -> Result<()> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(id);
        Ok(())
    }
}

#[cfg(feature = "keychain")]
pub struct Keychain;

#[cfg(feature = "keychain")]
impl Keychain {
    /// Whether the OS keychain can be used on this machine.
    pub fn available() -> bool {
        keyring::Entry::store_status().is_ok()
    }
}

#[cfg(feature = "keychain")]
fn kerr(e: keyring::Error) -> Error {
    Error::Keychain(e.to_string())
}

#[cfg(feature = "keychain")]
impl KeyStore for Keychain {
    fn set(&self, id: &str, key: &str) -> Result<()> {
        keyring::Entry::new(SERVICE, id)
            .map_err(kerr)?
            .set_password(key)
            .map_err(kerr)
    }
    fn get(&self, id: &str) -> Result<Option<String>> {
        match keyring::Entry::new(SERVICE, id).map_err(kerr)?.get_password() {
            Ok(k) => Ok(Some(k)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(kerr(e)),
        }
    }
    fn delete(&self, id: &str) -> Result<()> {
        match keyring::Entry::new(SERVICE, id).map_err(kerr)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(kerr(e)),
        }
    }
}

/// The best store on this machine: the OS keychain if it works, else memory.
pub fn default_store() -> (Box<dyn KeyStore>, bool) {
    #[cfg(feature = "keychain")]
    if Keychain::available() {
        return (Box::new(Keychain), true);
    }
    let _ = SERVICE;
    (Box::<MemoryKeys>::default(), false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store() {
        let s = MemoryKeys::default();
        assert_eq!(s.get("a").unwrap(), None);
        s.set("a", "sk-1").unwrap();
        assert_eq!(s.get("a").unwrap().as_deref(), Some("sk-1"));
        s.delete("a").unwrap();
        s.delete("a").unwrap();
        assert_eq!(s.get("a").unwrap(), None);
    }
}
