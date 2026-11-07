//! Stand-in for Noughtpad Sync, so the open app builds without the private
//! crate. Same shape as `noughtpad_sync::service` in pdmesh/noughtpad-sync;
//! keep the two in step. Everything here says "not in this build".

use std::path::PathBuf;
use std::sync::Arc;

use noughtpad_core::device::Device;
use noughtpad_core::sync::{Journal, SecretStore};
use noughtpad_core::{Error, Result, Vault};
use serde::Serialize;

/// This build doesn't include sync.
pub const AVAILABLE: bool = false;

pub struct Config {
    pub secrets: Arc<dyn SecretStore>,
    pub open_url: Arc<dyn Fn(&str) + Send + Sync>,
    pub on_status: Arc<dyn Fn(&Status) + Send + Sync>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum State {
    #[default]
    Off,
    Idle,
    Syncing,
    Error,
    SignedOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConflictNote {
    pub path: String,
    pub copy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub uploaded: usize,
    pub downloaded: usize,
    pub merged: usize,
    pub deleted_here: usize,
    pub deleted_there: usize,
    pub restored: Vec<String>,
    pub conflicts: Vec<ConflictNote>,
    pub too_big: Vec<String>,
    pub changed_here: bool,
    pub user_state_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub available: bool,
    pub configured: bool,
    pub state: State,
    pub account: Option<String>,
    pub signed_in: bool,
    pub notebook: Option<String>,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    pub last: Option<Summary>,
}

pub struct Service;

fn unavailable() -> Error {
    Error::Sync("this build doesn't include Noughtpad Sync".into())
}

impl Service {
    pub fn new(_cfg: Config) -> Arc<Service> {
        Arc::new(Service)
    }
    pub fn status(&self) -> Status {
        Status::default()
    }
    pub fn sign_in(&self) -> Result<String> {
        Err(unavailable())
    }
    /// Sign in through an app redirect (mobile): `authorize` shows Google's
    /// page and returns the URL it redirected to.
    pub fn sign_in_with(&self, _authorize: &dyn Fn(&str, &str) -> Result<String>) -> Result<String> {
        Err(unavailable())
    }
    pub fn sign_out(&self) -> Result<()> {
        Ok(())
    }
    pub fn start(
        self: &Arc<Self>,
        _vault: Arc<Vault>,
        _device: Device,
        _notebook: &str,
        _journal: Option<Arc<Journal>>,
    ) -> Result<()> {
        Err(unavailable())
    }
    pub fn stop(&self) {}
    pub fn nudge(&self) {}
    pub fn sync_now(&self) {}
}

pub fn journal_path(vault: &Vault) -> PathBuf {
    vault
        .root()
        .join(noughtpad_core::vault::DATA_DIR)
        .join(noughtpad_core::sync::SYNC_DIR)
        .join(noughtpad_core::sync::JOURNAL_FILE)
}
