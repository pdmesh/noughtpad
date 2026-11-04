//! A vault is a folder of Markdown notes plus a hidden `.noughtpad/` data folder.
//!
//! The vault is the only component that writes note files. It writes them
//! atomically and keeps the index in step. Edits made by other tools are
//! picked up through [`Vault::sync_path`] (driven by the watcher) or
//! [`Vault::rescan`].

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

use chrono::{DateTime, FixedOffset, Local, TimeZone};
use serde::{Deserialize, Serialize};

use crate::error::IoContext;
use crate::index::{DaySummary, Index, IndexedNote, LabelCount, ListQuery, MonthLog, NoteSummary, SearchHit};
use crate::note::{slugify, Note, NoteKind, NoteMeta, Source};
use crate::{Error, Result};

pub const DATA_DIR: &str = ".noughtpad";
const INDEX_FILE: &str = "index.sqlite";
const TRASH_DIR: &str = "trash";
/// Notes written per index transaction during a scan.
const SCAN_BATCH: usize = 250;

/// A note as the editor sees it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteDoc {
    pub id: String,
    pub path: String,
    /// The body without the conventional blank line after frontmatter.
    pub content: String,
    /// Explicit title, if the user set one.
    pub title: Option<String>,
    pub labels: Vec<String>,
    pub source: Option<Source>,
    /// Explicit kind override; `None` means inferred.
    pub kind_override: Option<NoteKind>,
    pub kind: NoteKind,
    pub created: String,
    /// Hash of the file on disk; send it back on save to detect external edits.
    pub hash: String,
}

/// Metadata the user can edit. Sent in full on every metadata change.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct EditableMeta {
    pub title: Option<String>,
    pub labels: Vec<String>,
    pub source: Option<Source>,
    pub kind_override: Option<NoteKind>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NewNote {
    pub content: String,
    pub meta: EditableMeta,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NoteUpdate {
    pub content: Option<String>,
    pub meta: Option<EditableMeta>,
    /// Hash the editor last saw; if the file changed since, the save is refused.
    pub base_hash: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub indexed: usize,
    pub unchanged: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Change {
    Unchanged,
    Upserted { id: String, path: String },
    Removed { path: String },
}

pub struct Vault {
    root: PathBuf,
    index: Mutex<Index>,
    /// Held across read-check-write of a file (editor saves, sync writes), so
    /// a sync can't slip in between the editor's hash check and its write.
    writes: Mutex<()>,
}

/// A file that syncs, as found on disk (see [`Vault::synced_files`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub path: String,
    pub size: u64,
    /// Modification time, ms since the epoch.
    pub modified: i64,
}

/// Lines `.noughtpad/.gitignore` must hold: derived or per-device data.
const GITIGNORE: [&str; 3] = ["index.sqlite*", "trash/", "sync/"];

impl Vault {
    /// Open (creating if needed) a vault at `root`. Call [`Vault::rescan`] afterwards.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        fs::create_dir_all(root).at(root)?;
        let root = root.canonicalize().at(root)?;
        let data = root.join(DATA_DIR);
        fs::create_dir_all(&data).at(&data)?;
        ensure_gitignore(&data.join(".gitignore"))?;
        let index = Index::open(&data.join(INDEX_FILE))?;
        Ok(Vault {
            root,
            index: Mutex::new(index),
            writes: Mutex::new(()),
        })
    }

    /// An in-memory index over `root`; for tests.
    pub fn open_ephemeral(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize().at(root.as_ref())?;
        Ok(Vault {
            root,
            index: Mutex::new(Index::open_in_memory()?),
            writes: Mutex::new(()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Run `f` with exclusive access to the index (for the semantic layer).
    pub(crate) fn with_index<R>(&self, f: impl FnOnce(&mut Index) -> R) -> R {
        f(&mut self.ix())
    }

    pub fn summary(&self, id: &str) -> Result<Option<NoteSummary>> {
        self.ix().get(id)
    }

    fn write_guard(&self) -> MutexGuard<'_, ()> {
        self.writes.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn ix(&self) -> MutexGuard<'_, Index> {
        // A panic while holding the lock can't leave the derived index in a state
        // worse than "stale", so recover rather than poison the whole app.
        self.index.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ---- reading ---------------------------------------------------------

    pub fn list(&self, q: &ListQuery) -> Result<Vec<NoteSummary>> {
        self.ix().list(q)
    }
    pub fn search(&self, q: &str, limit: u32) -> Result<Vec<SearchHit>> {
        self.ix().search(q, limit)
    }
    pub fn labels(&self) -> Result<Vec<LabelCount>> {
        self.ix().labels()
    }
    pub fn days(&self, q: &ListQuery) -> Result<Vec<DaySummary>> {
        self.ix().days(q)
    }
    pub fn month_log(&self, month: &str) -> Result<MonthLog> {
        self.ix().month_log(month)
    }
    pub fn day_counts(&self, from_day: &str) -> Result<Vec<crate::index::DayCount>> {
        self.ix().day_counts(from_day)
    }
    pub fn months(&self) -> Result<Vec<String>> {
        self.ix().months()
    }
    pub fn count(&self) -> Result<i64> {
        self.ix().count()
    }

    pub fn get(&self, id: &str) -> Result<NoteDoc> {
        let rel = self.ix().path_of(id)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        let abs = self.abs(&rel)?;
        let text = fs::read_to_string(&abs).at(&abs)?;
        let note = Note::parse(&text);
        let created = self.created_for(&note, &rel, &abs);
        Ok(doc(id.to_string(), rel, &note, created, hash(&text)))
    }

    // ---- writing ---------------------------------------------------------

    /// Create a note. Returns its document. Empty content is allowed, but the
    /// UI normally only calls this once the user has typed something.
    pub fn create(&self, input: NewNote) -> Result<NoteDoc> {
        let now = Local::now().fixed_offset();
        let id = ulid::Ulid::new().to_string();
        let mut meta = NoteMeta {
            id: Some(id.clone()),
            title: input.meta.title,
            created: Some(trim_subsec(now)),
            labels: input.meta.labels,
            source: input.meta.source,
            kind: input.meta.kind_override,
        };
        meta.normalise();
        let note = Note::new(meta, input.content);
        let rel = self.unique_path(&note, now)?;
        let text = note.render();
        self.write_atomic(&rel, &text)?;
        self.index_text(&rel, &text)?;
        self.get(&id)
    }

    /// Update content and/or metadata. The body is replaced verbatim; the
    /// frontmatter is rewritten only if metadata actually changed.
    pub fn update(&self, id: &str, upd: NoteUpdate) -> Result<NoteDoc> {
        let _w = self.write_guard();
        let rel = self.ix().path_of(id)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        let abs = self.abs(&rel)?;
        let current = fs::read_to_string(&abs).at(&abs)?;
        if let Some(base) = &upd.base_hash {
            if *base != hash(&current) {
                return Err(Error::Conflict(rel));
            }
        }
        let mut note = Note::parse(&current);
        if let Some(content) = upd.content {
            // Keep whatever blank line separated frontmatter from the body.
            let lead = note.body[..note.body.len() - note.content().len()].to_string();
            note.body = format!("{lead}{content}");
        }
        if let Some(m) = upd.meta {
            let mut meta = note.meta.clone();
            meta.title = m.title;
            meta.labels = m.labels;
            meta.source = m.source;
            meta.kind = m.kind_override;
            meta.normalise();
            note.meta = meta;
        }
        let text = note.render();
        if text != current {
            self.write_atomic(&rel, &text)?;
            self.index_text(&rel, &text)?;
        }
        self.get(id)
    }

    /// Move a note to `.noughtpad/trash/` (recoverable), and drop it from the index.
    pub fn trash(&self, id: &str) -> Result<()> {
        let _w = self.write_guard();
        let rel = self.ix().path_of(id)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        self.move_to_trash(&rel)?;
        self.ix().remove_path(&rel)?;
        Ok(())
    }

    fn move_to_trash(&self, rel: &str) -> Result<()> {
        let from = self.abs(rel)?;
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let trash = self.root.join(DATA_DIR).join(TRASH_DIR);
        let name = rel.replace('/', "__");
        let mut to = trash.join(format!("{stamp}-{name}"));
        for n in 2.. {
            if !to.exists() {
                break;
            }
            to = trash.join(format!("{stamp}-{n}-{name}"));
        }
        fs::create_dir_all(&trash).at(&trash)?;
        fs::rename(&from, &to).at(&from)?;
        Ok(())
    }

    // ---- for sync --------------------------------------------------------

    /// Every file in the notebook that syncs (`sync::is_synced`): notes,
    /// other files, chats, trash, user-state logs, tombstones.
    pub fn synced_files(&self) -> Result<Vec<LocalFile>> {
        let mut out = Vec::new();
        let walker = walkdir::WalkDir::new(&self.root)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                match e.depth() {
                    0 => true,
                    1 if name == DATA_DIR => true,
                    2 if e
                        .path()
                        .parent()
                        .and_then(|p| p.file_name())
                        .is_some_and(|p| p == DATA_DIR) =>
                    {
                        crate::sync::SYNCED_DATA_DIRS.contains(&name.as_ref())
                    }
                    _ => !is_ignored_name(&name),
                }
            });
        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("skipping unreadable entry: {e}");
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = self.rel(entry.path())?;
            if !crate::sync::is_synced(&rel) {
                continue;
            }
            let meta = entry.metadata().map_err(|e| Error::Sync(e.to_string()))?;
            out.push(LocalFile {
                path: rel,
                size: meta.len(),
                modified: mtime_ms(entry.path()),
            });
        }
        Ok(out)
    }

    /// The bytes of a synced file, or `None` if it doesn't exist.
    pub fn read_synced(&self, rel: &str) -> Result<Option<Vec<u8>>> {
        let abs = self.abs(rel)?;
        match fs::read(&abs) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Io { path: abs, source: e }),
        }
    }

    /// Write a file that sync downloaded or merged. Refused with
    /// [`Error::Conflict`] unless the file on disk still has the hash the
    /// engine planned against (`expected`, from [`hash_bytes`]; `None`: it
    /// must not exist), so an edit made meanwhile is never overwritten; the
    /// engine plans that path again next round. Notes are re-indexed at once.
    pub fn apply_synced(&self, rel: &str, bytes: &[u8], expected: Option<&str>) -> Result<Change> {
        if !crate::sync::is_synced(rel) {
            return Err(Error::OutsideVault(PathBuf::from(rel)));
        }
        let _w = self.write_guard();
        self.check_synced(rel, expected)?;
        self.write_atomic_bytes(rel, bytes)?;
        self.after_synced(rel)
    }

    /// Remove a file because it was deleted on another device. Notebook files
    /// go to `.noughtpad/trash/` (sync never deletes outright); files already
    /// inside `.noughtpad/` (e.g. a trash entry) are removed. Same `expected`
    /// rule as [`Vault::apply_synced`].
    pub fn remove_synced(&self, rel: &str, expected: &str) -> Result<Change> {
        if !crate::sync::is_synced(rel) {
            return Err(Error::OutsideVault(PathBuf::from(rel)));
        }
        let _w = self.write_guard();
        self.check_synced(rel, Some(expected))?;
        if rel.starts_with(&format!("{DATA_DIR}/")) {
            let abs = self.abs(rel)?;
            fs::remove_file(&abs).at(&abs)?;
        } else {
            self.move_to_trash(rel)?;
        }
        self.after_synced(rel)
    }

    fn check_synced(&self, rel: &str, expected: Option<&str>) -> Result<()> {
        let current = self.read_synced(rel)?.map(|b| hash_bytes(&b));
        if current.as_deref() != expected {
            return Err(Error::Conflict(rel.to_string()));
        }
        Ok(())
    }

    fn after_synced(&self, rel: &str) -> Result<Change> {
        if rel.split('/').any(is_ignored_name) || !is_markdown(Path::new(rel)) {
            return Ok(Change::Unchanged);
        }
        self.sync_rel(rel)
    }

    /// The vault-relative path of `abs`, if it is inside the vault.
    pub fn relative(&self, abs: &Path) -> Option<String> {
        self.rel(abs).ok()
    }

    // ---- syncing with disk -----------------------------------------------

    /// Walk the vault and bring the index up to date. Cheap when nothing changed.
    pub fn rescan(&self) -> Result<ScanReport> {
        let mut report = ScanReport::default();
        let known = self.ix().fingerprints()?;
        let mut pending = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let walker = walkdir::WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !is_ignored_name(&e.file_name().to_string_lossy()));
        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("skipping unreadable entry: {e}");
                    continue;
                }
            };
            if !entry.file_type().is_file() || !is_markdown(entry.path()) {
                continue;
            }
            let rel = self.rel(entry.path())?;
            seen.insert(rel.clone());
            let mtime = mtime_ms(entry.path());
            match known.get(&rel) {
                Some((_, known_mtime)) if *known_mtime == mtime => report.unchanged += 1,
                _ => pending.push(rel),
            }
        }
        // Read and parse without holding the index lock, then write in batches,
        // so the UI's queries and the editor's saves are never starved during
        // a large first scan.
        for chunk in pending.chunks(SCAN_BATCH) {
            let mut batch = Vec::with_capacity(chunk.len());
            for rel in chunk {
                let abs = self.root.join(rel);
                let text = match fs::read_to_string(&abs) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!("skipping {rel}: {e}");
                        continue;
                    }
                };
                if known.get(rel).is_some_and(|(h, _)| *h == hash(&text)) {
                    report.unchanged += 1;
                    continue;
                }
                batch.push(self.prepare(rel, &text));
            }
            report.indexed += batch.len();
            self.ix().upsert_many(&batch)?;
        }
        let mut ix = self.ix();
        for path in known.keys().filter(|p| !seen.contains(*p)) {
            ix.remove_path(path)?;
            report.removed += 1;
        }
        Ok(report)
    }

    /// Re-index (or un-index) a single path after a filesystem event.
    pub fn sync_path(&self, abs: &Path) -> Result<Change> {
        let rel = match self.rel(abs) {
            Ok(r) => r,
            Err(Error::OutsideVault(_)) => return Ok(Change::Unchanged),
            Err(e) => return Err(e),
        };
        if rel.split('/').any(is_ignored_name) || !is_markdown(abs) {
            return Ok(Change::Unchanged);
        }
        self.sync_rel(&rel)
    }

    fn sync_rel(&self, rel: &str) -> Result<Change> {
        let abs = self.root.join(rel);
        match fs::read_to_string(&abs) {
            Ok(text) => {
                if self.ix().hash_of_path(rel)?.as_deref() == Some(hash(&text).as_str()) {
                    return Ok(Change::Unchanged);
                }
                let id = self.index_text(rel, &text)?;
                Ok(Change::Upserted {
                    id,
                    path: rel.to_string(),
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if self.ix().remove_path(rel)? {
                    Ok(Change::Removed { path: rel.to_string() })
                } else {
                    Ok(Change::Unchanged)
                }
            }
            Err(e) => Err(Error::Io { path: abs, source: e }),
        }
    }

    /// Delete and rebuild the whole index from the Markdown files.
    pub fn rebuild(&self) -> Result<ScanReport> {
        self.ix().clear()?;
        self.rescan()
    }

    // ---- internals -------------------------------------------------------

    fn index_text(&self, rel: &str, text: &str) -> Result<String> {
        let n = self.prepare(rel, text);
        self.ix().upsert(&n)?;
        Ok(n.id)
    }

    /// Parse a file into what the index stores (no locking, no writes).
    fn prepare(&self, rel: &str, text: &str) -> IndexedNote {
        let abs = self.root.join(rel);
        let note = Note::parse(text);
        IndexedNote {
            id: note.meta.id.clone().unwrap_or_else(|| path_id(rel)),
            path: rel.to_string(),
            created: self.created_for(&note, rel, &abs),
            modified_ms: mtime_ms(&abs),
            kind: note.meta.effective_kind(),
            title: note.title(),
            has_title: note.meta.title.is_some(),
            body: note.content().to_string(),
            labels: note.meta.labels.clone(),
            source: note.meta.source.clone(),
            hash: hash(text),
        }
    }

    /// `created` frontmatter, else a date in the filename, else file times.
    fn created_for(&self, note: &Note, rel: &str, abs: &Path) -> DateTime<FixedOffset> {
        if let Some(c) = note.meta.created {
            return c;
        }
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if name.len() >= 10 {
            if let Some(d) = crate::note::parse_datetime(&name[..10]) {
                return d;
            }
        }
        let t = fs::metadata(abs)
            .and_then(|m| m.created().or_else(|_| m.modified()))
            .ok();
        let ms = t
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis() as i64);
        Local
            .timestamp_millis_opt(ms)
            .single()
            .unwrap_or_else(Local::now)
            .fixed_offset()
    }

    fn unique_path(&self, note: &Note, now: DateTime<FixedOffset>) -> Result<String> {
        let dir = now.format("%Y/%m").to_string();
        let stem = format!("{}-{}", now.format("%Y-%m-%d-%H%M"), slugify(&note.title(), 6));
        for n in 1.. {
            let name = if n == 1 {
                format!("{stem}.md")
            } else {
                format!("{stem}-{n}.md")
            };
            let rel = format!("{dir}/{name}");
            if !self.root.join(&rel).exists() {
                return Ok(rel);
            }
        }
        unreachable!()
    }

    fn write_atomic(&self, rel: &str, text: &str) -> Result<()> {
        self.write_atomic_bytes(rel, text.as_bytes())
    }

    fn write_atomic_bytes(&self, rel: &str, bytes: &[u8]) -> Result<()> {
        let abs = self.abs(rel)?;
        let dir = abs.parent().expect("note path has a parent");
        fs::create_dir_all(dir).at(dir)?;
        let mut tmp = tempfile::Builder::new()
            .prefix(".noughtpad-tmp-")
            .tempfile_in(dir)
            .at(dir)?;
        std::io::Write::write_all(&mut tmp, bytes).at(tmp.path())?;
        tmp.as_file().sync_all().at(&abs)?;
        tmp.persist(&abs).map_err(|e| Error::Io {
            path: abs.clone(),
            source: e.error,
        })?;
        Ok(())
    }

    fn rel(&self, abs: &Path) -> Result<String> {
        let rel = abs
            .strip_prefix(&self.root)
            .map_err(|_| Error::OutsideVault(abs.to_path_buf()))?;
        Ok(rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"))
    }

    /// Resolve a vault-relative path, refusing anything that escapes the vault.
    fn abs(&self, rel: &str) -> Result<PathBuf> {
        let p = Path::new(rel);
        if p.components().any(|c| !matches!(c, Component::Normal(_))) {
            return Err(Error::OutsideVault(p.to_path_buf()));
        }
        Ok(self.root.join(p))
    }
}

fn doc(id: String, path: String, note: &Note, created: DateTime<FixedOffset>, hash: String) -> NoteDoc {
    NoteDoc {
        id,
        path,
        content: note.content().to_string(),
        title: note.meta.title.clone(),
        labels: note.meta.labels.clone(),
        source: note.meta.source.clone(),
        kind_override: note.meta.kind,
        kind: note.meta.effective_kind(),
        created: created.to_rfc3339(),
        hash,
    }
}

fn trim_subsec(t: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
    use chrono::Timelike;
    t.with_nanosecond(0).unwrap_or(t)
}

pub fn hash(text: &str) -> String {
    hash_bytes(text.as_bytes())
}

/// The vault's content hash, for any file (notes hash the same as [`hash`]).
pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex()[..32].to_string()
}

/// Add any missing [`GITIGNORE`] lines, keeping what the user wrote.
fn ensure_gitignore(path: &Path) -> Result<()> {
    let current = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            tracing::warn!("could not read {}: {e}", path.display());
            return Ok(());
        }
    };
    let have: std::collections::HashSet<&str> = current.lines().map(str::trim).collect();
    let missing: Vec<&str> = GITIGNORE.iter().copied().filter(|l| !have.contains(l)).collect();
    if missing.is_empty() {
        return Ok(());
    }
    let mut text = current.clone();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for l in missing {
        text.push_str(l);
        text.push('\n');
    }
    fs::write(path, text).at(path)
}

/// Stable id for files without an `id` in frontmatter, derived from the path.
fn path_id(rel: &str) -> String {
    format!("p-{}", &blake3::hash(rel.as_bytes()).to_hex()[..24])
}

fn is_ignored_name(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules"
}

fn is_markdown(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

pub(crate) fn mtime_ms(p: &Path) -> i64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> (tempfile::TempDir, Vault) {
        let dir = tempfile::tempdir().unwrap();
        let v = Vault::open(dir.path()).unwrap();
        (dir, v)
    }

    #[test]
    fn create_writes_minimal_markdown() {
        let (_d, v) = vault();
        let doc = v
            .create(NewNote {
                content: "Ownership seems to matter more than I thought in VC.\n".into(),
                meta: EditableMeta {
                    labels: vec!["VC".into()],
                    ..Default::default()
                },
            })
            .unwrap();
        assert!(
            doc.path.ends_with("-ownership-seems-to-matter-more-than.md"),
            "{}",
            doc.path
        );
        let text = fs::read_to_string(v.root().join(&doc.path)).unwrap();
        assert!(text.starts_with("---\nid: "));
        assert!(text.contains("labels: [VC]\n"));
        assert!(text.ends_with("---\n\nOwnership seems to matter more than I thought in VC.\n"));
        assert_eq!(doc.content, "Ownership seems to matter more than I thought in VC.\n");
        assert_eq!(v.search("ownership", 5).unwrap().len(), 1);
    }

    #[test]
    fn optional_title_names_the_file_and_is_indexed() {
        let (_d, v) = vault();
        let doc = v
            .create(NewNote {
                content: "body text".into(),
                meta: EditableMeta {
                    title: Some("Fund maths".into()),
                    ..Default::default()
                },
            })
            .unwrap();
        assert!(doc.path.ends_with("-fund-maths.md"), "{}", doc.path);
        assert_eq!(doc.title.as_deref(), Some("Fund maths"));
        let s = &v.list(&ListQuery::default()).unwrap()[0];
        assert!(s.has_title);
        assert_eq!(s.title, "Fund maths");
        assert_eq!(v.search("maths", 5).unwrap().len(), 1);
        // Clearing the title falls back to the first line; the file is not renamed.
        let doc2 = v
            .update(
                &doc.id,
                NoteUpdate {
                    meta: Some(EditableMeta::default()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(doc2.path, doc.path);
        let s = &v.list(&ListQuery::default()).unwrap()[0];
        assert!(!s.has_title);
        assert_eq!(s.title, "body text");
    }

    #[test]
    fn update_preserves_body_and_detects_conflicts() {
        let (_d, v) = vault();
        let doc = v
            .create(NewNote {
                content: "first".into(),
                ..Default::default()
            })
            .unwrap();
        let doc2 = v
            .update(
                &doc.id,
                NoteUpdate {
                    content: Some("first\n\n  second *line*  ".into()),
                    base_hash: Some(doc.hash.clone()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(doc2.content, "first\n\n  second *line*  ");
        assert_eq!(doc2.path, doc.path, "file is never renamed after creation");
        // Stale hash -> conflict.
        let err = v.update(
            &doc.id,
            NoteUpdate {
                content: Some("x".into()),
                base_hash: Some(doc.hash),
                ..Default::default()
            },
        );
        assert!(matches!(err, Err(Error::Conflict(_))));
        // Metadata change keeps body.
        let doc3 = v
            .update(
                &doc.id,
                NoteUpdate {
                    meta: Some(EditableMeta {
                        title: None,
                        labels: vec!["Books".into()],
                        source: Some(Source {
                            title: Some("The Mom Test".into()),
                            url: None,
                        }),
                        kind_override: None,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(doc3.content, doc2.content);
        assert_eq!(doc3.kind, NoteKind::Source);
        assert_eq!(v.labels().unwrap()[0].label, "Books");
    }

    #[test]
    fn external_files_and_rescan() {
        let (_d, v) = vault();
        fs::create_dir_all(v.root().join("misc")).unwrap();
        fs::write(
            v.root().join("misc/2026-02-03 thoughts.md"),
            "# Plain file\nNo frontmatter here",
        )
        .unwrap();
        fs::create_dir_all(v.root().join(".obsidian")).unwrap();
        fs::write(v.root().join(".obsidian/ignored.md"), "ignored").unwrap();
        fs::write(v.root().join("readme.txt"), "not markdown").unwrap();
        let r = v.rescan().unwrap();
        assert_eq!(
            r,
            ScanReport {
                indexed: 1,
                unchanged: 0,
                removed: 0
            }
        );
        let n = &v.list(&ListQuery::default()).unwrap()[0];
        assert_eq!(n.title, "Plain file");
        assert_eq!(n.day, "2026-02-03");
        assert!(n.id.starts_with("p-"));
        // Editing an external file through Noughtpad keeps it frontmatter-free.
        let doc = v
            .update(
                &n.id,
                NoteUpdate {
                    content: Some("# Plain file\nEdited".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(v.root().join(&doc.path)).unwrap(),
            "# Plain file\nEdited"
        );
        // Second scan is a no-op; deletion is noticed.
        assert_eq!(v.rescan().unwrap().indexed, 0);
        fs::remove_file(v.root().join("misc/2026-02-03 thoughts.md")).unwrap();
        assert_eq!(v.rescan().unwrap().removed, 1);
        assert_eq!(v.count().unwrap(), 0);
    }

    #[test]
    fn sync_path_handles_external_edit_and_delete() {
        let (_d, v) = vault();
        let doc = v
            .create(NewNote {
                content: "alpha".into(),
                ..Default::default()
            })
            .unwrap();
        let abs = v.root().join(&doc.path);
        assert_eq!(v.sync_path(&abs).unwrap(), Change::Unchanged, "own writes are no-ops");
        let text = fs::read_to_string(&abs).unwrap().replace("alpha", "beta");
        fs::write(&abs, text).unwrap();
        assert!(matches!(v.sync_path(&abs).unwrap(), Change::Upserted { .. }));
        assert_eq!(v.search("beta", 5).unwrap().len(), 1);
        fs::remove_file(&abs).unwrap();
        assert!(matches!(v.sync_path(&abs).unwrap(), Change::Removed { .. }));
    }

    #[test]
    fn trash_and_rebuild() {
        let (_d, v) = vault();
        let a = v
            .create(NewNote {
                content: "keep me".into(),
                ..Default::default()
            })
            .unwrap();
        let b = v
            .create(NewNote {
                content: "bin me".into(),
                ..Default::default()
            })
            .unwrap();
        v.trash(&b.id).unwrap();
        assert_eq!(v.count().unwrap(), 1);
        assert_eq!(v.rebuild().unwrap().indexed, 1, "trash is not indexed");
        assert_eq!(v.get(&a.id).unwrap().content, "keep me");
    }

    #[test]
    fn sample_notebook_indexes_cleanly() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/vault");
        let dir = tempfile::tempdir().unwrap();
        for e in walkdir::WalkDir::new(&src)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
        {
            let to = dir.path().join(e.path().strip_prefix(&src).unwrap());
            fs::create_dir_all(to.parent().unwrap()).unwrap();
            fs::copy(e.path(), to).unwrap();
        }
        let v = Vault::open(dir.path()).unwrap();
        assert_eq!(v.rescan().unwrap().indexed, 37);
        let notes = v
            .list(&ListQuery {
                limit: Some(100),
                ..Default::default()
            })
            .unwrap();
        assert!(
            notes.iter().filter(|n| !n.id.starts_with("p-")).count() == 36,
            "all but the imported file have ids"
        );
        assert_eq!(v.search("ownership", 50).unwrap().len(), 5);
        assert!(v.labels().unwrap().iter().any(|l| l.label == "VC" && l.count == 10));
    }

    #[test]
    fn rejects_path_escape() {
        let (_d, v) = vault();
        assert!(matches!(v.abs("../etc/passwd"), Err(Error::OutsideVault(_))));
        assert!(matches!(v.sync_path(Path::new("/etc/passwd")), Ok(Change::Unchanged)));
    }

    // ---- sync ------------------------------------------------------------

    #[test]
    fn apply_synced_writes_only_over_the_planned_version() {
        let dir = tempfile::tempdir().unwrap();
        let v = Vault::open(dir.path()).unwrap();
        // A new note from another device.
        let c = v
            .apply_synced("2026/09/a.md", b"---\nid: AAA\n---\n\nFrom the Mac", None)
            .unwrap();
        assert!(matches!(c, Change::Upserted { ref id, .. } if id == "AAA"));
        assert_eq!(v.search("Mac", 5).unwrap().len(), 1);
        // Creating it again is refused: it exists now.
        assert!(matches!(
            v.apply_synced("2026/09/a.md", b"x", None),
            Err(Error::Conflict(_))
        ));
        // The user edits it before the next sync writes: refused, edit kept.
        let planned = hash_bytes(&v.read_synced("2026/09/a.md").unwrap().unwrap());
        let doc = v.get("AAA").unwrap();
        v.update(
            "AAA",
            NoteUpdate {
                content: Some("Typed here".into()),
                base_hash: Some(doc.hash),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            v.apply_synced("2026/09/a.md", b"merged", Some(&planned)),
            Err(Error::Conflict(_))
        ));
        assert_eq!(v.get("AAA").unwrap().content, "Typed here");
        // With the right hash it goes through.
        let now = hash_bytes(&v.read_synced("2026/09/a.md").unwrap().unwrap());
        v.apply_synced("2026/09/a.md", b"---\nid: AAA\n---\n\nMerged", Some(&now))
            .unwrap();
        assert_eq!(v.get("AAA").unwrap().content, "Merged");
    }

    #[test]
    fn apply_synced_handles_other_files_and_refuses_unsynced_paths() {
        let dir = tempfile::tempdir().unwrap();
        let v = Vault::open(dir.path()).unwrap();
        assert_eq!(
            v.apply_synced("img/p.png", b"\x89PNG", None).unwrap(),
            Change::Unchanged
        );
        assert_eq!(v.read_synced("img/p.png").unwrap().unwrap(), b"\x89PNG");
        v.apply_synced(".noughtpad/chats/C1.json", b"{}", None).unwrap();
        for bad in [".noughtpad/index.sqlite", ".git/config", "../x.md"] {
            assert!(v.apply_synced(bad, b"x", None).is_err(), "{bad}");
        }
    }

    #[test]
    fn remove_synced_moves_notes_to_the_trash() {
        let dir = tempfile::tempdir().unwrap();
        let v = Vault::open(dir.path()).unwrap();
        let doc = v
            .create(NewNote {
                content: "Deleted on the other Mac".into(),
                ..Default::default()
            })
            .unwrap();
        let h = hash_bytes(&v.read_synced(&doc.path).unwrap().unwrap());
        assert!(matches!(v.remove_synced(&doc.path, "stale"), Err(Error::Conflict(_))));
        assert!(matches!(
            v.remove_synced(&doc.path, &h).unwrap(),
            Change::Removed { .. }
        ));
        assert!(v.read_synced(&doc.path).unwrap().is_none());
        assert_eq!(v.count().unwrap(), 0);
        let trashed: Vec<_> = v
            .synced_files()
            .unwrap()
            .into_iter()
            .filter(|f| f.path.starts_with(".noughtpad/trash/"))
            .collect();
        assert_eq!(trashed.len(), 1);
        // A trash entry removed elsewhere is just removed.
        let t = &trashed[0].path;
        let th = hash_bytes(&v.read_synced(t).unwrap().unwrap());
        v.remove_synced(t, &th).unwrap();
        assert!(v.read_synced(t).unwrap().is_none());
    }

    #[test]
    fn synced_files_lists_what_syncs() {
        let dir = tempfile::tempdir().unwrap();
        let v = Vault::open(dir.path()).unwrap();
        let r = v.root().to_path_buf();
        for (p, body) in [
            ("a.md", "a"),
            ("img/p.png", "p"),
            (".noughtpad/chats/C1.json", "{}"),
            (".noughtpad/user-state/D1.jsonl", ""),
            (".noughtpad/sync/state.sqlite", "x"),
            (".noughtpad/artifacts/k.md", "x"),
            (".git/HEAD", "x"),
            (".DS_Store", "x"),
        ] {
            fs::create_dir_all(r.join(p).parent().unwrap()).unwrap();
            fs::write(r.join(p), body).unwrap();
        }
        let files: Vec<_> = v.synced_files().unwrap().into_iter().map(|f| f.path).collect();
        assert_eq!(
            files,
            vec![
                ".noughtpad/chats/C1.json",
                ".noughtpad/user-state/D1.jsonl",
                "a.md",
                "img/p.png"
            ]
        );
    }

    #[test]
    fn gitignore_gains_missing_lines_and_keeps_the_users() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(DATA_DIR)).unwrap();
        let gi = dir.path().join(DATA_DIR).join(".gitignore");
        fs::write(&gi, "index.sqlite*\ntrash/\nmine/").unwrap();
        Vault::open(dir.path()).unwrap();
        Vault::open(dir.path()).unwrap();
        assert_eq!(
            fs::read_to_string(&gi).unwrap(),
            "index.sqlite*\ntrash/\nmine/\nsync/\n"
        );
    }
}
