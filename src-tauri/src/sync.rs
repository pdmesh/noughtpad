//! Noughtpad Sync in the app (DESIGN §12): Settings → Sync commands, the
//! Google sign-in kept in the OS keychain, and starting or stopping the
//! background sync when a notebook opens.
//!
//! The engine lives in the private `noughtpad-sync` crate. Open builds link
//! the stand-in in `crates/noughtpad-sync-stub`, which reports "not in this
//! build"; the UI then hides the Sync section.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use noughtpad_core::sync::SecretStore;
use noughtpad_llm::keys::KeyStore;
use noughtpad_sync::{Config, Service, Status};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Window, WindowEvent};

use crate::{load_config, save_config, AppState, CmdResult};

pub(crate) struct SyncState(pub Arc<Service>);

/// Saved in `config.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SyncSettings {
    pub enabled: bool,
    /// The notebook folder's name in Drive (`Noughtpad/<name>`); defaults to
    /// the notes folder's name.
    pub notebook: Option<String>,
}

/// The sign-in goes in the same keychain as provider keys.
struct Keys(Box<dyn KeyStore>);

impl SecretStore for Keys {
    fn get(&self, key: &str) -> noughtpad_core::Result<Option<String>> {
        self.0.get(key).map_err(|e| noughtpad_core::Error::Sync(e.to_string()))
    }
    fn set(&self, key: &str, value: &str) -> noughtpad_core::Result<()> {
        self.0
            .set(key, value)
            .map_err(|e| noughtpad_core::Error::Sync(e.to_string()))
    }
    fn delete(&self, key: &str) -> noughtpad_core::Result<()> {
        self.0
            .delete(key)
            .map_err(|e| noughtpad_core::Error::Sync(e.to_string()))
    }
}

pub(crate) fn init(app: &AppHandle) -> SyncState {
    let (keys, _) = noughtpad_llm::keys::default_store();
    let handle = app.clone();
    let seen = Arc::new(AtomicI64::new(0));
    SyncState(Service::new(Config {
        secrets: Arc::new(Keys(keys)),
        open_url: Arc::new(|url| {
            if let Err(e) = tauri_plugin_opener::open_url(url, None::<&str>) {
                tracing::warn!("could not open the browser for sign-in: {e}");
            }
        }),
        on_status: Arc::new(move |s| on_status(&handle, s, &seen)),
    }))
}

fn on_status(app: &AppHandle, s: &Status, seen: &AtomicI64) {
    let _ = app.emit("sync-status", s);
    // Act on each finished sync once.
    let Some(at) = s.last_sync else { return };
    if seen.swap(at, Ordering::SeqCst) == at {
        return;
    }
    let Some(sum) = &s.last else { return };
    let state = app.state::<AppState>();
    if sum.user_state_changed {
        if let Some(user) = state.user.read().unwrap_or_else(|e| e.into_inner()).clone() {
            if let Err(e) = user.reload() {
                tracing::warn!("could not reload user-state after sync: {e}");
            }
        }
    }
    if sum.changed_here {
        if let Ok(sem) = state.semantic() {
            sem.nudge();
        }
        let _ = app.emit("vault-changed", Vec::<noughtpad_core::vault::Change>::new());
    }
}

/// Start or stop syncing the open notebook, following the settings.
pub(crate) fn follow_settings(
    app: &AppHandle,
    vault: &Arc<noughtpad_core::Vault>,
    journal: Option<Arc<noughtpad_core::sync::Journal>>,
) {
    let svc = app.state::<SyncState>().0.clone();
    let cfg = load_config(app).sync;
    if !(cfg.enabled && noughtpad_sync::AVAILABLE) {
        svc.stop();
        return;
    }
    let device = match crate::this_device(app) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("sync not started: {}", e.message);
            return;
        }
    };
    let name = cfg.notebook.unwrap_or_else(|| notebook_name(vault));
    if let Err(e) = svc.start(vault.clone(), device, &name, journal) {
        tracing::warn!("sync not started: {e}");
    }
}

/// The Drive folder name for a notes folder: its own name.
fn notebook_name(vault: &noughtpad_core::Vault) -> String {
    vault
        .root()
        .file_name()
        .map(|n| n.to_string_lossy().trim().to_string())
        .filter(|n| !n.is_empty() && !n.starts_with('.'))
        .unwrap_or_else(|| "Notes".into())
}

/// Whether the open notebook should keep a change journal (sync is on).
pub(crate) fn wants_journal(app: &AppHandle) -> bool {
    noughtpad_sync::AVAILABLE && load_config(app).sync.enabled
}

pub(crate) fn on_window_event(window: &Window, event: &WindowEvent) {
    if let WindowEvent::Focused(true) = event {
        if let Some(s) = window.app_handle().try_state::<SyncState>() {
            s.0.nudge();
        }
    }
}

/// Status plus what the Settings page needs to show before sync is on.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncInfo {
    #[serde(flatten)]
    status: Status,
    enabled: bool,
    /// The Drive folder this notebook syncs to (or would).
    notebook_name: Option<String>,
}

#[tauri::command]
pub(crate) fn sync_status(app: AppHandle, state: State<'_, AppState>, sync: State<'_, SyncState>) -> SyncInfo {
    let cfg = load_config(&app).sync;
    let notebook_name = cfg
        .notebook
        .clone()
        .or_else(|| state.vault().ok().map(|v| notebook_name(&v)));
    SyncInfo {
        status: sync.0.status(),
        enabled: cfg.enabled,
        notebook_name,
    }
}

#[tauri::command]
pub(crate) async fn sync_sign_in(sync: State<'_, SyncState>) -> CmdResult<String> {
    let svc = sync.0.clone();
    tauri::async_runtime::spawn_blocking(move || svc.sign_in())
        .await
        .map_err(|e| crate::CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
pub(crate) fn sync_sign_out(app: AppHandle, sync: State<'_, SyncState>) -> CmdResult<()> {
    sync.0.sign_out()?;
    let mut cfg = load_config(&app);
    cfg.sync.enabled = false;
    save_config(&app, &cfg);
    Ok(())
}

/// Turn syncing this notebook on or off.
#[tauri::command]
pub(crate) fn sync_set_enabled(app: AppHandle, state: State<'_, AppState>, on: bool) -> CmdResult<()> {
    let mut cfg = load_config(&app);
    cfg.sync.enabled = on;
    save_config(&app, &cfg);
    // The watcher records changes for sync only while it's on.
    let vault = state.vault()?;
    crate::start_watching(&app, &state, &vault)?;
    Ok(())
}

#[tauri::command]
pub(crate) fn sync_now(sync: State<'_, SyncState>) {
    sync.0.sync_now();
}
