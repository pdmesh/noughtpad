//! Tauri shell: exposes `noughtpad-core` to the UI as commands and forwards
//! filesystem changes as events.

mod chat;
#[cfg(desktop)]
mod lifecycle;
#[cfg(target_os = "macos")]
mod menu;
mod semantic;
mod sync;
mod think;
#[cfg(desktop)]
mod updater;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use noughtpad_core::device::Device;
use noughtpad_core::index::{DaySummary, LabelCount, ListQuery, MonthLog, NoteSummary, SearchHit};
use noughtpad_core::semantic::Semantic;
use noughtpad_core::userstate::UserState;
use noughtpad_core::vault::{Change, NewNote, NoteDoc, NoteUpdate, ScanReport};
use noughtpad_core::watch::{watch_journaled, VaultWatcher};
use noughtpad_core::Vault;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
pub(crate) struct AppState {
    vault: RwLock<Option<Arc<Vault>>>,
    watcher: Mutex<Option<VaultWatcher>>,
    semantic: RwLock<Option<Arc<Semantic>>>,
    /// The open notebook's user-state, reloaded when sync brings in another device's.
    pub(crate) user: RwLock<Option<Arc<UserState>>>,
}

impl AppState {
    pub(crate) fn vault(&self) -> Result<Arc<Vault>, CmdError> {
        self.vault
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| CmdError {
                kind: "noVault",
                message: "No vault is open".into(),
            })
    }

    pub(crate) fn semantic(&self) -> Result<Arc<Semantic>, CmdError> {
        self.semantic
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or_else(|| CmdError {
                kind: "noVault",
                message: "No vault is open".into(),
            })
    }
}

/// Errors as the UI sees them: a machine-readable kind plus a message.
#[derive(Debug, Serialize)]
pub(crate) struct CmdError {
    kind: &'static str,
    message: String,
}

impl From<noughtpad_core::Error> for CmdError {
    fn from(e: noughtpad_core::Error) -> Self {
        use noughtpad_core::Error::*;
        let kind = match &e {
            Conflict(_) => "conflict",
            NotFound(_) => "notFound",
            Io { .. } => "io",
            _ => "internal",
        };
        CmdError {
            kind,
            message: e.to_string(),
        }
    }
}

pub(crate) type CmdResult<T> = Result<T, CmdError>;

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Config {
    vault_path: Option<PathBuf>,
    /// Local semantic search: `None` = never asked, so it stays off.
    semantic: Option<bool>,
    /// Language models the user opted into (M4). Keys live in the keychain.
    #[serde(default)]
    llm: noughtpad_llm::settings::LlmSettings,
    /// Noughtpad Sync (builds that include it).
    #[serde(default)]
    sync: sync::SyncSettings,
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("config.json"))
}

pub(crate) fn load_config(app: &AppHandle) -> Config {
    config_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub(crate) fn save_config(app: &AppHandle, cfg: &Config) {
    if let Some(p) = config_path(app) {
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        if let Err(e) = std::fs::write(&p, serde_json::to_string_pretty(cfg).unwrap()) {
            tracing::warn!("could not save config: {e}");
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultInfo {
    path: String,
    note_count: i64,
    default_path: String,
    /// Whether the UI should offer to switch notebooks. Desktop only: mobile
    /// has one fixed location (see `default_vault_path`) and no folder picker.
    can_switch_vault: bool,
}

/// Where notes live when nothing else says otherwise.
///
/// Desktop: `~/Documents/Noughtpad` (or the home folder if there's no
/// Documents directory), chosen once and then replaceable via the folder
/// picker. Mobile has no folder picker and no `NOUGHTPAD_VAULT` override
/// (both are desktop ideas, see docs/MOBILE.md): it always uses the app's own
/// documents directory, which Tauri's path resolver — unlike the `dirs` crate
/// `default_vault_path` used before mobile existed — knows how to place on
/// iOS (`NSDocumentDirectory`, visible in the Files app once the Xcode
/// project turns on file sharing) and Android (the app's external files dir).
fn default_vault_path(app: &AppHandle) -> PathBuf {
    app.path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("Noughtpad")
}

/// This device's id and name (`device.json` in the app data folder), created
/// on first use. The id names this device's user-state log in every notebook.
pub(crate) fn this_device(app: &AppHandle) -> CmdResult<Device> {
    static DEVICE: std::sync::OnceLock<Device> = std::sync::OnceLock::new();
    if let Some(d) = DEVICE.get() {
        return Ok(d.clone());
    }
    let dir = app.path().app_data_dir().map_err(|e| CmdError {
        kind: "io",
        message: e.to_string(),
    })?;
    let d = Device::load_or_create(&dir, computer_name)?;
    Ok(DEVICE.get_or_init(|| d).clone())
}

/// The name the OS gives this computer ("MacBook Air"), for the device name.
fn computer_name() -> String {
    let run = |cmd: &str, args: &[&str]| {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let name = if cfg!(target_os = "macos") {
        run("scutil", &["--get", "ComputerName"])
    } else {
        std::env::var("COMPUTERNAME").ok().or_else(|| run("hostname", &[]))
    };
    name.unwrap_or_default()
}

/// (Re)start the watcher for the open notebook, with a change journal while
/// sync is on, and start or stop sync to match the settings.
pub(crate) fn start_watching(app: &AppHandle, state: &AppState, vault: &Arc<Vault>) -> CmdResult<()> {
    // Drop the old watcher first, so its journal is closed before a new one opens.
    state.watcher.lock().unwrap_or_else(|e| e.into_inner()).take();
    let journal = if sync::wants_journal(app) {
        match noughtpad_core::sync::Journal::open(&noughtpad_sync::journal_path(vault)) {
            Ok(j) => Some(Arc::new(j)),
            Err(e) => {
                tracing::warn!("no change journal for sync: {e}");
                None
            }
        }
    } else {
        None
    };
    let sem = state.semantic()?;
    let svc = app.state::<sync::SyncState>().0.clone();
    let handle = app.clone();
    let watcher = watch_journaled(vault.clone(), journal.clone(), move |changes: Vec<Change>| {
        sem.nudge();
        svc.nudge();
        let _ = handle.emit("vault-changed", changes);
    })?;
    *state.watcher.lock().unwrap_or_else(|e| e.into_inner()) = Some(watcher);
    sync::follow_settings(app, vault, journal);
    Ok(())
}

/// The steps of opening a notebook: each is shown under the startup loader
/// (`startup-step` event) and logged with how long the one before took, so a
/// slow start says where the time went.
struct Steps<'a> {
    app: &'a AppHandle,
    current: Option<(&'static str, std::time::Instant)>,
}

impl<'a> Steps<'a> {
    fn new(app: &'a AppHandle) -> Self {
        Steps { app, current: None }
    }

    fn next(&mut self, label: &'static str) {
        self.finish();
        let _ = self.app.emit("startup-step", label);
        self.current = Some((label, std::time::Instant::now()));
    }

    fn finish(&mut self) {
        if let Some((label, at)) = self.current.take() {
            tracing::info!(ms = at.elapsed().as_millis() as u64, "open step: {label}");
        }
    }
}

impl Drop for Steps<'_> {
    fn drop(&mut self) {
        self.finish();
    }
}

/// Open a vault, index it, and start watching it.
fn activate(app: &AppHandle, state: &AppState, path: &Path, remember: bool) -> CmdResult<VaultInfo> {
    let mut step = Steps::new(app);
    step.next("Opening your notes");
    let vault = Arc::new(Vault::open(path)?);
    // Let the webview load images (attachments) from inside this vault only.
    if let Err(e) = app.asset_protocol_scope().allow_directory(vault.root(), true) {
        tracing::warn!("could not allow asset access to vault: {e}");
    }
    // Semantic layer: the previous vault's worker stops when its engine drops.
    if let Some(old) = state.semantic.write().unwrap_or_else(|e| e.into_inner()).take() {
        old.shutdown();
    }
    step.next("Loading your preferences");
    let device = this_device(app)?;
    let user = Arc::new(UserState::open(
        &vault.root().join(noughtpad_core::vault::DATA_DIR),
        &device.id,
    )?);
    *state.user.write().unwrap_or_else(|e| e.into_inner()) = Some(user.clone());
    step.next("Preparing search");
    let sem = Semantic::new(vault.clone(), user)?;
    let handle = app.clone();
    sem.on_status(move |s| semantic::emit_status(&handle, s));
    let handle = app.clone();
    sem.on_ideas(move || {
        let _ = handle.emit("ideas-updated", ());
    });
    sem.spawn_worker();
    if load_config(app).semantic == Some(true) {
        semantic::start(app, sem.clone());
    }
    *state.semantic.write().unwrap_or_else(|e| e.into_inner()) = Some(sem.clone());
    // Catch up with edits made while Noughtpad was closed, without blocking the
    // editor: capture must be available immediately, even on a large vault.
    let (bg, handle) = (vault.clone(), app.clone());
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        match bg.rescan() {
            Ok(report) => {
                tracing::info!(
                    ?report,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "vault scanned: {}",
                    bg.root().display()
                );
                if report.indexed + report.removed > 0 {
                    let _ = handle.emit("vault-changed", Vec::<Change>::new());
                }
            }
            Err(e) => tracing::error!("vault scan failed: {e}"),
        }
    });
    *state.vault.write().unwrap_or_else(|e| e.into_inner()) = Some(vault.clone());
    step.next("Almost ready");
    start_watching(app, state, &vault)?;
    if remember {
        let mut cfg = load_config(app);
        cfg.vault_path = Some(vault.root().to_path_buf());
        save_config(app, &cfg);
    }
    Ok(VaultInfo {
        path: vault.root().display().to_string(),
        note_count: vault.count()?,
        default_path: default_vault_path(app).display().to_string(),
        can_switch_vault: cfg!(desktop),
    })
}

// ---- commands ---------------------------------------------------------------

/// The current vault, opening the last-used one on first call. `None` on first run.
#[tauri::command]
async fn vault_info(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Option<VaultInfo>> {
    if let Ok(v) = state.vault() {
        return Ok(Some(VaultInfo {
            path: v.root().display().to_string(),
            note_count: v.count()?,
            default_path: default_vault_path(&app).display().to_string(),
            can_switch_vault: cfg!(desktop),
        }));
    }
    // `NOUGHTPAD_VAULT` (e.g. from `pnpm dev:sample`) opens a folder for this
    // session only, without replacing the remembered notes folder. A desktop
    // idea: mobile has no env vars to set before launch.
    #[cfg(desktop)]
    if let Some(p) = std::env::var_os("NOUGHTPAD_VAULT").map(PathBuf::from) {
        return activate(&app, &state, &p, false).map(Some);
    }
    match load_config(&app).vault_path {
        Some(p) if p.exists() => activate(&app, &state, &p, true).map(Some),
        // Mobile has no folder picker (desktop's "choose a folder" screen), so
        // there's nothing to show while waiting for a choice: open the one
        // fixed location straight away, same as a blank page opening instantly.
        #[cfg(mobile)]
        _ => activate(&app, &state, &default_vault_path(&app), true).map(Some),
        #[cfg(desktop)]
        _ => Ok(None),
    }
}

static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Called by the UI once the editor is focused; logs the time-to-capture budget.
#[tauri::command]
fn mark_ready() -> u64 {
    let ms = since_start();
    tracing::info!(ms, "editor ready");
    ms
}

fn since_start() -> u64 {
    STARTED.get().map_or(0, |t| t.elapsed().as_millis() as u64)
}

#[tauri::command]
fn default_vault(app: AppHandle) -> String {
    default_vault_path(&app).display().to_string()
}

#[tauri::command]
async fn open_vault(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<VaultInfo> {
    activate(&app, &state, Path::new(&path), true)
}

#[tauri::command]
async fn list_notes(state: State<'_, AppState>, query: ListQuery) -> CmdResult<Vec<NoteSummary>> {
    Ok(state.vault()?.list(&query)?)
}

#[tauri::command]
async fn search_notes(state: State<'_, AppState>, query: String, limit: Option<u32>) -> CmdResult<Vec<SearchHit>> {
    // Hybrid (words + meaning) when a local model is active, full-text otherwise.
    let sem = state.semantic()?;
    tauri::async_runtime::spawn_blocking(move || sem.search(&query, limit.unwrap_or(50)))
        .await
        .map_err(|e| CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
async fn get_note(state: State<'_, AppState>, id: String) -> CmdResult<NoteDoc> {
    Ok(state.vault()?.get(&id)?)
}

#[tauri::command]
async fn create_note(state: State<'_, AppState>, input: NewNote) -> CmdResult<NoteDoc> {
    Ok(state.vault()?.create(input)?)
}

#[tauri::command]
async fn update_note(state: State<'_, AppState>, id: String, update: NoteUpdate) -> CmdResult<NoteDoc> {
    Ok(state.vault()?.update(&id, update)?)
}

#[tauri::command]
async fn trash_note(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    Ok(state.vault()?.trash(&id)?)
}

#[tauri::command]
async fn list_labels(state: State<'_, AppState>) -> CmdResult<Vec<LabelCount>> {
    Ok(state.vault()?.labels()?)
}

#[tauri::command]
async fn list_days(state: State<'_, AppState>, query: ListQuery) -> CmdResult<Vec<DaySummary>> {
    Ok(state.vault()?.days(&query)?)
}

#[tauri::command]
async fn list_months(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(state.vault()?.months()?)
}

/// Notes per day from `from_day` (`YYYY-MM-DD`), for the activity map.
#[tauri::command]
async fn day_counts(state: State<'_, AppState>, from_day: String) -> CmdResult<Vec<noughtpad_core::index::DayCount>> {
    Ok(state.vault()?.day_counts(&from_day)?)
}

#[tauri::command]
async fn month_log(state: State<'_, AppState>, month: String) -> CmdResult<MonthLog> {
    Ok(state.vault()?.month_log(&month)?)
}

#[tauri::command]
async fn rebuild_index(state: State<'_, AppState>) -> CmdResult<ScanReport> {
    let v = state.vault()?;
    tauri::async_runtime::spawn_blocking(move || v.rebuild())
        .await
        .map_err(|e| CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}

fn io_err(e: impl std::fmt::Display) -> CmdError {
    CmdError {
        kind: "io",
        message: e.to_string(),
    }
}

/// Write `bytes` under `attachments/YYYY-MM/` and return the vault-relative path.
fn write_attachment(vault: &Vault, bytes: &[u8], extension: &str) -> CmdResult<String> {
    let ext: String = extension
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();
    let ext = if ext.is_empty() {
        "bin".to_string()
    } else {
        ext.to_lowercase()
    };
    let rel = format!(
        "attachments/{}/{}.{ext}",
        chrono::Local::now().format("%Y-%m"),
        ulid::Ulid::new().to_string().to_lowercase()
    );
    let abs = vault.root().join(&rel);
    std::fs::create_dir_all(abs.parent().unwrap())
        .and_then(|_| std::fs::write(&abs, bytes))
        .map_err(io_err)?;
    Ok(rel)
}

/// Save pasted/dropped binary data and return its vault-relative path (for a
/// Markdown link). The bytes arrive base64-encoded in ordinary JSON arguments:
/// a raw binary body doesn't reach the command intact on every platform's IPC
/// (on macOS it failed with "expected the attachment's bytes").
#[tauri::command]
async fn save_attachment(state: State<'_, AppState>, data: String, extension: String) -> CmdResult<String> {
    use base64::Engine as _;
    let v = state.vault()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|e| CmdError {
            kind: "invalid",
            message: format!("the image data was unreadable: {e}"),
        })?;
    tauri::async_runtime::spawn_blocking(move || write_attachment(&v, &bytes, &extension))
        .await
        .map_err(|e| CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
}

/// Save the image on the system clipboard as a PNG attachment, if there is one.
/// The webview doesn't always hand pasted images to the page (a screenshot or an
/// image copied from another app can arrive with no file), so the editor asks
/// here when a paste carries nothing it can use.
#[tauri::command]
async fn paste_clipboard_image(state: State<'_, AppState>) -> CmdResult<Option<String>> {
    let v = state.vault()?;
    #[cfg(desktop)]
    {
        tauri::async_runtime::spawn_blocking(move || {
            let Ok(img) = arboard::Clipboard::new().and_then(|mut c| c.get_image()) else {
                return Ok(None);
            };
            let mut png_bytes = Vec::new();
            {
                let mut enc = png::Encoder::new(&mut png_bytes, img.width as u32, img.height as u32);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                let mut w = enc.write_header().map_err(io_err)?;
                w.write_image_data(&img.bytes).map_err(io_err)?;
            }
            write_attachment(&v, &png_bytes, "png").map(Some)
        })
        .await
        .map_err(|e| CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
    }
    #[cfg(mobile)]
    {
        let _ = v;
        Ok(None)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    STARTED.get_or_init(std::time::Instant::now);
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let mut builder = tauri::Builder::default();
    // One copy of the app per user: a second launch shows the running window
    // and exits, so two copies never open one notebook (its index, user-state
    // and sync engine). Registered first, as the plugin asks.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tracing::info!("second launch: showing the running window");
            lifecycle::show_main(app);
        }));
    }
    builder = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .manage(think::LlmState::default());

    // `capture`/`capture_alt` are registered in `setup` below, which runs on every
    // platform (mobile needs `sync::init` to run too); the shortcut itself is
    // desktop-only, so only its registration is `#[cfg(desktop)]`.
    #[cfg(desktop)]
    let (capture, capture_alt) = {
        use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
        // Quick capture from anywhere: Cmd/Ctrl+Shift+Space.
        let capture = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::Space);
        let capture_alt = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed && (shortcut == &capture || shortcut == &capture_alt) {
                        lifecycle::show_main(app);
                        let _ = app.emit("quick-capture", ());
                    }
                })
                .build(),
        );
        builder = builder
            .plugin(lifecycle::autostart_plugin())
            .plugin(tauri_plugin_updater::Builder::new().build())
            .manage(updater::PendingUpdate::default())
            .on_window_event(|window, event| {
                lifecycle::on_window_event(window, event);
                sync::on_window_event(window, event);
            });
        (capture, capture_alt)
    };
    #[cfg(target_os = "macos")]
    {
        builder = builder.menu(menu::build).on_menu_event(menu::on_menu_event);
    }

    // Runs on every platform: without this, `SyncState` is never managed and
    // every sync command (registered below, unconditionally) would fail at
    // runtime on first call. Found while proving a mobile build of this
    // crate's logic (it used to be nested inside the desktop-only block above).
    builder = builder.setup(move |app| {
        tracing::info!(ms = since_start(), "setup");
        app.manage(sync::init(app.handle()));
        #[cfg(desktop)]
        {
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            lifecycle::show_at_startup(app.handle());
            let shortcut = if cfg!(target_os = "macos") {
                capture
            } else {
                capture_alt
            };
            if let Err(e) = app.global_shortcut().register(shortcut) {
                tracing::warn!("could not register quick-capture shortcut: {e}");
            }
        }
        Ok(())
    });

    builder
        // Startup timeline next to `editor ready`: page load start/finish show the webview's share.
        .on_page_load(|_, payload| tracing::info!(ms = since_start(), event = ?payload.event(), "page load"))
        .invoke_handler(tauri::generate_handler![
            vault_info,
            mark_ready,
            default_vault,
            open_vault,
            list_notes,
            search_notes,
            get_note,
            create_note,
            update_note,
            trash_note,
            list_labels,
            list_days,
            list_months,
            month_log,
            day_counts,
            rebuild_index,
            save_attachment,
            paste_clipboard_image,
            sync::sync_status,
            sync::sync_sign_in,
            sync::sync_sign_out,
            sync::sync_set_enabled,
            sync::sync_now,
            semantic::semantic_status,
            semantic::enable_semantic,
            semantic::disable_semantic,
            semantic::related_notes,
            semantic::dismiss_related,
            semantic::undismiss_related,
            semantic::related_opened,
            semantic::reset_related_feedback,
            semantic::ideas_overview,
            semantic::theme_timeline,
            semantic::note_theme,
            think::llm_overview,
            think::llm_download,
            think::llm_cancel_download,
            think::llm_delete_model,
            think::llm_list_remote_models,
            think::llm_save_remote,
            think::llm_remove_remote,
            think::llm_use,
            think::llm_turn_off,
            think::llm_warm,
            chat::chat_list,
            chat::chat_get,
            chat::chat_delete,
            chat::chat_send,
            chat::chat_cancel,
            #[cfg(desktop)]
            lifecycle::autostart_enabled,
            #[cfg(desktop)]
            lifecycle::set_autostart,
            #[cfg(desktop)]
            updater::app_version,
            #[cfg(desktop)]
            updater::update_check,
            #[cfg(desktop)]
            updater::update_install,
        ])
        .build(tauri::generate_context!())
        .expect("error while running Noughtpad")
        .run(|_app, _event| {
            #[cfg(desktop)]
            lifecycle::on_run_event(_app, &_event);
        });
}
