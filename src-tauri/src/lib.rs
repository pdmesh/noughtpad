//! Tauri shell: exposes `noughtpad-core` to the UI as commands and forwards
//! filesystem changes as events.

mod chat;
#[cfg(desktop)]
mod lifecycle;
#[cfg(target_os = "macos")]
mod menu;
#[cfg(desktop)]
mod updater;
mod semantic;
mod think;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use noughtpad_core::index::{DaySummary, LabelCount, ListQuery, MonthLog, NoteSummary, SearchHit};
use noughtpad_core::semantic::Semantic;
use noughtpad_core::userstate::{UserState, USER_STATE_FILE};
use noughtpad_core::vault::{Change, NewNote, NoteDoc, NoteUpdate, ScanReport};
use noughtpad_core::watch::{watch, VaultWatcher};
use noughtpad_core::Vault;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
pub(crate) struct AppState {
    vault: RwLock<Option<Arc<Vault>>>,
    watcher: Mutex<Option<VaultWatcher>>,
    semantic: RwLock<Option<Arc<Semantic>>>,
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
}

fn default_vault_path() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Noughtpad")
}

/// Open a vault, index it, and start watching it.
fn activate(app: &AppHandle, state: &AppState, path: &Path, remember: bool) -> CmdResult<VaultInfo> {
    let vault = Arc::new(Vault::open(path)?);
    // Let the webview load images (attachments) from inside this vault only.
    if let Err(e) = app.asset_protocol_scope().allow_directory(vault.root(), true) {
        tracing::warn!("could not allow asset access to vault: {e}");
    }
    // Semantic layer: the previous vault's worker stops when its engine drops.
    if let Some(old) = state.semantic.write().unwrap_or_else(|e| e.into_inner()).take() {
        old.shutdown();
    }
    let user = Arc::new(UserState::open(
        &vault.root().join(noughtpad_core::vault::DATA_DIR).join(USER_STATE_FILE),
    )?);
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

    let handle = app.clone();
    let watcher = watch(vault.clone(), move |changes: Vec<Change>| {
        sem.nudge();
        let _ = handle.emit("vault-changed", changes);
    })?;
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
    *state.watcher.lock().unwrap_or_else(|e| e.into_inner()) = Some(watcher);
    *state.vault.write().unwrap_or_else(|e| e.into_inner()) = Some(vault.clone());
    if remember {
        let mut cfg = load_config(app);
        cfg.vault_path = Some(vault.root().to_path_buf());
        save_config(app, &cfg);
    }
    Ok(VaultInfo {
        path: vault.root().display().to_string(),
        note_count: vault.count()?,
        default_path: default_vault_path().display().to_string(),
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
            default_path: default_vault_path().display().to_string(),
        }));
    }
    // `NOUGHTPAD_VAULT` (e.g. from `pnpm dev:sample`) opens a folder for this
    // session only, without replacing the remembered notes folder.
    if let Some(p) = std::env::var_os("NOUGHTPAD_VAULT").map(PathBuf::from) {
        return activate(&app, &state, &p, false).map(Some);
    }
    match load_config(&app).vault_path {
        Some(p) if p.exists() => activate(&app, &state, &p, true).map(Some),
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
fn default_vault() -> String {
    default_vault_path().display().to_string()
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

/// Save pasted/dropped binary data under `attachments/YYYY-MM/` and return
/// its vault-relative path (for a Markdown link).
#[tauri::command]
async fn save_attachment(state: State<'_, AppState>, bytes: Vec<u8>, extension: String) -> CmdResult<String> {
    let v = state.vault()?;
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
    let abs = v.root().join(&rel);
    std::fs::create_dir_all(abs.parent().unwrap())
        .and_then(|_| std::fs::write(&abs, bytes))
        .map_err(|e| CmdError {
            kind: "io",
            message: e.to_string(),
        })?;
    Ok(rel)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    STARTED.get_or_init(std::time::Instant::now);
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .manage(think::LlmState::default());

    #[cfg(desktop)]
    {
        use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
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
            .on_window_event(lifecycle::on_window_event);
        #[cfg(target_os = "macos")]
        {
            builder = builder.menu(menu::build).on_menu_event(menu::on_menu_event);
        }
        builder = builder.setup(move |app| {
            tracing::info!(ms = since_start(), "setup");
            lifecycle::show_at_startup(app.handle());
            let shortcut = if cfg!(target_os = "macos") {
                capture
            } else {
                capture_alt
            };
            if let Err(e) = app.global_shortcut().register(shortcut) {
                tracing::warn!("could not register quick-capture shortcut: {e}");
            }
            Ok(())
        });
    }

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
