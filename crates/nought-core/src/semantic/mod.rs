//! Quiet retrieval (M2): local embeddings, related notes and hybrid search.
//!
//! * Notes are chunked and embedded in a background thread, only after a
//!   note has been still for a few seconds, newest notes first.
//! * Vectors are cached by chunk-text hash in the (disposable) index file.
//! * Related notes and meaning-based search read an in-memory flat index.
//! * Nothing here writes to note files.

pub mod chunk;
pub mod context;
pub mod embedder;
pub mod feedback;
pub mod ideas;
pub mod model;
pub(crate) mod store;
mod vectors;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::index::{parse_query, MatchKind, NoteSummary, SearchHit};
use crate::userstate::UserState;
use crate::vault::hash;
use crate::{Error, Result, Vault};
use embedder::{dot, Embedder, ModelSpec, TextKind};
use vectors::VectorIndex;

/// Don't embed a note until it has been unchanged this long (ms).
const QUIET_MS: i64 = 3_000;
/// Notes embedded per worker iteration.
const BATCH: usize = 16;
/// Reciprocal-rank-fusion constant.
const RRF_K: f32 = 60.0;
const PASSAGE_CHARS: usize = 280;
/// How far below `min_related` candidates may fall when the learned model re-checks them.
const LEARNED_WIDEN: f32 = 0.08;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Status {
    /// Semantic features are not enabled.
    Off,
    Downloading {
        received: u64,
        total: u64,
    },
    Loading,
    Indexing {
        done: i64,
        total: i64,
    },
    Ready {
        notes: i64,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Strength {
    Strong,
    Possible,
}

/// One related note and the evidence for showing it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Related {
    pub note: NoteSummary,
    pub score: f32,
    pub strength: Strength,
    /// The passage in the related note that matched best.
    pub passage: String,
    /// The passage in the current note it matched, when the note has several.
    pub source_passage: Option<String>,
    pub shared_labels: Vec<String>,
    pub same_source: Option<String>,
    /// Days from the current note to the related one (negative = earlier).
    pub days_apart: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RelatedNotes {
    /// False when semantic features are off.
    pub enabled: bool,
    /// True when this note hasn't been embedded yet (e.g. just edited).
    pub pending: bool,
    pub items: Vec<Related>,
    /// True when suggestions are tuned by this user's own feedback.
    pub personalized: bool,
}

type Listener = Box<dyn Fn(&Status) + Send + Sync>;

pub struct Semantic {
    vault: Arc<Vault>,
    user: Arc<UserState>,
    embedder: Mutex<Option<Box<dyn Embedder>>>,
    spec: RwLock<Option<ModelSpec>>,
    vectors: RwLock<VectorIndex>,
    status: Mutex<Status>,
    listener: Mutex<Option<Listener>>,
    wake: (Mutex<bool>, Condvar),
    stop: AtomicBool,
    /// Bumped whenever vectors change; the ideas model records the version it saw.
    version: AtomicU64,
    ideas: RwLock<Option<Arc<ideas::Model>>>,
    ideas_listener: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
    /// Learned relatedness model, keyed by the feedback count it was fitted on.
    learned: Mutex<Option<(usize, Option<feedback::Model>)>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Semantic {
    pub fn new(vault: Arc<Vault>, user: Arc<UserState>) -> Result<Arc<Self>> {
        vault.with_index(|ix| store::init(ix.conn()))?;
        Ok(Arc::new(Semantic {
            vault,
            user,
            embedder: Mutex::new(None),
            spec: RwLock::new(None),
            vectors: RwLock::new(VectorIndex::default()),
            status: Mutex::new(Status::Off),
            listener: Mutex::new(None),
            wake: (Mutex::new(false), Condvar::new()),
            stop: AtomicBool::new(false),
            version: AtomicU64::new(1),
            ideas: RwLock::new(None),
            ideas_listener: Mutex::new(None),
            learned: Mutex::new(None),
        }))
    }

    /// Called whenever the status changes.
    pub fn on_status(&self, f: impl Fn(&Status) + Send + Sync + 'static) {
        *lock(&self.listener) = Some(Box::new(f));
    }

    /// Called after the ideas model (themes, timelines) is recomputed.
    pub fn on_ideas(&self, f: impl Fn() + Send + Sync + 'static) {
        *lock(&self.ideas_listener) = Some(Box::new(f));
    }

    pub fn status(&self) -> Status {
        lock(&self.status).clone()
    }

    pub fn set_status(&self, s: Status) {
        let mut cur = lock(&self.status);
        if *cur == s {
            return;
        }
        *cur = s.clone();
        drop(cur);
        if !matches!(s, Status::Downloading { .. } | Status::Indexing { .. }) {
            tracing::info!(status = ?s, "semantic status");
        }
        if let Some(f) = lock(&self.listener).as_ref() {
            f(&s);
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.spec.read().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    /// Activate a model: load its cached vectors and start (re)indexing.
    pub fn set_embedder(&self, e: Box<dyn Embedder>) -> Result<()> {
        let spec = e.spec().clone();
        let rows = self.vault.with_index(|ix| store::load_all(ix.conn(), spec.id))?;
        *self.vectors.write().unwrap_or_else(|e| e.into_inner()) = VectorIndex::from_rows(rows);
        *lock(&self.embedder) = Some(e);
        *self.spec.write().unwrap_or_else(|e| e.into_inner()) = Some(spec);
        self.version.fetch_add(1, Ordering::SeqCst);
        self.update_progress();
        self.nudge();
        Ok(())
    }

    /// Turn semantic features off (cached vectors stay on disk for next time).
    pub fn disable(&self) {
        *self.spec.write().unwrap_or_else(|e| e.into_inner()) = None;
        *lock(&self.embedder) = None;
        *self.vectors.write().unwrap_or_else(|e| e.into_inner()) = VectorIndex::default();
        *self.ideas.write().unwrap_or_else(|e| e.into_inner()) = None;
        self.set_status(Status::Off);
    }

    /// Wake the worker early (e.g. after a file change).
    pub fn nudge(&self) {
        let (m, cv) = &self.wake;
        *lock(m) = true;
        cv.notify_all();
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.nudge();
    }

    fn wait(&self, d: Duration) {
        let (m, cv) = &self.wake;
        let mut woken = lock(m);
        if !*woken {
            woken = cv
                .wait_timeout(woken, d)
                .map(|(g, _)| g)
                .unwrap_or_else(|e| e.into_inner().0);
        }
        *woken = false;
    }

    // ---- background indexing --------------------------------------------

    pub fn spawn_worker(self: &Arc<Self>) -> std::thread::JoinHandle<()> {
        let me = self.clone();
        std::thread::Builder::new()
            .name("nought-semantic".into())
            .spawn(move || me.run())
            .expect("spawn semantic worker")
    }

    fn run(&self) {
        let mut last_sweep: Option<Instant> = None;
        let mut last_ideas: Option<Instant> = None;
        while !self.stop.load(Ordering::SeqCst) {
            let Some(spec) = self.spec.read().unwrap_or_else(|e| e.into_inner()).clone() else {
                self.wait(Duration::from_secs(5));
                continue;
            };
            if last_sweep.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
                match self.vault.with_index(|ix| store::sweep(ix.conn())) {
                    Ok(gone) => {
                        let mut v = self.vectors.write().unwrap_or_else(|e| e.into_inner());
                        gone.iter().for_each(|id| v.remove(id));
                        if !gone.is_empty() {
                            self.version.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(e) => tracing::warn!("semantic sweep failed: {e}"),
                }
                last_sweep = Some(Instant::now());
            }
            match self.step(&spec) {
                Ok(0) => {
                    self.update_progress();
                    // Themes are recomputed when indexing is idle and something
                    // changed, at most once a minute.
                    if self.ideas_stale() && last_ideas.is_none_or(|t| t.elapsed() > Duration::from_secs(60)) {
                        match self.refresh_ideas() {
                            Ok(()) => {}
                            Err(e) => tracing::warn!("computing themes failed: {e}"),
                        }
                        last_ideas = Some(Instant::now());
                    }
                    self.wait(Duration::from_secs(2));
                }
                Ok(_) => {
                    self.update_progress();
                    // Yield between batches so typing and searching stay smooth.
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(e) => {
                    tracing::error!("semantic indexing failed: {e}");
                    self.set_status(Status::Error { message: e.to_string() });
                    self.wait(Duration::from_secs(30));
                }
            }
        }
    }

    /// Index everything pending right away, ignoring the quiet period
    /// (for tools, benchmarks and tests; the app uses the background worker).
    pub fn index_now(&self) -> Result<()> {
        let Some(spec) = self.spec.read().unwrap_or_else(|e| e.into_inner()).clone() else {
            return Ok(());
        };
        loop {
            let pending = self
                .vault
                .with_index(|ix| store::pending(ix.conn(), spec.id, i64::MAX, BATCH))?;
            if pending.is_empty() {
                break;
            }
            for n in &pending {
                self.embed_note(&spec, n)?;
            }
        }
        self.update_progress();
        Ok(())
    }

    /// Embed one batch of pending notes. Returns how many were processed.
    pub fn step(&self, spec: &ModelSpec) -> Result<usize> {
        let quiet = chrono::Utc::now().timestamp_millis() - QUIET_MS;
        let pending = self
            .vault
            .with_index(|ix| store::pending(ix.conn(), spec.id, quiet, BATCH))?;
        for n in &pending {
            if self.stop.load(Ordering::SeqCst) {
                break;
            }
            self.embed_note(spec, n)?;
        }
        Ok(pending.len())
    }

    fn embed_note(&self, spec: &ModelSpec, n: &store::PendingNote) -> Result<()> {
        let chunks = chunk::chunk(n.title.as_deref(), &n.body);
        let hashes: Vec<String> = chunks.iter().map(|c| hash(&c.text)).collect();
        let mut vecs: Vec<Option<Vec<f32>>> = self.vault.with_index(|ix| {
            hashes
                .iter()
                .map(|h| store::cached(ix.conn(), spec.id, h))
                .collect::<Result<_>>()
        })?;
        let missing: Vec<usize> = (0..chunks.len()).filter(|&i| vecs[i].is_none()).collect();
        if !missing.is_empty() {
            let texts: Vec<String> = missing.iter().map(|&i| chunks[i].text.clone()).collect();
            let out = {
                let mut guard = lock(&self.embedder);
                let e = guard
                    .as_mut()
                    .ok_or_else(|| Error::Embedding("model unloaded".into()))?;
                e.embed(&texts, TextKind::Passage)?
            };
            for (&i, v) in missing.iter().zip(out) {
                vecs[i] = Some(v);
            }
        }
        let rows: Vec<_> = chunks
            .into_iter()
            .zip(hashes)
            .zip(vecs)
            .map(|((c, h), v)| (c, h, v.unwrap_or_default()))
            .collect();
        self.vault
            .with_index(|ix| store::save(ix.conn(), spec.id, &n.id, &n.hash, &rows))?;
        let mem: Vec<(u32, Vec<f32>)> = rows.into_iter().map(|(c, _, v)| (c.ord, v)).collect();
        self.vectors
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .set(n.id.clone(), mem);
        self.version.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn update_progress(&self) {
        let Some(spec) = self.spec.read().unwrap_or_else(|e| e.into_inner()).clone() else {
            return;
        };
        if let Ok((done, total)) = self.vault.with_index(|ix| store::progress(ix.conn(), spec.id)) {
            self.set_status(if done >= total {
                Status::Ready { notes: done }
            } else {
                Status::Indexing { done, total }
            });
        }
    }

    // ---- related notes ----------------------------------------------------

    pub fn related(&self, id: &str, limit: usize) -> Result<RelatedNotes> {
        let Some(spec) = self.spec.read().unwrap_or_else(|e| e.into_inner()).clone() else {
            return Ok(RelatedNotes {
                enabled: false,
                pending: false,
                items: vec![],
                personalized: false,
            });
        };
        let dismissed = self.user.dismissed_for(id)?;
        let learned = self.learned_model(&spec)?;
        // With a learned model, the embeddings cast a slightly wider net and the
        // model decides what is shown.
        let floor = if learned.is_some() {
            spec.min_related - LEARNED_WIDEN
        } else {
            spec.min_related
        };

        // Score under the read lock; fetch summaries and text afterwards.
        struct Cand {
            id: String,
            score: f32,
            mean_sim: f32,
            chunk_sim: f32,
            q_ord: u32,
            c_ord: u32,
        }
        let (cands, source_chunks) = {
            let vectors = self.vectors.read().unwrap_or_else(|e| e.into_inner());
            let Some(q) = vectors.notes.get(id) else {
                return Ok(RelatedNotes {
                    enabled: true,
                    pending: true,
                    items: vec![],
                    personalized: false,
                });
            };
            let sims: Vec<(&String, f32)> = vectors
                .notes
                .iter()
                .filter(|(other, _)| other.as_str() != id && !dismissed.contains(other.as_str()))
                .map(|(other, nv)| (other, dot(&q.mean, &nv.mean)))
                .collect();
            let n = sims.len() as f32;
            let mean = sims.iter().map(|s| s.1).sum::<f32>() / n.max(1.0);
            let sd = (sims.iter().map(|s| (s.1 - mean).powi(2)).sum::<f32>() / n.max(1.0)).sqrt();
            let mut top = sims;
            top.sort_by(|a, b| b.1.total_cmp(&a.1));
            top.truncate(40);
            let cands: Vec<Cand> = top
                .into_iter()
                .filter_map(|(other, mean_sim)| {
                    // Relative to how similar this note is to everything else:
                    // filters out "everything looks alike" notes.
                    if n >= 20.0 && sd > 0.0 && (mean_sim - mean) / sd < 1.0 {
                        return None;
                    }
                    let nv = &vectors.notes[other];
                    let (q_ord, c_ord, chunk_sim) = q
                        .chunks
                        .iter()
                        .flat_map(|(qo, qv)| nv.chunks.iter().map(move |(co, cv)| (*qo, *co, dot(qv, cv))))
                        .max_by(|a, b| a.2.total_cmp(&b.2))?;
                    let score = 0.5 * mean_sim + 0.5 * chunk_sim;
                    (score >= floor).then(|| Cand {
                        id: other.clone(),
                        score,
                        mean_sim,
                        chunk_sim,
                        q_ord,
                        c_ord,
                    })
                })
                .collect();
            (cands, q.chunks.len())
        };

        let Some(me) = self.vault.summary(id)? else {
            return Ok(RelatedNotes {
                enabled: true,
                pending: false,
                items: vec![],
                personalized: false,
            });
        };
        let my_labels: HashSet<String> = me.labels.iter().map(|l| l.to_lowercase()).collect();
        let my_source = me.source.as_ref().and_then(|s| s.title.clone());
        let my_day = day_number(&me.day);

        let mut items = Vec::new();
        for c in cands {
            let Some(note) = self.vault.summary(&c.id)? else {
                continue;
            };
            let shared_labels: Vec<String> = note
                .labels
                .iter()
                .filter(|l| my_labels.contains(&l.to_lowercase()))
                .cloned()
                .collect();
            let same_source = match (&my_source, note.source.as_ref().and_then(|s| s.title.as_ref())) {
                (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => Some(b.clone()),
                _ => None,
            };
            let boost = 0.02 * shared_labels.len().min(3) as f32 + if same_source.is_some() { 0.03 } else { 0.0 };
            let mut score = c.score + boost;
            if let Some(m) = &learned {
                let p = m.probability(&pair_features(c.mean_sim, c.chunk_sim, &me, &note));
                if p < 0.5 {
                    continue;
                }
                score = p;
            }
            let (passage, mine) = self.vault.with_index(|ix| -> Result<_> {
                let p = store::chunk_text(ix.conn(), &c.id, c.c_ord)?.unwrap_or_default();
                let sp = store::chunk_text(ix.conn(), id, c.q_ord)?.unwrap_or_default();
                Ok((p, sp))
            })?;
            let source_passage = (source_chunks > 1).then(|| mine.clone());
            items.push(Related {
                days_apart: day_number(&note.day) - my_day,
                strength: if c.score >= spec.strong_related {
                    Strength::Strong
                } else {
                    Strength::Possible
                },
                score,
                passage: clip(without_title(&passage, &note), PASSAGE_CHARS),
                source_passage: source_passage.map(|s| clip(&s, PASSAGE_CHARS)),
                shared_labels,
                same_source,
                note,
            });
        }
        items.sort_by(|a, b| b.score.total_cmp(&a.score));
        items.truncate(limit);
        Ok(RelatedNotes {
            enabled: true,
            pending: false,
            items,
            personalized: learned.is_some(),
        })
    }

    // ---- learning from feedback --------------------------------------------

    /// The relatedness model fitted on this user's feedback, if there is enough.
    fn learned_model(&self, spec: &ModelSpec) -> Result<Option<feedback::Model>> {
        let examples: Vec<(feedback::PairFeatures, bool)> = self
            .user
            .feedback(spec.id)?
            .into_iter()
            .filter_map(|(json, useful)| serde_json::from_str(&json).ok().map(|f| (f, useful)))
            .collect();
        let mut cache = lock(&self.learned);
        if let Some((n, m)) = cache.as_ref() {
            if *n == examples.len() {
                return Ok(*m);
            }
        }
        let m = feedback::Model::fit(feedback::Model::prior(spec.min_related), &examples);
        *cache = Some((examples.len(), m));
        Ok(m)
    }

    /// Features of a pair right now (needs both notes embedded).
    fn features_now(&self, a: &str, b: &str) -> Result<Option<feedback::PairFeatures>> {
        let sims = {
            let v = self.vectors.read().unwrap_or_else(|e| e.into_inner());
            match (v.notes.get(a), v.notes.get(b)) {
                (Some(x), Some(y)) => {
                    let best = x
                        .chunks
                        .iter()
                        .flat_map(|(_, xv)| y.chunks.iter().map(move |(_, yv)| dot(xv, yv)))
                        .fold(f32::MIN, f32::max);
                    Some((dot(&x.mean, &y.mean), best))
                }
                _ => None,
            }
        };
        let (Some((mean_sim, chunk_sim)), Some(me), Some(other)) =
            (sims, self.vault.summary(a)?, self.vault.summary(b)?)
        else {
            return Ok(None);
        };
        Ok(Some(pair_features(mean_sim, chunk_sim, &me, &other)))
    }

    fn record(&self, a: &str, b: &str, useful: bool) -> Result<()> {
        let Some(spec) = self.spec.read().unwrap_or_else(|e| e.into_inner()).clone() else {
            return Ok(());
        };
        if let Some(f) = self.features_now(a, b)? {
            let json = serde_json::to_string(&f).unwrap_or_default();
            self.user.record_feedback(a, b, useful, spec.id, &json)?;
        }
        Ok(())
    }

    /// The user opened suggestion `b` from note `a`'s related notes: useful.
    pub fn opened_related(&self, a: &str, b: &str) -> Result<()> {
        self.record(a, b, true)
    }

    /// Forget everything learned from related-note feedback (dismissals stay).
    pub fn reset_feedback(&self) -> Result<()> {
        self.user.clear_feedback()?;
        *lock(&self.learned) = None;
        Ok(())
    }

    // ---- ideas over time (M3) ----------------------------------------------

    fn ideas_stale(&self) -> bool {
        let current = self.version.load(Ordering::SeqCst);
        self.ideas
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_none_or(|m| m.version != current)
    }

    /// Recompute themes and temporal links from the current vectors.
    pub fn refresh_ideas(&self) -> Result<()> {
        self.refresh_ideas_at(chrono::Local::now().date_naive())
    }

    pub fn refresh_ideas_at(&self, today: chrono::NaiveDate) -> Result<()> {
        let Some(spec) = self.spec.read().unwrap_or_else(|e| e.into_inner()).clone() else {
            return Ok(());
        };
        let version = self.version.load(Ordering::SeqCst);
        let started = Instant::now();
        let (rows, previous) = self
            .vault
            .with_index(|ix| -> Result<_> { Ok((store::note_rows(ix.conn())?, store::load_themes(ix.conn())?)) })?;
        let (rows, means): (Vec<_>, Vec<_>) = {
            let v = self.vectors.read().unwrap_or_else(|e| e.into_inner());
            rows.into_iter()
                .filter_map(|r| v.notes.get(&r.id).map(|nv| nv.mean.clone()).map(|m| (r, m)))
                .unzip()
        };
        let model = ideas::compute(
            rows,
            &means,
            &previous,
            &ideas::Thresholds {
                related: spec.min_related,
                strong: spec.strong_related,
            },
            today,
            version,
        );
        let save: Vec<(String, Vec<String>)> = model
            .themes
            .iter()
            .map(|t| {
                (
                    t.id.clone(),
                    t.members.iter().map(|&m| model.notes[m].id.clone()).collect(),
                )
            })
            .collect();
        self.vault.with_index(|ix| store::save_themes(ix.conn(), &save))?;
        tracing::info!(
            notes = model.notes.len(),
            themes = model.themes.len(),
            ms = started.elapsed().as_millis() as u64,
            "themes computed"
        );
        *self.ideas.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(model));
        if let Some(f) = lock(&self.ideas_listener).as_ref() {
            f();
        }
        Ok(())
    }

    fn ideas_model(&self) -> Result<Option<Arc<ideas::Model>>> {
        if !self.is_enabled() {
            return Ok(None);
        }
        let existing = self.ideas.read().unwrap_or_else(|e| e.into_inner()).clone();
        if existing.is_none() {
            self.refresh_ideas()?;
            return Ok(self.ideas.read().unwrap_or_else(|e| e.into_inner()).clone());
        }
        Ok(existing)
    }

    fn summaries(&self, model: &ideas::Model, idx: &[usize]) -> Result<Vec<NoteSummary>> {
        idx.iter()
            .filter_map(|&i| self.vault.summary(&model.notes[i].id).transpose())
            .collect()
    }

    pub fn ideas_overview(&self) -> Result<ideas::IdeasOverview> {
        let empty = |enabled| ideas::IdeasOverview {
            enabled,
            stale: false,
            returning: vec![],
            forming: vec![],
            dormant: vec![],
            other: vec![],
            influences: vec![],
            open_questions: vec![],
        };
        let Some(model) = self.ideas_model()? else {
            return Ok(empty(false));
        };
        let sections = model.sections();
        let to = |v: &[usize]| v.iter().map(|&ti| model.summary(ti)).collect::<Vec<_>>();
        let theme_of = |i: usize| model.note_theme.get(&model.notes[i].id).map(|&ti| &model.themes[ti]);
        let mut influences = Vec::new();
        for (src, later) in model.influences().into_iter().take(6) {
            let Some(source) = self.vault.summary(&model.notes[src].id)? else {
                continue;
            };
            influences.push(ideas::Influence {
                source,
                later: self.summaries(&model, &later)?,
                theme_id: theme_of(src).map(|t| t.id.clone()),
                theme_name: theme_of(src).map(|t| t.name.clone()),
            });
        }
        let mut open_questions = Vec::new();
        for &q in model.open_questions.iter().take(8) {
            let Some(note) = self.vault.summary(&model.notes[q].id)? else {
                continue;
            };
            open_questions.push(ideas::OpenQuestion {
                note,
                theme_id: theme_of(q).map(|t| t.id.clone()),
                theme_name: theme_of(q).map(|t| t.name.clone()),
            });
        }
        Ok(ideas::IdeasOverview {
            enabled: true,
            stale: model.version != self.version.load(Ordering::SeqCst),
            returning: to(&sections.returning),
            forming: to(&sections.forming),
            dormant: to(&sections.dormant),
            other: to(&sections.other),
            influences,
            open_questions,
        })
    }

    pub fn theme_timeline(&self, theme_id: &str) -> Result<Option<ideas::ThemeTimeline>> {
        let Some(model) = self.ideas_model()? else {
            return Ok(None);
        };
        let Some(ti) = model.themes.iter().position(|t| t.id == theme_id) else {
            return Ok(None);
        };
        let summary_of = |i: usize| self.vault.summary(&model.notes[i].id);
        let mut entries = Vec::new();
        for &m in &model.themes[ti].members {
            let Some(note) = summary_of(m)? else { continue };
            entries.push(ideas::TimelineEntry {
                note,
                influenced_by: match model.influenced_by.get(&m) {
                    Some(l) => summary_of(l.to)?,
                    None => None,
                },
                revisits: match model.revisits.get(&m) {
                    Some(l) => summary_of(l.to)?,
                    None => None,
                },
                may_revise: model.may_revise.contains(&m),
                is_question: model.questions.contains(&m),
            });
        }
        Ok(Some(ideas::ThemeTimeline {
            theme: model.summary(ti),
            entries,
        }))
    }

    /// The theme a note belongs to, if any.
    pub fn note_theme(&self, note_id: &str) -> Result<Option<ideas::ThemeSummary>> {
        let model = self.ideas.read().unwrap_or_else(|e| e.into_inner()).clone();
        Ok(model.and_then(|m| m.note_theme.get(note_id).map(|&ti| m.summary(ti))))
    }

    /// "Not related": hide the pair for good, and learn from it.
    pub fn dismiss(&self, a: &str, b: &str) -> Result<()> {
        self.record(a, b, false)?;
        self.user.dismiss(a, b)
    }

    pub fn undismiss(&self, a: &str, b: &str) -> Result<()> {
        self.user.remove_feedback(a, b, false)?;
        self.user.undismiss(a, b)
    }

    // ---- hybrid search ----------------------------------------------------

    /// Full-text search fused with meaning-based search (reciprocal rank
    /// fusion). Falls back to full-text only when no model is active.
    pub fn search(&self, input: &str, limit: u32) -> Result<Vec<SearchHit>> {
        let parsed = parse_query(input);
        let mut text_hits = self.vault.search(input, limit.max(50))?;
        let spec = self.spec.read().unwrap_or_else(|e| e.into_inner()).clone();
        let Some(spec) = spec.filter(|_| parsed.text.trim().chars().count() >= 3) else {
            text_hits.truncate(limit as usize);
            return Ok(text_hits);
        };

        let qv = {
            let mut guard = lock(&self.embedder);
            match guard.as_mut() {
                Some(e) => e.embed(std::slice::from_ref(&parsed.text), TextKind::Query)?.pop(),
                None => None,
            }
        };
        let meaning: Vec<(String, u32, f32)> = match qv {
            Some(qv) => self
                .vectors
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .search(&qv, 50)
                .into_iter()
                .filter(|h| h.2 >= spec.min_search)
                .collect(),
            None => vec![],
        };

        let mut fused: HashMap<String, f32> = HashMap::new();
        for (rank, h) in text_hits.iter().enumerate() {
            *fused.entry(h.note.id.clone()).or_default() += 1.0 / (RRF_K + rank as f32 + 1.0);
        }
        let mut meaning_ord: HashMap<String, u32> = HashMap::new();
        for (rank, (id, ord, _)) in meaning.iter().enumerate() {
            *fused.entry(id.clone()).or_default() += 1.0 / (RRF_K + rank as f32 + 1.0);
            meaning_ord.insert(id.clone(), *ord);
        }

        let mut by_id: HashMap<String, SearchHit> = text_hits.into_iter().map(|h| (h.note.id.clone(), h)).collect();
        let mut out = Vec::new();
        for (id, score) in fused {
            let hit = match by_id.remove(&id) {
                Some(mut h) => {
                    if meaning_ord.contains_key(&id) {
                        h.matched = MatchKind::Both;
                    }
                    h
                }
                None => {
                    let Some(note) = self.vault.summary(&id)? else { continue };
                    if parsed
                        .label
                        .as_ref()
                        .is_some_and(|l| !note.labels.iter().any(|x| x.eq_ignore_ascii_case(l)))
                        || parsed.kind.is_some_and(|k| note.kind != k)
                    {
                        continue;
                    }
                    let ord = meaning_ord[&id];
                    let passage = self
                        .vault
                        .with_index(|ix| store::chunk_text(ix.conn(), &id, ord))?
                        .unwrap_or_else(|| note.excerpt.clone());
                    SearchHit {
                        snippet: clip(without_title(&passage, &note), 240),
                        note,
                        score: 0.0,
                        matched: MatchKind::Meaning,
                    }
                }
            };
            out.push(SearchHit {
                score: score as f64,
                ..hit
            });
        }
        out.sort_by(|a, b| b.score.total_cmp(&a.score));
        out.truncate(limit as usize);
        Ok(out)
    }
}

impl Drop for Semantic {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn pair_features(mean_sim: f32, chunk_sim: f32, a: &NoteSummary, b: &NoteSummary) -> feedback::PairFeatures {
    let la: HashSet<String> = a.labels.iter().map(|l| l.to_lowercase()).collect();
    let lb: HashSet<String> = b.labels.iter().map(|l| l.to_lowercase()).collect();
    let shared = la.intersection(&lb).count();
    let same_source = match (
        a.source.as_ref().and_then(|s| s.title.as_ref()),
        b.source.as_ref().and_then(|s| s.title.as_ref()),
    ) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
        _ => false,
    };
    use crate::note::NoteKind;
    feedback::PairFeatures {
        note_sim: mean_sim,
        passage_sim: chunk_sim,
        shared_labels: (shared.min(3) as f32) / 3.0,
        same_source: same_source as u8 as f32,
        time_apart: feedback::PairFeatures::time_feature(day_number(&b.day) - day_number(&a.day)),
        both_personal: (a.kind == NoteKind::Personal && b.kind == NoteKind::Personal) as u8 as f32,
        label_mismatch: (!la.is_empty() && !lb.is_empty() && shared == 0) as u8 as f32,
    }
}

fn day_number(day: &str) -> i64 {
    chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
        .map(|d| d.num_days_from_ce() as i64)
        .unwrap_or(0)
}

use chrono::Datelike;

/// Chunk 0 of a titled note starts with the title (for embedding context);
/// drop it when showing the passage, since the title is displayed separately.
fn without_title<'a>(passage: &'a str, note: &NoteSummary) -> &'a str {
    if note.has_title {
        if let Some(rest) = passage.strip_prefix(note.title.as_str()) {
            return rest.trim_start();
        }
    }
    passage
}

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max).collect();
    // Prefer to end on a word boundary.
    let cut = match cut.rfind(char::is_whitespace) {
        Some(i) if i > max / 2 => cut[..i].to_string(),
        _ => cut,
    };
    format!("{}…", cut.trim_end())
}

#[cfg(test)]
mod tests {
    use super::embedder::HashEmbedder;
    use super::*;
    use crate::vault::{EditableMeta, NewNote};

    fn setup() -> (tempfile::TempDir, Arc<Vault>, Arc<Semantic>) {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Vault::open(dir.path()).unwrap());
        let user = Arc::new(UserState::open_in_memory().unwrap());
        let sem = Semantic::new(vault.clone(), user).unwrap();
        (dir, vault, sem)
    }

    fn add(v: &Vault, text: &str, labels: &[&str]) -> String {
        v.create(NewNote {
            content: text.into(),
            meta: EditableMeta {
                labels: labels.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            },
        })
        .unwrap()
        .id
    }

    /// Run the worker loop synchronously until nothing is pending.
    fn index_all(sem: &Semantic) {
        let spec = sem.spec.read().unwrap().clone().unwrap();
        // Notes were just written; pretend the quiet period has passed.
        sem.vault
            .with_index(|ix| ix.conn().execute("UPDATE notes SET modified_ms = 0", []))
            .unwrap();
        while sem.step(&spec).unwrap() > 0 {}
        sem.update_progress();
    }

    fn corpus(v: &Vault) -> Vec<String> {
        vec![
            add(
                v,
                "Venture returns follow a power law: a few outlier investments return the fund.",
                &["VC"],
            ),
            add(
                v,
                "Fund returns depend on ownership in the outlier investments that follow the power law.",
                &["VC"],
            ),
            add(
                v,
                "Kyoto temples and the Tokyo itinerary for the spring trip.",
                &["Travel"],
            ),
            add(
                v,
                "Customers describe what they want, not what they actually do. Interview about past behaviour.",
                &[],
            ),
            add(v, "Presentation software should let slides flow like documents.", &[]),
        ]
    }

    #[test]
    fn context_for_questions_by_scope() {
        use context::Scope;
        let (_d, v, sem) = setup();
        let ids = corpus(&v);
        sem.set_embedder(Box::new(HashEmbedder::default())).unwrap();
        index_all(&sem);

        let c = sem.context(&Scope::Notebook, "power law fund returns", 3).unwrap();
        let got: Vec<&str> = c.iter().map(|n| n.note.id.as_str()).collect();
        assert!(
            got.contains(&ids[0].as_str()) && got.contains(&ids[1].as_str()),
            "{got:?}"
        );
        assert!(c.iter().all(|n| !n.body.is_empty()));
        assert!(
            c.windows(2).all(|w| w[0].note.created <= w[1].note.created),
            "oldest first"
        );

        let c = sem.context(&Scope::Note { id: ids[0].clone() }, "", 4).unwrap();
        assert!(c.iter().any(|n| n.note.id == ids[0]), "includes the note itself");
        assert!(c.iter().any(|n| n.note.id == ids[1]), "and what relates to it");

        let c = sem.context(&Scope::Label { label: "vc".into() }, "", 10).unwrap();
        assert_eq!(c.len(), 2);
        let month = &c[0].note.day[..7];
        let c = sem.context(&Scope::Month { month: month.into() }, "", 2).unwrap();
        assert_eq!(c.len(), 2, "spread down to the limit");

        let c = sem.context(&Scope::Note { id: "missing".into() }, "", 4).unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn related_notes_with_reasons_and_dismissal() {
        let (_d, v, sem) = setup();
        let ids = corpus(&v);
        let r = sem.related(&ids[0], 5).unwrap();
        assert!(!r.enabled, "off until a model is set");

        sem.set_embedder(Box::new(HashEmbedder::default())).unwrap();
        index_all(&sem);
        assert_eq!(sem.status(), Status::Ready { notes: 5 });

        let r = sem.related(&ids[0], 5).unwrap();
        assert!(r.enabled && !r.pending);
        assert_eq!(
            r.items.first().map(|i| i.note.id.as_str()),
            Some(ids[1].as_str()),
            "{r:?}"
        );
        let top = &r.items[0];
        assert_eq!(top.shared_labels, vec!["VC"]);
        assert!(top.passage.contains("ownership"));
        assert!(r.items.iter().all(|i| i.note.id != ids[2]), "travel note is unrelated");

        sem.dismiss(&ids[0], &ids[1]).unwrap();
        let r = sem.related(&ids[0], 5).unwrap();
        assert!(r.items.iter().all(|i| i.note.id != ids[1]));
        let r = sem.related(&ids[1], 5).unwrap();
        assert!(r.items.iter().all(|i| i.note.id != ids[0]), "dismissal is symmetric");
    }

    #[test]
    fn edits_are_reembedded_and_cached_vectors_reused() {
        let (_d, v, sem) = setup();
        let ids = corpus(&v);
        sem.set_embedder(Box::new(HashEmbedder::default())).unwrap();
        index_all(&sem);
        // Rebuilding the notes index keeps the vector cache: nothing to re-embed.
        v.rebuild().unwrap();
        let spec = sem.spec.read().unwrap().clone().unwrap();
        let (done, total) = v.with_index(|ix| store::progress(ix.conn(), spec.id)).unwrap();
        assert_eq!((done, total), (5, 5));

        // An edit makes the note pending again.
        v.update(
            &ids[2],
            crate::vault::NoteUpdate {
                content: Some("Ownership and the power law of fund returns.".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let (done, _) = v.with_index(|ix| store::progress(ix.conn(), spec.id)).unwrap();
        assert_eq!(done, 4);
        index_all(&sem);
        let r = sem.related(&ids[2], 5).unwrap();
        assert!(r.items.iter().any(|i| i.note.id == ids[0] || i.note.id == ids[1]));

        // Deleting a note removes it from related results after a sweep.
        v.trash(&ids[1]).unwrap();
        let gone = v.with_index(|ix| store::sweep(ix.conn())).unwrap();
        assert_eq!(gone, vec![ids[1].clone()]);
    }

    #[test]
    fn hybrid_search_finds_meaning_and_text() {
        let (_d, v, sem) = setup();
        let ids = corpus(&v);
        // Without a model: plain full-text.
        let hits = sem.search("outlier", 10).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.matched == MatchKind::Text));

        sem.set_embedder(Box::new(HashEmbedder::default())).unwrap();
        index_all(&sem);
        let hits = sem.search("fund ownership outlier", 10).unwrap();
        assert!(
            hits.iter().any(|h| h.note.id == ids[1] && h.matched == MatchKind::Both),
            "{hits:?}"
        );
        // Filters apply to meaning-only matches too.
        let hits = sem.search("fund ownership #travel", 10).unwrap();
        assert!(hits.iter().all(|h| h.note.labels.contains(&"Travel".to_string())));
    }
}

#[cfg(test)]
mod ideas_tests {
    use super::embedder::HashEmbedder;
    use super::*;

    fn write(root: &std::path::Path, id: &str, day: &str, labels: &str, source: Option<&str>, body: &str) {
        let src = source.map(|s| format!("source:\n  title: {s}\n")).unwrap_or_default();
        let text = format!("---\nid: {id}\ncreated: {day}T10:00:00+00:00\nlabels: [{labels}]\n{src}---\n\n{body}\n");
        std::fs::write(root.join(format!("{id}.md")), text).unwrap();
    }

    fn notebook() -> (tempfile::TempDir, Arc<Vault>, Arc<Semantic>) {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path();
        // A VC thread across the year, starting from a source.
        write(
            r,
            "vc1",
            "2026-01-14",
            "VC",
            Some("Venture Deals"),
            "Venture fund returns follow a power law: outlier companies return the fund.",
        );
        write(
            r,
            "vc2",
            "2026-03-03",
            "VC",
            None,
            "Fund returns depend on ownership in the outlier companies of the power law.",
        );
        write(
            r,
            "vc3",
            "2026-06-20",
            "VC",
            None,
            "Ownership targets and fund returns: concentration on outlier companies.",
        );
        write(r, "vc4", "2026-09-12", "VC", None, "But maybe ownership in outlier companies matters less than fund returns from many companies? Tension with March.");
        // Travel in spring only: dormant by November.
        write(
            r,
            "tr1",
            "2026-02-25",
            "Travel",
            None,
            "Tokyo itinerary: Shibuya, Asakusa, Kyoto temples trip.",
        );
        write(
            r,
            "tr2",
            "2026-03-01",
            "Travel",
            None,
            "Kyoto temples trip and the Tokyo itinerary for spring.",
        );
        write(
            r,
            "tr3",
            "2026-03-20",
            "Travel",
            None,
            "Revisit the Tokyo itinerary and Kyoto temples dates.",
        );
        // A new theme in the last weeks: forming.
        write(
            r,
            "ga1",
            "2026-10-10",
            "",
            None,
            "Garden compost beds and tomato seedlings.",
        );
        write(
            r,
            "ga2",
            "2026-10-18",
            "",
            None,
            "Tomato seedlings need the compost garden beds warmer.",
        );
        write(
            r,
            "ga3",
            "2026-10-25",
            "",
            None,
            "Compost for the garden tomato beds is ready.",
        );
        // An unrelated question nobody came back to.
        write(
            r,
            "q1",
            "2026-05-02",
            "",
            None,
            "Should the sailing club change its membership fees?",
        );
        let vault = Arc::new(Vault::open(r).unwrap());
        vault.rescan().unwrap();
        let sem = Semantic::new(vault.clone(), Arc::new(UserState::open_in_memory().unwrap())).unwrap();
        sem.set_embedder(Box::new(HashEmbedder::default())).unwrap();
        sem.index_now().unwrap();
        (dir, vault, sem)
    }

    fn today() -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
    }

    #[test]
    fn themes_sections_and_signals() {
        let (_d, _v, sem) = notebook();
        sem.refresh_ideas_at(today()).unwrap();
        let o = sem.ideas_overview().unwrap();
        let names = |v: &[ideas::ThemeSummary]| v.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
        assert_eq!(o.returning.len(), 1, "returning: {:?}", names(&o.returning));
        let vc = &o.returning[0];
        assert_eq!(vc.note_count, 4);
        assert_eq!(vc.labels, vec!["VC"]);
        assert_eq!(vc.active_months, 4);
        assert!(
            vc.name.contains("outlier") || vc.name.contains("fund") || vc.name.contains("ownership"),
            "{}",
            vc.name
        );
        assert_eq!(vc.months.len(), 12);
        assert_eq!(vc.months.last().unwrap().month, "2026-11");
        assert_eq!(o.dormant.len(), 1, "dormant: {:?}", names(&o.dormant));
        assert_eq!(o.dormant[0].labels, vec!["Travel"]);
        assert_eq!(o.forming.len(), 1, "forming: {:?}", names(&o.forming));

        // The source note is shown as influencing later personal notes.
        assert_eq!(o.influences[0].source.id, "vc1");
        assert!(o.influences[0].later.iter().any(|n| n.id == "vc2"));

        // The sailing question was never picked up again.
        assert!(o.open_questions.iter().any(|q| q.note.id == "q1"));

        // Timeline: oldest first, with revisit and possible revision marked.
        let tl = sem.theme_timeline(&vc.id).unwrap().unwrap();
        let ids: Vec<_> = tl.entries.iter().map(|e| e.note.id.as_str()).collect();
        assert_eq!(ids, ["vc1", "vc2", "vc3", "vc4"]);
        let last = &tl.entries[3];
        assert!(last.revisits.is_some());
        assert!(last.may_revise, "'but… tension' suggests rethinking");
        assert!(last.is_question);
        assert_eq!(tl.entries[1].influenced_by.as_ref().map(|n| n.id.as_str()), Some("vc1"));

        assert_eq!(sem.note_theme("vc2").unwrap().map(|t| t.id), Some(vc.id.clone()));
        assert!(sem.note_theme("q1").unwrap().is_none());
    }

    #[test]
    fn theme_identity_survives_new_notes() {
        let (d, v, sem) = notebook();
        sem.refresh_ideas_at(today()).unwrap();
        let before = sem.ideas_overview().unwrap().returning[0].id.clone();
        write(
            d.path(),
            "vc5",
            "2026-10-30",
            "VC",
            None,
            "Outlier companies and fund returns again: ownership matters.",
        );
        v.rescan().unwrap();
        sem.index_now().unwrap();
        assert!(sem.ideas_overview().unwrap().stale, "new vectors make the model stale");
        sem.refresh_ideas_at(today()).unwrap();
        let after = sem.ideas_overview().unwrap();
        let vc = after
            .returning
            .iter()
            .chain(&after.forming)
            .find(|t| t.labels == ["VC"])
            .unwrap();
        assert_eq!(vc.id, before, "same theme keeps its id");
        assert_eq!(vc.note_count, 5);
    }

    #[test]
    fn off_without_model() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Arc::new(Vault::open(dir.path()).unwrap());
        let sem = Semantic::new(vault, Arc::new(UserState::open_in_memory().unwrap())).unwrap();
        assert!(!sem.ideas_overview().unwrap().enabled);
    }
}

#[cfg(test)]
mod feedback_tests {
    use super::embedder::HashEmbedder;
    use super::*;
    use crate::vault::{EditableMeta, NewNote};

    fn note(v: &Vault, label: &str, text: &str) -> String {
        v.create(NewNote {
            content: text.into(),
            meta: EditableMeta {
                labels: vec![label.into()],
                ..Default::default()
            },
        })
        .unwrap()
        .id
    }

    #[test]
    fn suggestions_adapt_to_this_users_feedback() {
        let dir = tempfile::tempdir().unwrap();
        let v = Arc::new(Vault::open(dir.path()).unwrap());
        // Two topics that share vocabulary (returns, ownership, growth…).
        let vc: Vec<String> = [
            "Fund returns depend on ownership of outlier companies and growth.",
            "Ownership of outlier companies drives fund returns and growth.",
            "Outlier companies, ownership and fund returns: growth matters.",
            "Growth of outlier companies and ownership decide fund returns.",
            "Fund returns, ownership and outlier growth in venture portfolios.",
        ]
        .iter()
        .map(|t| note(&v, "VC", t))
        .collect();
        let garden: Vec<String> = [
            "Garden returns: ownership of the allotment and tomato growth.",
            "Tomato growth returns each spring; allotment ownership matters.",
            "Allotment ownership and garden growth returns for tomatoes.",
        ]
        .iter()
        .map(|t| note(&v, "Garden", t))
        .collect();
        let sem = Semantic::new(v.clone(), Arc::new(UserState::open_in_memory().unwrap())).unwrap();
        sem.set_embedder(Box::new(HashEmbedder::default())).unwrap();
        sem.index_now().unwrap();

        let labels = |r: &RelatedNotes| r.items.iter().map(|i| i.note.labels[0].clone()).collect::<Vec<_>>();
        let target = &vc[4];
        let before = sem.related(target, 10).unwrap();
        assert!(!before.personalized);
        assert!(
            labels(&before).contains(&"Garden".to_string()),
            "cross-topic look-alike shown before: {:?}",
            labels(&before)
        );

        // Feedback on *other* notes: open same-topic, dismiss cross-topic.
        for (i, a) in vc[..3].iter().enumerate() {
            sem.opened_related(a, &vc[(i + 1) % 3]).unwrap();
            sem.dismiss(a, &garden[i]).unwrap();
        }
        let after = sem.related(target, 10).unwrap();
        assert!(after.personalized);
        assert!(
            !labels(&after).contains(&"Garden".to_string()),
            "learned to skip: {:?}",
            labels(&after)
        );
        assert!(
            labels(&after).iter().filter(|l| *l == "VC").count() >= 2,
            "{:?}",
            labels(&after)
        );

        // Undo and reset are honoured.
        sem.reset_feedback().unwrap();
        assert!(!sem.related(target, 10).unwrap().personalized);
    }
}
