//! Updates from GitHub Releases (`tauri-plugin-updater`).
//!
//! Nothing runs on its own: the user asks from Settings or the app menu, the UI calls
//! `update_check`, and only if they then choose "Install and restart" is anything downloaded.
//! The download is checked against the public key in `tauri.conf.json` before it replaces the app.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::CmdError;

/// The update found by the last check, kept so "Install" fetches exactly that one.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    current: String,
    /// `None` when this is the newest version.
    available: Option<AvailableUpdate>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    notes: Option<String>,
    /// `YYYY-MM-DD`.
    date: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

fn plain(e: tauri_plugin_updater::Error) -> CmdError {
    use tauri_plugin_updater::Error::*;
    let message = match &e {
        Reqwest(_) | Network(_) => "Couldn't reach GitHub. Check your connection and try again.".to_string(),
        ReleaseNotFound => "Couldn't find any published release yet.".to_string(),
        TargetNotFound(_) | TargetsNotFound(_) => "The newest release has no update for this computer.".to_string(),
        Minisign(_) | SignatureUtf8(_) | Base64(_) => {
            "The download didn't match Noughtpad's signature, so it wasn't installed.".to_string()
        }
        _ => e.to_string(),
    };
    tracing::warn!("update: {e}");
    CmdError {
        kind: "update",
        message,
    }
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub async fn update_check(app: AppHandle, pending: State<'_, PendingUpdate>) -> Result<UpdateCheck, CmdError> {
    let current = app.package_info().version.to_string();
    let update = app.updater().map_err(plain)?.check().await.map_err(plain)?;
    tracing::info!(current, latest = ?update.as_ref().map(|u| &u.version), "update check");
    let available = update.as_ref().map(|u| AvailableUpdate {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
        date: u.date.map(|d| d.date().to_string()),
    });
    *pending.0.lock().unwrap() = update;
    Ok(UpdateCheck { current, available })
}

/// Downloads, verifies and installs the update from the last check, then restarts into it.
/// Progress goes out as `update-progress` events.
#[tauri::command]
pub async fn update_install(app: AppHandle, pending: State<'_, PendingUpdate>) -> Result<(), CmdError> {
    let update = pending.0.lock().unwrap().take().ok_or(CmdError {
        kind: "update",
        message: "Check for updates first.".into(),
    })?;
    let mut downloaded = 0u64;
    let result = update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = app.emit("update-progress", Progress { downloaded, total });
            },
            || {},
        )
        .await;
    if let Err(e) = result {
        // Keep it so "Try again" works without another check.
        *pending.0.lock().unwrap() = Some(update);
        return Err(plain(e));
    }
    tracing::info!(version = update.version, "update installed; restarting");
    app.restart()
}
