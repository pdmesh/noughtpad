//! A vault is a folder of Markdown notes plus a hidden `.nought/` data folder.
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

pub const DATA_DIR: &str = ".nought";
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
}

impl Vault {
    /// Open (creating if needed) a vault at `root`. Call [`Vault::rescan`] afterwards.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        fs::create_dir_all(root).at(root)?;
        let root = root.canonicalize().at(root)?;
        let data = root.join(DATA_DIR);
        fs::create_dir_all(&data).at(&data)?;
        let ignore = data.join(".gitignore");
        if !ignore.exists() {
            // The index is derived; keep it out of git by default.
            fs::write(&ignore, "index.sqlite*\ntrash/\n").at(&ignore)?;
        }
        let index = Index::open(&data.join(INDEX_FILE))?;
        Ok(Vault {
            root,
            index: Mutex::new(index),
        })
    }

    /// An in-memory index over `root`; for tests.
    pub fn open_ephemeral(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize().at(root.as_ref())?;
        Ok(Vault {
            root,
            index: Mutex::new(Index::open_in_memory()?),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
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

    /// Move a note to `.nought/trash/` (recoverable), and drop it from the index.
    pub fn trash(&self, id: &str) -> Result<()> {
        let rel = self.ix().path_of(id)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
        let from = self.abs(&rel)?;
        let stamp = Local::now().format("%Y%m%d-%H%M%S");
        let to = self
            .root
            .join(DATA_DIR)
            .join(TRASH_DIR)
            .join(format!("{stamp}-{}", rel.replace('/', "__")));
        fs::create_dir_all(to.parent().unwrap()).at(&to)?;
        fs::rename(&from, &to).at(&from)?;
        self.ix().remove_path(&rel)?;
        Ok(())
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
        let abs = self.abs(rel)?;
        let dir = abs.parent().expect("note path has a parent");
        fs::create_dir_all(dir).at(dir)?;
        let mut tmp = tempfile::Builder::new()
            .prefix(".nought-tmp-")
            .tempfile_in(dir)
            .at(dir)?;
        std::io::Write::write_all(&mut tmp, text.as_bytes()).at(tmp.path())?;
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
    blake3::hash(text.as_bytes()).to_hex()[..32].to_string()
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

fn mtime_ms(p: &Path) -> i64 {
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
        // Editing an external file through Nought keeps it frontmatter-free.
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
    fn rejects_path_escape() {
        let (_d, v) = vault();
        assert!(matches!(v.abs("../etc/passwd"), Err(Error::OutsideVault(_))));
        assert!(matches!(v.sync_path(Path::new("/etc/passwd")), Ok(Change::Unchanged)));
    }
}
