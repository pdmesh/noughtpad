//! Chat with your notes (M4.5).
//!
//! * Online providers search and read notes through read-only tools, bounded
//!   by the chat's scope (a note, theme, month, collection or the whole
//!   notebook). Every search and read appears in the chat as a step.
//! * Local models, and providers that reject tools, get the most relevant
//!   notes for each turn chosen by the app instead.
//! * Chats are saved as JSON in `.noughtpad/chats/` (the chat history). Notes are
//!   never written.
//!
//! Privacy, enforced here: nothing runs until a model is chosen, and a remote
//! provider needs the user's approval of this chat's scope and destination
//! before the first note leaves the machine.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use noughtpad_core::index::NoteSummary;
use noughtpad_core::semantic::context::Scope;
use noughtpad_core::semantic::Semantic;
use noughtpad_core::Vault;
use noughtpad_llm::catalog;
use noughtpad_llm::prompt::{self, ContextNote};
use noughtpad_llm::settings::Active;
use noughtpad_llm::{ChatRequest, NoteTools, ToolSpec, Turn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::think::{active_label, err, models_dir, settings, update_settings, LlmState};
use crate::{AppState, CmdResult};

const MAX_TOOL_ROUNDS: usize = 6;
const MAX_TOKENS: u32 = 900;
/// Earlier exchanges sent along with a new question.
const HISTORY_TURNS: usize = 6;
/// Notes chosen per turn when the model can't use tools.
const TURN_NOTES: usize = 6;
const SEARCH_HITS: usize = 6;
const LIST_NOTES: usize = 12;
const CHATS: &str = "chats";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatNote {
    /// The note's number in this chat (`[n]`), from 1.
    pub n: usize,
    pub id: String,
    pub title: String,
    pub day: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// What happened, e.g. `Searched “ownership”`.
    pub label: String,
    /// Numbers of the notes it surfaced or read.
    pub notes: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// `user` or `assistant`.
    pub role: String,
    pub text: String,
    #[serde(default)]
    pub steps: Vec<Step>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: String,
    pub title: String,
    pub created: String,
    pub updated: String,
    pub scope: Scope,
    pub scope_label: String,
    /// The remote destination the user approved for this chat, if any.
    #[serde(default)]
    pub approved: Option<String>,
    #[serde(default)]
    pub notes: Vec<ChatNote>,
    #[serde(default)]
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    id: String,
    title: String,
    updated: String,
    scope_label: String,
    questions: usize,
}

fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

fn chats_dir(vault: &Vault) -> PathBuf {
    vault.root().join(noughtpad_core::vault::DATA_DIR).join(CHATS)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric())
}

fn load(vault: &Vault, id: &str) -> CmdResult<Chat> {
    if !valid_id(id) {
        return Err(err("invalid", "bad chat id"));
    }
    let p = chats_dir(vault).join(format!("{id}.json"));
    let text = std::fs::read_to_string(&p).map_err(|_| err("notFound", "That chat was deleted"))?;
    serde_json::from_str(&text).map_err(|e| err("internal", format!("chat file unreadable: {e}")))
}

fn save(vault: &Vault, chat: &Chat) -> CmdResult<()> {
    let dir = chats_dir(vault);
    std::fs::create_dir_all(&dir).map_err(|e| err("io", e))?;
    let tmp = dir.join(format!("{}.json.tmp", chat.id));
    std::fs::write(&tmp, serde_json::to_vec_pretty(chat).expect("chat serialises")).map_err(|e| err("io", e))?;
    std::fs::rename(&tmp, dir.join(format!("{}.json", chat.id))).map_err(|e| err("io", e))
}

fn title_from(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match flat.char_indices().nth(60) {
        Some((i, _)) => format!("{}…", flat[..i].trim_end()),
        None => flat,
    }
}

/// Chats, most recently updated first.
#[tauri::command]
pub fn chat_list(state: State<'_, AppState>) -> CmdResult<Vec<ChatSummary>> {
    let vault = state.vault()?;
    let Ok(rd) = std::fs::read_dir(chats_dir(&vault)) else {
        return Ok(vec![]);
    };
    let mut out: Vec<ChatSummary> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let id = name.strip_suffix(".json")?;
            let c = load(&vault, id).ok()?;
            Some(ChatSummary {
                questions: c.messages.iter().filter(|m| m.role == "user").count(),
                id: c.id,
                title: c.title,
                updated: c.updated,
                scope_label: c.scope_label,
            })
        })
        .collect();
    out.sort_by(|a, b| b.updated.cmp(&a.updated));
    Ok(out)
}

#[tauri::command]
pub fn chat_get(state: State<'_, AppState>, id: String) -> CmdResult<Chat> {
    load(&*state.vault()?, &id)
}

#[tauri::command]
pub fn chat_delete(state: State<'_, AppState>, llm: State<'_, LlmState>, id: String) -> CmdResult<()> {
    if !valid_id(&id) {
        return Err(err("invalid", "bad chat id"));
    }
    if llm.runs.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&id) {
        return Err(err("invalid", "Stop the answer first"));
    }
    let p = chats_dir(&*state.vault()?).join(format!("{id}.json"));
    if p.exists() {
        std::fs::remove_file(p).map_err(|e| err("io", e))?;
    }
    Ok(())
}

#[tauri::command]
pub fn chat_cancel(llm: State<'_, LlmState>, id: String) {
    if let Some(c) = llm.runs.lock().unwrap_or_else(|e| e.into_inner()).get(&id) {
        c.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

// ---- notes the model may read -------------------------------------------------

/// Numbering of the notes the model has seen in this chat.
#[derive(Default)]
struct Book {
    notes: Vec<ChatNote>,
}

impl Book {
    fn number(&mut self, s: &NoteSummary) -> (usize, bool) {
        if let Some(n) = self.notes.iter().find(|n| n.id == s.id) {
            return (n.n, false);
        }
        let n = self.notes.len() + 1;
        self.notes.push(ChatNote {
            n,
            id: s.id.clone(),
            title: s.title.clone(),
            day: s.day.clone(),
            source: s.source.as_ref().and_then(|x| x.title.clone()),
        });
        (n, true)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StepEvent {
    chat_id: String,
    step: Step,
    /// The notes in this step, so the UI can show and link them.
    notes: Vec<ChatNote>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaEvent {
    chat_id: String,
    text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoneEvent {
    chat_id: String,
}

/// Note access for one chat turn, bounded by the chat's scope.
struct Notes {
    app: AppHandle,
    chat_id: String,
    vault: Arc<Vault>,
    sem: Arc<Semantic>,
    /// `None`: the whole notebook.
    allowed: Option<HashSet<String>>,
    book: Mutex<Book>,
    steps: Mutex<Vec<Step>>,
}

impl Notes {
    fn allows(&self, id: &str) -> bool {
        self.allowed.as_ref().is_none_or(|a| a.contains(id))
    }

    fn context(&self, s: &NoteSummary) -> Option<ContextNote> {
        let doc = self.vault.get(&s.id).ok()?;
        Some(ContextNote {
            id: s.id.clone(),
            day: s.day.clone(),
            title: doc.title.clone(),
            source: doc.source.as_ref().and_then(|x| x.title.clone().or(x.url.clone())),
            labels: doc.labels.clone(),
            body: doc.content,
        })
    }

    /// Record a step: number the notes, tell the UI, return `(n, note)` pairs.
    fn step(&self, label: String, found: &[NoteSummary]) -> Vec<(usize, ContextNote)> {
        let mut book = self.book.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::new();
        let mut nums = Vec::new();
        for s in found {
            let Some(c) = self.context(s) else { continue };
            let (n, _) = book.number(s);
            nums.push(n);
            out.push((n, c));
        }
        let notes = book.notes.iter().filter(|c| nums.contains(&c.n)).cloned().collect();
        drop(book);
        let step = Step { label, notes: nums };
        self.steps.lock().unwrap_or_else(|e| e.into_inner()).push(step.clone());
        let _ = self.app.emit(
            "chat-step",
            StepEvent {
                chat_id: self.chat_id.clone(),
                step,
                notes,
            },
        );
        out
    }

    fn snippets(found: &[(usize, ContextNote)]) -> String {
        if found.is_empty() {
            return "No notes matched.".into();
        }
        found
            .iter()
            .map(|(n, c)| prompt::snippet(*n, c))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn search(&self, query: &str, limit: usize) -> Vec<NoteSummary> {
        let hits = self.sem.search(query, 40).unwrap_or_default();
        hits.into_iter()
            .map(|h| h.note)
            .filter(|n| self.allows(&n.id))
            .take(limit)
            .collect()
    }
}

fn month_name(m: &str) -> String {
    chrono::NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d")
        .map_or_else(|_| m.to_string(), |d| d.format("%B %Y").to_string())
}

impl NoteTools for Notes {
    fn specs(&self) -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: "search_notes",
                description:
                    "Search the person's notes by meaning and words. Returns numbered notes with a short snippet each.",
                schema: json!({"type": "object", "properties": {"query": {"type": "string", "description": "What to look for"}}, "required": ["query"]}),
            },
            ToolSpec {
                name: "read_note",
                description: "Read one note in full, by the number shown in earlier results.",
                schema: json!({"type": "object", "properties": {"number": {"type": "integer"}}, "required": ["number"]}),
            },
            ToolSpec {
                name: "notes_in_month",
                description: "List the notes written in a calendar month, oldest first.",
                schema: json!({"type": "object", "properties": {"month": {"type": "string", "description": "YYYY-MM"}}, "required": ["month"]}),
            },
            ToolSpec {
                name: "open_questions",
                description: "List questions the person asked themselves in their notes and hasn't returned to.",
                schema: json!({"type": "object", "properties": {}}),
            },
        ]
    }

    fn call(&self, name: &str, args: &Value) -> String {
        match name {
            "search_notes" => {
                let q = args["query"].as_str().unwrap_or("").trim();
                if q.is_empty() {
                    return "Give a query.".into();
                }
                let found = self.search(q, SEARCH_HITS);
                Self::snippets(&self.step(format!("Searched “{q}”"), &found))
            }
            "read_note" => {
                let n = args["number"]
                    .as_u64()
                    .or_else(|| args["number"].as_str().and_then(|s| s.trim().parse().ok()));
                let id = n.and_then(|n| {
                    let book = self.book.lock().unwrap_or_else(|e| e.into_inner());
                    book.notes.iter().find(|c| c.n as u64 == n).map(|c| c.id.clone())
                });
                let Some(s) = id.and_then(|id| self.vault.summary(&id).ok().flatten()) else {
                    return "No note has that number. Use a number from earlier results.".into();
                };
                let found = self.step(format!("Read “{}”", s.title), std::slice::from_ref(&s));
                found
                    .first()
                    .map_or_else(|| "That note no longer exists.".into(), |(n, c)| prompt::full(*n, c))
            }
            "notes_in_month" => {
                let m = args["month"].as_str().unwrap_or("").trim().to_string();
                let Ok(log) = self.vault.month_log(&m) else {
                    return "Use a month like 2026-09.".into();
                };
                let mut notes: Vec<NoteSummary> = log
                    .days
                    .into_iter()
                    .flat_map(|d| d.notes)
                    .filter(|n| self.allows(&n.id))
                    .collect();
                notes.sort_by(|a, b| a.created.cmp(&b.created));
                notes.truncate(LIST_NOTES);
                Self::snippets(&self.step(format!("Looked at {}", month_name(&m)), &notes))
            }
            "open_questions" => {
                let notes: Vec<NoteSummary> = self
                    .sem
                    .ideas_overview()
                    .map(|o| {
                        o.open_questions
                            .into_iter()
                            .map(|q| q.note)
                            .filter(|n| self.allows(&n.id))
                            .take(LIST_NOTES)
                            .collect()
                    })
                    .unwrap_or_default();
                Self::snippets(&self.step("Looked at your open questions".into(), &notes))
            }
            other => format!("Unknown tool {other}."),
        }
    }
}

// ---- sending ------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    chat_id: String,
}

/// Ask a question in a chat (`chat_id: None` starts one). Returns at once;
/// progress arrives as `chat-step` and `chat-delta` events, then `chat-done`
/// after the answer is saved.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn chat_send(
    app: AppHandle,
    state: State<'_, AppState>,
    llm: State<'_, LlmState>,
    chat_id: Option<String>,
    scope: Scope,
    scope_label: String,
    text: String,
    consent: bool,
) -> CmdResult<Sent> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(err("invalid", "Ask something"));
    }
    let s = settings(&app);
    let Some(active) = s.active.clone() else {
        return Err(err("noModel", "Choose a model on the Models page first"));
    };
    let vault = state.vault()?;
    let sem = state.semantic()?;
    let mut chat = match chat_id {
        Some(id) => load(&vault, &id)?,
        None => Chat {
            id: ulid::Ulid::new().to_string().to_lowercase(),
            title: title_from(&text),
            created: now(),
            updated: now(),
            scope,
            scope_label,
            approved: None,
            notes: vec![],
            messages: vec![],
        },
    };
    if llm
        .runs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains_key(&chat.id)
    {
        return Err(err("invalid", "Wait for the answer, or stop it"));
    }

    // Remote: the user approves this chat's scope and destination once.
    let remote = s.active_remote().cloned();
    if let Some(r) = &remote {
        let dest = r.destination();
        if chat.approved.as_deref() != Some(dest.as_str()) {
            if !consent {
                return Err(err("consent", "Approve sending notes from this chat first"));
            }
            chat.approved = Some(dest);
            update_settings(&app, |s| s.record_consent(r));
        }
    }

    // Earlier exchanges (text only).
    let history: Vec<Turn> = chat
        .messages
        .chunks(2)
        .filter_map(|p| match p {
            [u, a] if u.role == "user" && a.role == "assistant" && !a.text.trim().is_empty() => Some(Turn {
                user: u.text.clone(),
                assistant: a.text.clone(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .take(HISTORY_TURNS)
        .rev()
        .collect();
    let first_turn = history.is_empty();
    let prev_question = chat
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.text.clone());

    chat.messages.push(Message {
        role: "user".into(),
        text: text.clone(),
        steps: vec![],
        model: None,
        error: None,
        at: now(),
    });
    chat.updated = now();
    save(&vault, &chat)?;

    let allowed = match &chat.scope {
        Scope::Notebook => None,
        scope => Some(
            sem.context(scope, "", 500)?
                .into_iter()
                .map(|c| c.note.id)
                .collect::<HashSet<_>>(),
        ),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    llm.runs
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(chat.id.clone(), cancel.clone());
    let notes = Notes {
        app: app.clone(),
        chat_id: chat.id.clone(),
        vault: vault.clone(),
        sem: sem.clone(),
        allowed,
        book: Mutex::new(Book {
            notes: chat.notes.clone(),
        }),
        steps: Mutex::new(vec![]),
    };
    let key = match &remote {
        Some(r) => llm.keys.get(&r.id)?,
        None => None,
    };
    let local = llm.local.clone();
    let dir = models_dir(&app)?;
    let model_label = active_label(&s);
    let chat_id = chat.id.clone();

    tauri::async_runtime::spawn(async move {
        let mut answer = String::new();
        let mut on_text = |t: &str| {
            answer.push_str(t);
            let _ = app.emit(
                "chat-delta",
                DeltaEvent {
                    chat_id: chat.id.clone(),
                    text: t.to_string(),
                },
            );
        };

        // A narrow scope's notes are known up front: show them to the model on the first turn.
        let seed = if first_turn && !matches!(chat.scope, Scope::Notebook) {
            let found = sem
                .context(&chat.scope, &text, 8)
                .unwrap_or_default()
                .into_iter()
                .map(|c| c.note)
                .collect::<Vec<_>>();
            Some(found)
        } else {
            None
        };

        // The app picks this turn's notes (local models, or providers without tools).
        let fallback = |notes: &Notes| -> ChatRequest {
            let query = match (&prev_question, text.split_whitespace().count() < 6) {
                (Some(p), true) => format!("{p} {text}"),
                _ => text.clone(),
            };
            let found: Vec<NoteSummary> = match &chat.scope {
                Scope::Notebook => notes.search(&query, TURN_NOTES),
                scope => sem
                    .context(scope, &query, TURN_NOTES)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| c.note)
                    .collect(),
            };
            let label = match (&chat.scope, found.len()) {
                (_, 0) => "Found no matching notes".to_string(),
                (Scope::Notebook, k) => format!("Found {k} {}", if k == 1 { "note" } else { "notes" }),
                (_, k) => format!("Read {k} from {}", chat.scope_label),
            };
            let picked = notes.step(label, &found);
            ChatRequest {
                system: prompt::system(false),
                history: history.clone(),
                user: prompt::with_notes(&text, &picked),
                max_tokens: MAX_TOKENS,
                temperature: 0.2,
            }
        };

        let result = match (&active, &remote) {
            (Active::Local { model }, _) => match catalog::find(model) {
                Some(m) => {
                    let req = fallback(&notes);
                    local.chat(&dir, m, &req, &cancel, &mut on_text).await
                }
                None => Err(noughtpad_llm::Error::Unavailable("unknown model".into())),
            },
            (Active::Remote { .. }, Some(r)) => {
                let mut user = text.clone();
                if let Some(found) = &seed {
                    let picked = notes.step(format!("Gathered {}", chat.scope_label), found);
                    user = format!(
                        "These notes are the scope of this chat ({}):\n\n{}\n\nQuestion: {text}",
                        chat.scope_label,
                        Notes::snippets(&picked)
                    );
                }
                let req = ChatRequest {
                    system: prompt::system(true),
                    history: history.clone(),
                    user,
                    max_tokens: MAX_TOKENS,
                    temperature: 0.2,
                };
                match noughtpad_llm::remote::chat_with_tools(
                    r,
                    key.clone(),
                    &req,
                    &notes,
                    MAX_TOOL_ROUNDS,
                    &cancel,
                    &mut on_text,
                )
                .await
                {
                    Err(noughtpad_llm::Error::ToolsUnsupported(m)) => {
                        tracing::info!("{} can't use tools ({m}); choosing notes instead", r.model);
                        let req = fallback(&notes);
                        noughtpad_llm::remote::chat(r, key, &req, &cancel, &mut on_text).await
                    }
                    other => other,
                }
            }
            (Active::Remote { .. }, None) => Err(noughtpad_llm::Error::Unavailable("provider removed".into())),
        };

        let error = match result {
            Ok(_) | Err(noughtpad_llm::Error::Cancelled) => None,
            Err(e) => {
                tracing::warn!("chat failed: {e}");
                Some(e.to_string())
            }
        };
        let steps = std::mem::take(&mut *notes.steps.lock().unwrap_or_else(|e| e.into_inner()));
        chat.notes = std::mem::take(&mut notes.book.lock().unwrap_or_else(|e| e.into_inner()).notes);
        chat.messages.push(Message {
            role: "assistant".into(),
            text: answer.trim().to_string(),
            steps,
            model: model_label,
            error,
            at: now(),
        });
        chat.updated = now();
        if let Err(e) = save(&vault, &chat) {
            tracing::warn!("could not save chat: {}", e.message);
        }
        app.state::<LlmState>()
            .runs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&chat.id);
        let _ = app.emit(
            "chat-done",
            DoneEvent {
                chat_id: chat.id.clone(),
            },
        );
    });
    Ok(Sent { chat_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_ids() {
        assert_eq!(
            title_from("  How has my\nthinking changed? "),
            "How has my thinking changed?"
        );
        assert!(title_from(&"word ".repeat(40)).ends_with('…'));
        assert!(valid_id("01abc"));
        assert!(!valid_id("../x") && !valid_id(""));
    }

    #[test]
    fn book_numbers_once() {
        let s = |id: &str| NoteSummary {
            id: id.into(),
            path: String::new(),
            title: id.into(),
            has_title: false,
            excerpt: String::new(),
            created: String::new(),
            day: "2026-01-01".into(),
            kind: noughtpad_core::NoteKind::Personal,
            labels: vec![],
            source: None,
        };
        let mut b = Book::default();
        assert_eq!(b.number(&s("a")), (1, true));
        assert_eq!(b.number(&s("b")), (2, true));
        assert_eq!(b.number(&s("a")), (1, false));
    }

    #[test]
    fn chat_file_roundtrip() {
        let c = Chat {
            id: "x1".into(),
            title: "t".into(),
            created: "c".into(),
            updated: "u".into(),
            scope: Scope::Month {
                month: "2026-09".into(),
            },
            scope_label: "September".into(),
            approved: None,
            notes: vec![],
            messages: vec![],
        };
        let back: Chat = serde_json::from_str(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }
}
