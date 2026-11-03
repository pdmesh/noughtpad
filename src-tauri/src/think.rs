//! The Models page: explicit opt-in to a local model or the user's own
//! provider. Nothing is sent anywhere until the user picks a model
//! (`llm_use`). Chats live in `chat.rs`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use nought_llm::catalog::{self, LocalModel};
use nought_llm::download;
use nought_llm::keys::{default_store, KeyStore};
use nought_llm::local::LocalRuntime;
use nought_llm::settings::{Active, LlmSettings, Remote, RemoteKind};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{load_config, save_config, CmdError, CmdResult};

/// A running download: its cancel flag and bytes received so far.
type Download = (Arc<AtomicBool>, u64);

pub struct LlmState {
    pub(crate) keys: Box<dyn KeyStore>,
    keychain: bool,
    pub(crate) local: Arc<LocalRuntime>,
    /// Running downloads: model id → (cancel flag, bytes received).
    downloads: Mutex<HashMap<String, Download>>,
    /// Running chats: chat id → cancel flag.
    pub(crate) runs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Default for LlmState {
    fn default() -> Self {
        let (keys, keychain) = default_store();
        Self {
            keys,
            keychain,
            local: Arc::new(LocalRuntime::default()),
            downloads: Mutex::new(HashMap::new()),
            runs: Mutex::new(HashMap::new()),
        }
    }
}

pub(crate) fn err(kind: &'static str, message: impl ToString) -> CmdError {
    CmdError {
        kind,
        message: message.to_string(),
    }
}

impl From<nought_llm::Error> for CmdError {
    fn from(e: nought_llm::Error) -> Self {
        let kind = match e {
            nought_llm::Error::Cancelled => "cancelled",
            nought_llm::Error::Provider(_) => "provider",
            _ => "internal",
        };
        err(kind, e)
    }
}

pub(crate) fn models_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("models").join("llm"))
        .map_err(|e| err("io", e))
}

pub(crate) fn settings(app: &AppHandle) -> LlmSettings {
    load_config(app).llm
}

pub(crate) fn update_settings(app: &AppHandle, f: impl FnOnce(&mut LlmSettings)) -> LlmSettings {
    let mut cfg = load_config(app);
    f(&mut cfg.llm);
    save_config(app, &cfg);
    cfg.llm
}

fn local_model(id: &str) -> CmdResult<&'static LocalModel> {
    catalog::find(id).ok_or_else(|| err("notFound", format!("unknown model {id}")))
}

// ---- Models page ----------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalEntry {
    model: &'static LocalModel,
    downloaded: bool,
    /// Bytes of an unfinished download (resumable), or of one in progress.
    received: u64,
    downloading: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntry {
    remote: Remote,
    has_key: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    kind: RemoteKind,
    label: &'static str,
    needs_key: bool,
    needs_url: bool,
    default_url: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmOverview {
    active: Option<Active>,
    /// Human-readable name of the active model, e.g. "Qwen3 4B Instruct (on this computer)".
    active_label: Option<String>,
    /// Where notes go with the active provider (`None` for a local model).
    active_destination: Option<String>,
    /// The same, for people: a server address or the provider's name.
    active_where: Option<String>,
    local: Vec<LocalEntry>,
    remotes: Vec<RemoteEntry>,
    providers: Vec<ProviderInfo>,
    local_available: bool,
    remote_available: bool,
    /// Keys are kept in the OS keychain (false: only until Nought quits).
    keychain: bool,
}

pub(crate) fn active_label(s: &LlmSettings) -> Option<String> {
    match s.active.as_ref()? {
        Active::Local { model } => catalog::find(model).map(|m| format!("{} (on this computer)", m.name)),
        Active::Remote { id } => s.remote(id).map(|r| format!("{} · {}", r.label, r.model)),
    }
}

#[tauri::command]
pub fn llm_overview(app: AppHandle, llm: State<'_, LlmState>) -> CmdResult<LlmOverview> {
    let s = settings(&app);
    let dir = models_dir(&app)?;
    let downloads = llm.downloads.lock().unwrap_or_else(|e| e.into_inner());
    let local = catalog::CATALOG
        .iter()
        .map(|m| {
            let active = downloads.get(m.id);
            LocalEntry {
                model: m,
                downloaded: download::is_downloaded(&dir, m),
                received: active.map_or_else(|| download::partial_bytes(&dir, m), |d| d.1),
                downloading: active.is_some(),
            }
        })
        .collect();
    let remotes = s
        .remotes
        .iter()
        .map(|r| RemoteEntry {
            remote: r.clone(),
            has_key: llm.keys.get(&r.id).ok().flatten().is_some(),
        })
        .collect();
    let providers = [
        RemoteKind::Anthropic,
        RemoteKind::OpenAi,
        RemoteKind::Gemini,
        RemoteKind::Ollama,
        RemoteKind::OpenAiCompatible,
    ]
    .into_iter()
    .map(|k| ProviderInfo {
        kind: k,
        label: k.label(),
        needs_key: k.needs_key(),
        needs_url: k.needs_url(),
        default_url: k.default_url(),
    })
    .collect();
    Ok(LlmOverview {
        active_label: active_label(&s),
        active_destination: s.active_remote().map(|r| r.destination()),
        active_where: s
            .active_remote()
            .map(|r| r.endpoint().map_or_else(|| r.kind.label().to_string(), str::to_string)),
        active: s.active,
        local,
        remotes,
        providers,
        local_available: nought_llm::local::AVAILABLE,
        remote_available: nought_llm::remote::AVAILABLE,
        keychain: llm.keychain,
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadEvent {
    id: String,
    state: &'static str,
    received: u64,
    total: u64,
    message: Option<String>,
}

/// Start (or resume) downloading a local model. Progress arrives as `llm-download` events.
#[tauri::command]
pub fn llm_download(app: AppHandle, llm: State<'_, LlmState>, id: String) -> CmdResult<()> {
    let m = local_model(&id)?;
    let dir = models_dir(&app)?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut d = llm.downloads.lock().unwrap_or_else(|e| e.into_inner());
        if d.contains_key(&id) {
            return Ok(());
        }
        d.insert(id.clone(), (cancel.clone(), download::partial_bytes(&dir, m)));
    }
    std::thread::spawn(move || {
        let emit = |state: &'static str, received: u64, message: Option<String>| {
            let _ = app.emit(
                "llm-download",
                DownloadEvent {
                    id: m.id.into(),
                    state,
                    received,
                    total: m.bytes,
                    message,
                },
            );
        };
        let st = app.state::<LlmState>();
        let result = download::download(&dir, m, &cancel, |received, _| {
            if let Some(e) = st.downloads.lock().unwrap_or_else(|e| e.into_inner()).get_mut(m.id) {
                e.1 = received;
            }
            emit("downloading", received, None);
        });
        st.downloads.lock().unwrap_or_else(|e| e.into_inner()).remove(m.id);
        match result {
            Ok(_) => emit("done", m.bytes, None),
            Err(nought_llm::Error::Cancelled) => emit("cancelled", download::partial_bytes(&dir, m), None),
            Err(e) => {
                tracing::warn!("downloading {} failed: {e}", m.id);
                emit("error", download::partial_bytes(&dir, m), Some(e.to_string()))
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub fn llm_cancel_download(llm: State<'_, LlmState>, id: String) {
    if let Some((c, ..)) = llm.downloads.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
        c.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
pub async fn llm_delete_model(app: AppHandle, llm: State<'_, LlmState>, id: String) -> CmdResult<()> {
    let m = local_model(&id)?;
    if matches!(&settings(&app).active, Some(Active::Local { model }) if model == &id) {
        llm.local.unload().await;
        update_settings(&app, |s| s.active = None);
    }
    download::remove(&models_dir(&app)?, m)?;
    Ok(())
}

/// Model names a provider offers, to pick from. `api_key` is the one typed in
/// the form; without it the saved key of `remote_id` is used.
#[tauri::command]
pub async fn llm_list_remote_models(
    llm: State<'_, LlmState>,
    kind: RemoteKind,
    base_url: Option<String>,
    api_key: Option<String>,
    remote_id: Option<String>,
) -> CmdResult<Vec<String>> {
    let key = match api_key.filter(|k| !k.trim().is_empty()) {
        Some(k) => Some(k),
        None => match remote_id {
            Some(id) => llm.keys.get(&id)?,
            None => None,
        },
    };
    Ok(nought_llm::remote::list_models(kind, base_url, key).await?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInput {
    id: Option<String>,
    kind: RemoteKind,
    label: Option<String>,
    base_url: Option<String>,
    model: String,
    /// New key; `None` keeps the saved one.
    api_key: Option<String>,
}

/// Save a provider (and its key, in the keychain). Doesn't select it.
#[tauri::command]
pub fn llm_save_remote(app: AppHandle, llm: State<'_, LlmState>, input: RemoteInput) -> CmdResult<Remote> {
    if input.model.trim().is_empty() {
        return Err(err("invalid", "Choose a model"));
    }
    if input.kind.needs_url() && input.base_url.as_deref().is_none_or(|u| u.trim().is_empty()) {
        return Err(err("invalid", "Enter the server's address"));
    }
    let id = input.id.unwrap_or_else(|| ulid::Ulid::new().to_string().to_lowercase());
    if let Some(k) = input.api_key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        llm.keys.set(&id, k)?;
    }
    if input.kind.needs_key() && llm.keys.get(&id)?.is_none() {
        return Err(err("invalid", format!("{} needs an API key", input.kind.label())));
    }
    let remote = Remote {
        id: id.clone(),
        kind: input.kind,
        label: input
            .label
            .filter(|l| !l.trim().is_empty())
            .unwrap_or_else(|| input.kind.label().into()),
        base_url: input.base_url.filter(|u| !u.trim().is_empty()),
        model: input.model.trim().into(),
    };
    update_settings(&app, |s| match s.remotes.iter_mut().find(|r| r.id == id) {
        Some(r) => *r = remote.clone(),
        None => s.remotes.push(remote.clone()),
    });
    Ok(remote)
}

#[tauri::command]
pub fn llm_remove_remote(app: AppHandle, llm: State<'_, LlmState>, id: String) -> CmdResult<()> {
    llm.keys.delete(&id)?;
    update_settings(&app, |s| s.remove_remote(&id));
    Ok(())
}

/// The explicit opt-in: choose the model that answers.
#[tauri::command]
pub async fn llm_use(app: AppHandle, llm: State<'_, LlmState>, active: Active) -> CmdResult<()> {
    let s = settings(&app);
    match &active {
        Active::Local { model } => {
            let m = local_model(model)?;
            if !download::is_downloaded(&models_dir(&app)?, m) {
                return Err(err("invalid", format!("Download {} first", m.name)));
            }
        }
        Active::Remote { id } => {
            if s.remote(id).is_none() {
                return Err(err("notFound", "That provider was removed"));
            }
            llm.local.unload().await;
        }
    }
    update_settings(&app, |s| s.active = Some(active));
    Ok(())
}

/// Load the active local model in the background (called when chat opens),
/// so the first answer doesn't wait for a cold read of the model file.
#[tauri::command]
pub fn llm_warm(app: AppHandle, llm: State<'_, LlmState>) -> CmdResult<()> {
    if let Some(Active::Local { model }) = settings(&app).active {
        if let Some(m) = catalog::find(&model) {
            let dir = models_dir(&app)?;
            let local = llm.local.clone();
            tauri::async_runtime::spawn(async move {
                let t = std::time::Instant::now();
                match local.warm(&dir, m).await {
                    Ok(()) => tracing::info!("{} ready after {:.1}s", m.name, t.elapsed().as_secs_f32()),
                    Err(e) => tracing::warn!("warming {} failed: {e}", m.name),
                }
            });
        }
    }
    Ok(())
}

/// Opt out: no model answers, and a loaded local model is freed.
#[tauri::command]
pub async fn llm_turn_off(app: AppHandle, llm: State<'_, LlmState>) -> CmdResult<()> {
    llm.local.unload().await;
    update_settings(&app, |s| s.active = None);
    Ok(())
}
