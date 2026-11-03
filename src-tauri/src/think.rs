//! M4 wiring: the Models page (explicit opt-in to a local model or the user's
//! own provider) and the Thinking workspace (cited answers over chosen notes).
//!
//! Privacy rules enforced here, not just in the UI:
//! * nothing is sent anywhere until the user picks a model (`llm_use`);
//! * the model reads only the notes the user was shown (`think_prepare` →
//!   `think_ask` with those ids);
//! * a remote provider is refused until the user has seen which notes go
//!   where — the first time, and whenever the destination changes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use nought_core::index::NoteSummary;
use nought_core::semantic::context::Scope;
use nought_llm::catalog::{self, LocalModel};
use nought_llm::keys::{default_store, KeyStore};
use nought_llm::local::LocalRuntime;
use nought_llm::prompt::{self, ContextNote};
use nought_llm::settings::{Active, LlmSettings, Remote, RemoteKind};
use nought_llm::{download, ChatRequest, Usage};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{load_config, save_config, AppState, CmdError, CmdResult};

/// Most notes given to the model for one question.
const CONTEXT_NOTES: usize = 8;
const MAX_TOKENS: u32 = 700;

/// A running download: its cancel flag and bytes received so far.
type Download = (Arc<AtomicBool>, u64);

pub struct LlmState {
    keys: Box<dyn KeyStore>,
    keychain: bool,
    local: Arc<LocalRuntime>,
    /// Running downloads: model id → (cancel flag, bytes received).
    downloads: Mutex<HashMap<String, Download>>,
    runs: Mutex<HashMap<String, Arc<AtomicBool>>>,
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

fn err(kind: &'static str, message: impl ToString) -> CmdError {
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

fn models_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("models").join("llm"))
        .map_err(|e| err("io", e))
}

fn settings(app: &AppHandle) -> LlmSettings {
    load_config(app).llm
}

fn update_settings(app: &AppHandle, f: impl FnOnce(&mut LlmSettings)) -> LlmSettings {
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
    local: Vec<LocalEntry>,
    remotes: Vec<RemoteEntry>,
    providers: Vec<ProviderInfo>,
    local_available: bool,
    remote_available: bool,
    /// Keys are kept in the OS keychain (false: only until Nought quits).
    keychain: bool,
}

fn active_label(s: &LlmSettings) -> Option<String> {
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

/// Opt out: no model answers, and a loaded local model is freed.
#[tauri::command]
pub async fn llm_turn_off(app: AppHandle, llm: State<'_, LlmState>) -> CmdResult<()> {
    llm.local.unload().await;
    update_settings(&app, |s| s.active = None);
    Ok(())
}

// ---- Thinking workspace ----------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    label: String,
    model: String,
    /// True when notes stay on this computer.
    on_device: bool,
    /// Where notes go, in words ("Anthropic", "http://localhost:11434").
    destination: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prepared {
    notes: Vec<NoteSummary>,
    provider: Option<ProviderView>,
    /// Show the notes and destination and ask before sending.
    needs_consent: bool,
}

fn provider_view(s: &LlmSettings) -> Option<ProviderView> {
    match s.active.as_ref()? {
        Active::Local { model } => catalog::find(model).map(|m| ProviderView {
            label: m.name.into(),
            model: m.name.into(),
            on_device: true,
            destination: "this computer".into(),
        }),
        Active::Remote { id } => s.remote(id).map(|r| ProviderView {
            label: r.label.clone(),
            model: r.model.clone(),
            on_device: false,
            destination: r.endpoint().map_or_else(|| r.kind.label().to_string(), str::to_string),
        }),
    }
}

/// Which notes would be read for this question, and whether to ask first.
#[tauri::command]
pub fn think_prepare(
    app: AppHandle,
    state: State<'_, AppState>,
    question: String,
    scope: Scope,
) -> CmdResult<Prepared> {
    let s = settings(&app);
    let notes = state
        .semantic()?
        .context(&scope, &question, CONTEXT_NOTES)?
        .into_iter()
        .map(|c| c.note)
        .collect();
    Ok(Prepared {
        notes,
        provider: provider_view(&s),
        needs_consent: s.active_remote().is_some_and(|r| s.needs_consent(r)),
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaEvent {
    request_id: String,
    text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    request_id: String,
    /// Note ids in the order the answer first cites them.
    cited: Vec<String>,
    usage: Usage,
    error: Option<String>,
    cancelled: bool,
}

/// Answer `question` from exactly `note_ids` (the list the user was shown).
/// Text streams as `think-delta` events, then one `think-done`.
#[tauri::command]
pub async fn think_ask(
    app: AppHandle,
    state: State<'_, AppState>,
    llm: State<'_, LlmState>,
    request_id: String,
    question: String,
    note_ids: Vec<String>,
    consent: bool,
) -> CmdResult<()> {
    let s = settings(&app);
    let Some(active) = s.active.clone() else {
        return Err(err("noModel", "Choose a model on the Models page first"));
    };
    if note_ids.is_empty() {
        return Err(err("invalid", "No notes to think with"));
    }
    let remote = s.active_remote().cloned();
    if let Some(r) = &remote {
        if s.needs_consent(r) {
            if !consent {
                return Err(err("consent", "Confirm which notes are sent first"));
            }
            update_settings(&app, |s| s.record_consent(r));
        }
    }

    // Read the notes now, in the order shown (oldest first).
    let vault = state.vault()?;
    let mut notes: Vec<(String, ContextNote)> = Vec::new();
    for id in &note_ids {
        let doc = vault.get(id)?;
        let summary = vault.summary(id)?.ok_or_else(|| err("notFound", id))?;
        notes.push((
            summary.created.clone(),
            ContextNote {
                id: id.clone(),
                day: summary.day.clone(),
                title: doc.title.clone(),
                source: doc.source.as_ref().and_then(|s| s.title.clone().or(s.url.clone())),
                labels: doc.labels.clone(),
                body: doc.content,
            },
        ));
    }
    notes.sort_by(|a, b| a.0.cmp(&b.0));
    let notes: Vec<ContextNote> = notes.into_iter().map(|(_, n)| n).collect();
    let req = ChatRequest {
        system: prompt::SYSTEM.into(),
        user: prompt::user_message(&question, &notes),
        max_tokens: MAX_TOKENS,
        temperature: 0.2,
    };

    let cancel = Arc::new(AtomicBool::new(false));
    llm.runs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(request_id.clone(), cancel.clone());
    let key = match &remote {
        Some(r) => llm.keys.get(&r.id)?,
        None => None,
    };
    let local = llm.local.clone();
    let dir = models_dir(&app)?;

    tauri::async_runtime::spawn(async move {
        let mut text = String::new();
        let mut on_text = |t: &str| {
            text.push_str(t);
            let _ = app.emit(
                "think-delta",
                DeltaEvent {
                    request_id: request_id.clone(),
                    text: t.to_string(),
                },
            );
        };
        let result = match (&active, &remote) {
            (Active::Local { model }, _) => match catalog::find(model) {
                Some(m) => local.chat(&dir, m, &req, &cancel, &mut on_text).await,
                None => Err(nought_llm::Error::Unavailable("unknown model".into())),
            },
            (Active::Remote { .. }, Some(r)) => nought_llm::remote::chat(r, key, &req, &cancel, &mut on_text).await,
            (Active::Remote { .. }, None) => Err(nought_llm::Error::Unavailable("provider removed".into())),
        };
        let cited = prompt::citations(&text, notes.len())
            .into_iter()
            .map(|k| notes[k - 1].id.clone())
            .collect();
        let (usage, error, cancelled) = match result {
            Ok(u) => (u, None, false),
            Err(nought_llm::Error::Cancelled) => (Usage::default(), None, true),
            Err(e) => {
                tracing::warn!("thinking failed: {e}");
                (Usage::default(), Some(e.to_string()), false)
            }
        };
        app.state::<LlmState>()
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&request_id);
        let _ = app.emit(
            "think-done",
            DoneEvent {
                request_id,
                cited,
                usage,
                error,
                cancelled,
            },
        );
    });
    Ok(())
}

#[tauri::command]
pub fn think_cancel(llm: State<'_, LlmState>, request_id: String) {
    if let Some(c) = llm.runs.lock().unwrap_or_else(|e| e.into_inner()).get(&request_id) {
        c.store(true, Ordering::Relaxed);
    }
}

// ---- Kept answers -----------------------------------------------------------

const ARTIFACTS: &str = "artifacts";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeepInput {
    question: String,
    answer: String,
    note_ids: Vec<String>,
    model: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    /// File name inside `.nought/artifacts`.
    file: String,
    question: String,
    created: String,
    model: String,
    note_ids: Vec<String>,
    answer: String,
}

fn artifacts_dir(state: &AppState) -> CmdResult<PathBuf> {
    Ok(state.vault()?.root().join(nought_core::vault::DATA_DIR).join(ARTIFACTS))
}

fn slug(s: &str) -> String {
    let words: Vec<String> = s
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(6)
        .map(|w| w.to_lowercase())
        .collect();
    if words.is_empty() {
        "answer".into()
    } else {
        words.join("-")
    }
}

/// Save an answer as Markdown in `.nought/artifacts` (never among the user's notes).
#[tauri::command]
pub fn keep_answer(state: State<'_, AppState>, input: KeepInput) -> CmdResult<Kept> {
    let dir = artifacts_dir(&state)?;
    std::fs::create_dir_all(&dir).map_err(|e| err("io", e))?;
    let now = chrono::Local::now();
    let file = format!("{}-{}.md", now.format("%Y-%m-%d-%H%M%S"), slug(&input.question));
    let kept = Kept {
        file: file.clone(),
        question: input.question.trim().into(),
        created: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        model: input.model,
        note_ids: input.note_ids,
        answer: input.answer.trim().into(),
    };
    let front = serde_json::json!({
        "question": kept.question,
        "created": kept.created,
        "model": kept.model,
        "notes": kept.note_ids,
    });
    let text = format!("---\n{}---\n\n{}\n", yaml_front(&front), kept.answer);
    std::fs::write(dir.join(&file), text).map_err(|e| err("io", e))?;
    Ok(kept)
}

/// Minimal YAML for the kept-answer header (strings and string lists).
fn yaml_front(v: &serde_json::Value) -> String {
    let mut out = String::new();
    if let Some(map) = v.as_object() {
        for (k, val) in map {
            match val {
                serde_json::Value::Array(items) => {
                    out.push_str(&format!("{k}:\n"));
                    for i in items {
                        out.push_str(&format!("  - {}\n", i));
                    }
                }
                other => out.push_str(&format!("{k}: {other}\n")),
            }
        }
    }
    out
}

fn parse_kept(file: &str, text: &str) -> Option<Kept> {
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---\n")?;
    let (head, body) = (&rest[..end], &rest[end + 5..]);
    let mut question = String::new();
    let mut created = String::new();
    let mut model = String::new();
    let mut note_ids = Vec::new();
    for line in head.lines() {
        let unq = |s: &str| serde_json::from_str::<String>(s.trim()).unwrap_or_else(|_| s.trim().to_string());
        if let Some(v) = line.strip_prefix("question:") {
            question = unq(v);
        } else if let Some(v) = line.strip_prefix("created:") {
            created = unq(v);
        } else if let Some(v) = line.strip_prefix("model:") {
            model = unq(v);
        } else if let Some(v) = line.strip_prefix("  - ") {
            note_ids.push(unq(v));
        }
    }
    Some(Kept {
        file: file.into(),
        question,
        created,
        model,
        note_ids,
        answer: body.trim().into(),
    })
}

/// Kept answers, newest first.
#[tauri::command]
pub fn kept_answers(state: State<'_, AppState>) -> CmdResult<Vec<Kept>> {
    let dir = artifacts_dir(&state)?;
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(vec![]);
    };
    let mut out: Vec<Kept> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") {
                return None;
            }
            parse_kept(&name, &std::fs::read_to_string(e.path()).ok()?)
        })
        .collect();
    out.sort_by(|a, b| b.file.cmp(&a.file));
    Ok(out)
}

#[tauri::command]
pub fn forget_kept(state: State<'_, AppState>, file: String) -> CmdResult<()> {
    if file.contains('/') || file.contains('\\') || !file.ends_with(".md") {
        return Err(err("invalid", "bad file name"));
    }
    let p = artifacts_dir(&state)?.join(file);
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| err("io", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kept_roundtrip() {
        let front = serde_json::json!({
            "question": "What's \"open\"?",
            "created": "2026-11-03T10:00:00+05:30",
            "model": "Qwen3 4B",
            "notes": ["01a", "01b"],
        });
        let text = format!("---\n{}---\n\nYou asked [1].\n", yaml_front(&front));
        let k = parse_kept("x.md", &text).unwrap();
        assert_eq!(k.question, "What's \"open\"?");
        assert_eq!(k.note_ids, vec!["01a", "01b"]);
        assert_eq!(k.answer, "You asked [1].");
        assert_eq!(k.model, "Qwen3 4B");
    }

    #[test]
    fn slugs() {
        assert_eq!(
            slug("How has my thinking about ownership in VC changed?"),
            "how-has-my-thinking-about-ownership"
        );
        assert_eq!(slug("??"), "answer");
    }
}
