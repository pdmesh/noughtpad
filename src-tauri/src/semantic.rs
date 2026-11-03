//! Wiring for local semantic features: enabling (download + load the model),
//! status events, related notes and dismissals.

use std::sync::Arc;

use nought_core::semantic::ideas::{IdeasOverview, ThemeSummary, ThemeTimeline};
use nought_core::semantic::model::DEFAULT_MODEL;
use nought_core::semantic::{RelatedNotes, Semantic, Status};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{load_config, save_config, AppState, CmdError, CmdResult};

/// Whether this build includes the ONNX embedding runtime.
pub const AVAILABLE: bool = cfg!(any(feature = "embedding-static", feature = "embedding-dynamic"));

fn model_dir(app: &AppHandle) -> CmdResult<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("models"))
        .map_err(|e| CmdError {
            kind: "io",
            message: e.to_string(),
        })
}

/// Download (if needed) and load the model in the background, reporting
/// progress through the engine's status.
pub fn start(app: &AppHandle, sem: Arc<Semantic>) {
    if !AVAILABLE {
        sem.set_status(Status::Error {
            message: "This build of Nought has no local model runtime.".into(),
        });
        return;
    }
    let dir = match model_dir(app) {
        Ok(d) => d,
        Err(e) => {
            sem.set_status(Status::Error { message: e.message });
            return;
        }
    };
    std::thread::spawn(move || {
        if let Err(e) = load(&sem, &dir) {
            tracing::error!("enabling local semantic search failed: {e}");
            sem.set_status(Status::Error { message: e.to_string() });
        }
    });
}

#[cfg(any(feature = "embedding-static", feature = "embedding-dynamic"))]
fn load(sem: &Semantic, models: &std::path::Path) -> nought_core::Result<()> {
    use nought_core::semantic::model::{bundle_downloaded, download_bundle, model_dir, FastEmbedder};
    // Background work should never make the machine hot: use at most half the cores.
    let threads = std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).clamp(1, 4));
    let edir = model_dir(models);

    // One download for the default bundle.
    if !bundle_downloaded(models) {
        sem.set_status(Status::Downloading { received: 0, total: 0 });
        let mut last = 0u64;
        download_bundle(models, |received, total| {
            // Report every ~1 MB to keep events cheap.
            if received - last >= 1 << 20 || received == total {
                last = received;
                sem.set_status(Status::Downloading { received, total });
            }
        })?;
    }
    sem.set_status(Status::Loading);
    sem.set_embedder(Box::new(FastEmbedder::load(&DEFAULT_MODEL, &edir, threads)?))?;
    Ok(())
}

#[cfg(not(any(feature = "embedding-static", feature = "embedding-dynamic")))]
fn load(_sem: &Semantic, _dir: &std::path::Path) -> nought_core::Result<()> {
    Ok(())
}

pub fn emit_status(app: &AppHandle, status: &Status) {
    let _ = app.emit("semantic-status", status);
}

#[tauri::command]
pub async fn semantic_status(state: State<'_, AppState>) -> CmdResult<Status> {
    Ok(state.semantic().map(|s| s.status()).unwrap_or(Status::Off))
}

#[tauri::command]
pub async fn enable_semantic(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Status> {
    let sem = state.semantic()?;
    let mut cfg = load_config(&app);
    cfg.semantic = Some(true);
    save_config(&app, &cfg);
    if !sem.is_enabled() {
        start(&app, sem.clone());
    }
    Ok(sem.status())
}

#[tauri::command]
pub async fn disable_semantic(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Status> {
    let sem = state.semantic()?;
    let mut cfg = load_config(&app);
    cfg.semantic = Some(false);
    save_config(&app, &cfg);
    sem.disable();
    Ok(sem.status())
}

#[tauri::command]
pub async fn related_notes(state: State<'_, AppState>, id: String, limit: Option<usize>) -> CmdResult<RelatedNotes> {
    let sem = state.semantic()?;
    tauri::async_runtime::spawn_blocking(move || sem.related(&id, limit.unwrap_or(8)))
        .await
        .map_err(|e| CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn dismiss_related(state: State<'_, AppState>, a: String, b: String) -> CmdResult<()> {
    Ok(state.semantic()?.dismiss(&a, &b)?)
}

#[tauri::command]
pub async fn undismiss_related(state: State<'_, AppState>, a: String, b: String) -> CmdResult<()> {
    Ok(state.semantic()?.undismiss(&a, &b)?)
}

#[tauri::command]
pub async fn ideas_overview(state: State<'_, AppState>) -> CmdResult<IdeasOverview> {
    let sem = state.semantic()?;
    tauri::async_runtime::spawn_blocking(move || sem.ideas_overview())
        .await
        .map_err(|e| CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn theme_timeline(state: State<'_, AppState>, id: String) -> CmdResult<Option<ThemeTimeline>> {
    let sem = state.semantic()?;
    tauri::async_runtime::spawn_blocking(move || sem.theme_timeline(&id))
        .await
        .map_err(|e| CmdError {
            kind: "internal",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn note_theme(state: State<'_, AppState>, id: String) -> CmdResult<Option<ThemeSummary>> {
    Ok(state.semantic()?.note_theme(&id)?)
}

/// The user opened a suggested note from the Related panel (implicit "useful").
#[tauri::command]
pub async fn related_opened(state: State<'_, AppState>, a: String, b: String) -> CmdResult<()> {
    Ok(state.semantic()?.opened_related(&a, &b)?)
}

/// Forget what related notes learned from feedback (dismissals stay hidden).
#[tauri::command]
pub async fn reset_related_feedback(state: State<'_, AppState>) -> CmdResult<()> {
    Ok(state.semantic()?.reset_feedback()?)
}
