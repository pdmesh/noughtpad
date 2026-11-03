//! Window lifecycle, following each platform's conventions.
//!
//! - macOS: closing the window (red button or ⌘W) hides it and the app keeps
//!   running, so ⌘⇧Space capture is instant. Clicking the Dock icon brings the
//!   window back; ⌘Q quits.
//! - Windows and Linux: closing the window quits.
//!
//! "Open at login" starts the app with `--hidden`: hidden on macOS (the Dock
//! icon reopens it), minimised elsewhere so it stays reachable from the taskbar.

use tauri::{AppHandle, Manager, RunEvent, Window, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

use crate::CmdError;

pub const HIDDEN_ARG: &str = "--hidden";

pub fn autostart_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::Builder::new()
        .args([HIDDEN_ARG])
        .build()
}

/// Shows the main window at startup unless the app was launched at login.
/// The window is created hidden (`visible: false`) so a login launch never flashes it.
pub fn show_at_startup(app: &AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    if !std::env::args().any(|a| a == HIDDEN_ARG) {
        let _ = w.show();
        let _ = w.set_focus();
    } else if !cfg!(target_os = "macos") {
        let _ = w.show();
        let _ = w.minimize();
    }
}

/// Brings the main window to the front (Dock click, quick-capture shortcut).
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if cfg!(target_os = "macos") && window.label() == "main" {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window.hide();
        }
    }
}

pub fn on_run_event(app: &AppHandle, event: &RunEvent) {
    #[cfg(target_os = "macos")]
    if let RunEvent::Reopen {
        has_visible_windows: false,
        ..
    } = event
    {
        show_main(app);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, event);
}

#[tauri::command]
pub fn autostart_enabled(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, on: bool) -> Result<bool, CmdError> {
    let launcher = app.autolaunch();
    let res = if on { launcher.enable() } else { launcher.disable() };
    res.map_err(|e| CmdError {
        kind: "io",
        message: format!("could not change open at login: {e}"),
    })?;
    Ok(launcher.is_enabled().unwrap_or(on))
}
