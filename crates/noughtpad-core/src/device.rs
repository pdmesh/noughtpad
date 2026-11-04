//! This device's identity: a stable ULID and a name the user sees in sync
//! notices and conflict copies ("MacBook Air"). Kept in the app's data folder,
//! never inside a notebook, so copying a notebook to another machine doesn't
//! copy the identity with it.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::IoContext;
use crate::Result;

pub const DEVICE_FILE: &str = "device.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
}

impl Device {
    /// Read `dir/device.json`, creating it (named `default_name`) on first use.
    /// An unreadable file is replaced, since nothing in a notebook depends on
    /// the old id beyond this device's own sync state, which is then rebuilt.
    pub fn load_or_create(dir: &Path, default_name: impl FnOnce() -> String) -> Result<Device> {
        let path = dir.join(DEVICE_FILE);
        if let Some(d) = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Device>(&s).ok())
            .filter(|d| valid_id(&d.id))
        {
            return Ok(d);
        }
        let name = default_name();
        let device = Device {
            id: ulid::Ulid::new().to_string(),
            name: if name.trim().is_empty() {
                "This device".into()
            } else {
                name.trim().to_string()
            },
        };
        device.save(dir)?;
        Ok(device)
    }

    /// Rename this device (the id stays).
    pub fn rename(&mut self, dir: &Path, name: &str) -> Result<()> {
        if !name.trim().is_empty() {
            self.name = name.trim().to_string();
            self.save(dir)?;
        }
        Ok(())
    }

    fn save(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir).at(dir)?;
        let path = dir.join(DEVICE_FILE);
        let tmp = dir.join(format!("{DEVICE_FILE}.tmp"));
        fs::write(&tmp, serde_json::to_string_pretty(self).expect("device serialises")).at(&tmp)?;
        fs::rename(&tmp, &path).at(&path)?;
        Ok(())
    }
}

/// Ids name files (`user-state/<id>.jsonl`), so keep them to plain characters.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn created_once_then_reused() {
        let dir = tempfile::tempdir().unwrap();
        let a = Device::load_or_create(dir.path(), || "Mac".into()).unwrap();
        let b = Device::load_or_create(dir.path(), || unreachable!()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.name, "Mac");
        assert_eq!(a.id.len(), 26);
    }

    #[test]
    fn rename_keeps_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = Device::load_or_create(dir.path(), || "".into()).unwrap();
        assert_eq!(a.name, "This device");
        a.rename(dir.path(), " MacBook Air ").unwrap();
        let b = Device::load_or_create(dir.path(), || unreachable!()).unwrap();
        assert_eq!((b.id, b.name.as_str()), (a.id, "MacBook Air"));
    }

    #[test]
    fn a_damaged_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(DEVICE_FILE), "{\"id\":\"../x\",\"name\":\"n\"}").unwrap();
        let d = Device::load_or_create(dir.path(), || "Mac".into()).unwrap();
        assert!(valid_id(&d.id));
    }
}
