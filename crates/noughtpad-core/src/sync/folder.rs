//! A [`SyncBackend`] over a plain directory. Used by the engine's tests (two
//! or three simulated devices sharing one folder) and for trying sync between
//! two notebooks on one machine.
//!
//! Layout under the backend's root:
//!
//! ```text
//! <notebook name>/
//!   notebook.json    {"id": ULID, "format": 1}
//!   files/<path>     the notebook's files at their notebook-relative paths
//!   changes.jsonl    one line per write or removal: {"seq", "path", "rev", "modified", "size", "removed"}
//!   lock             held while writing (create-new), so writes from several
//!                    processes don't interleave
//! ```
//!
//! A revision is the content hash, so the "only if still at `expected`" rule
//! is a hash comparison under the lock.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{ChangePage, Cursor, Put, RemoteEntry, RemoteNotebook, Rev, SyncBackend};
use crate::error::IoContext;
use crate::vault::hash_bytes;
use crate::{Error, Result};

const NOTEBOOK_FILE: &str = "notebook.json";
const FILES: &str = "files";
const CHANGES: &str = "changes.jsonl";
const LOCK: &str = "lock";
const PAGE: usize = 500;

pub struct FolderBackend {
    root: PathBuf,
}

#[derive(Serialize, Deserialize)]
struct NotebookFile {
    id: String,
    format: u32,
}

#[derive(Serialize, Deserialize)]
struct ChangeLine {
    seq: u64,
    path: String,
    rev: String,
    modified: i64,
    size: u64,
    removed: bool,
}

/// Held while writing to a notebook; removed on drop.
struct Lock(PathBuf);

impl Lock {
    fn take(dir: &Path) -> Result<Lock> {
        let path = dir.join(LOCK);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Lock(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(Error::Sync(format!("{} is locked", dir.display())))
                }
                Err(e) => return Err(Error::Io { path, source: e }),
            }
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn check_path(path: &str) -> Result<()> {
    let p = Path::new(path);
    if path.is_empty() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(Error::OutsideVault(p.to_path_buf()));
    }
    Ok(())
}

impl FolderBackend {
    pub fn new(root: impl Into<PathBuf>) -> FolderBackend {
        FolderBackend { root: root.into() }
    }

    fn dir(&self, nb: &RemoteNotebook) -> PathBuf {
        self.root.join(&nb.handle)
    }

    fn read_changes(&self, dir: &Path) -> Result<Vec<ChangeLine>> {
        let path = dir.join(CHANGES);
        match fs::read_to_string(&path) {
            Ok(text) => Ok(text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(Error::Io { path, source: e }),
        }
    }

    fn append_change(&self, dir: &Path, mut line: ChangeLine) -> Result<()> {
        line.seq = self.read_changes(dir)?.last().map_or(1, |l| l.seq + 1);
        let path = dir.join(CHANGES);
        let mut text = serde_json::to_string(&line).expect("change serialises");
        text.push('\n');
        let mut f = OpenOptions::new().create(true).append(true).open(&path).at(&path)?;
        f.write_all(text.as_bytes()).at(&path)?;
        f.sync_data().at(&path)?;
        Ok(())
    }

    /// The current revision of a file, or `None` if it doesn't exist.
    fn current(&self, file: &Path) -> Result<Option<Rev>> {
        match fs::read(file) {
            Ok(bytes) => Ok(Some(Rev(hash_bytes(&bytes)))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Io {
                path: file.to_path_buf(),
                source: e,
            }),
        }
    }
}

impl SyncBackend for FolderBackend {
    fn kind(&self) -> &'static str {
        "folder"
    }

    fn open_notebook(&self, name: &str) -> Result<RemoteNotebook> {
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            return Err(Error::Sync(format!("not a usable notebook name: {name:?}")));
        }
        let dir = self.root.join(name);
        fs::create_dir_all(dir.join(FILES)).at(&dir)?;
        let _lock = Lock::take(&dir)?;
        let file = dir.join(NOTEBOOK_FILE);
        let id = match fs::read_to_string(&file)
            .ok()
            .and_then(|s| serde_json::from_str::<NotebookFile>(&s).ok())
        {
            Some(nb) => nb.id,
            None => {
                let nb = NotebookFile {
                    id: ulid::Ulid::new().to_string(),
                    format: 1,
                };
                fs::write(&file, serde_json::to_string(&nb).expect("notebook serialises")).at(&file)?;
                nb.id
            }
        };
        Ok(RemoteNotebook {
            id,
            name: name.to_string(),
            handle: name.to_string(),
        })
    }

    fn list(&self, nb: &RemoteNotebook) -> Result<Vec<RemoteEntry>> {
        let files = self.dir(nb).join(FILES);
        let mut out = Vec::new();
        for entry in walkdir::WalkDir::new(&files).sort_by_file_name() {
            let entry = entry.map_err(|e| Error::Sync(e.to_string()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&files)
                .expect("walk stays under its root")
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let bytes = fs::read(entry.path()).at(entry.path())?;
            let hash = hash_bytes(&bytes);
            out.push(RemoteEntry {
                path: rel,
                rev: Rev(hash.clone()),
                hash: Some(hash),
                size: bytes.len() as u64,
                modified: crate::vault::mtime_ms(entry.path()),
                removed: false,
            });
        }
        Ok(out)
    }

    fn changes(&self, nb: &RemoteNotebook, cursor: Option<&Cursor>) -> Result<ChangePage> {
        let lines = self.read_changes(&self.dir(nb))?;
        let last = lines.last().map_or(0, |l| l.seq);
        let Some(cursor) = cursor else {
            return Ok(ChangePage {
                entries: Vec::new(),
                cursor: Cursor(last.to_string()),
                more: false,
            });
        };
        let from: u64 = cursor
            .0
            .parse()
            .map_err(|_| Error::Sync(format!("bad cursor {:?}", cursor.0)))?;
        let newer: Vec<&ChangeLine> = lines.iter().filter(|l| l.seq > from).collect();
        let page = &newer[..newer.len().min(PAGE)];
        let mut latest: BTreeMap<&str, &ChangeLine> = BTreeMap::new();
        for l in page {
            latest.insert(&l.path, l);
        }
        Ok(ChangePage {
            entries: latest
                .into_values()
                .map(|l| RemoteEntry {
                    path: l.path.clone(),
                    rev: Rev(l.rev.clone()),
                    hash: (!l.removed).then(|| l.rev.clone()),
                    size: l.size,
                    modified: l.modified,
                    removed: l.removed,
                })
                .collect(),
            cursor: Cursor(page.last().map_or(from, |l| l.seq).to_string()),
            more: newer.len() > page.len(),
        })
    }

    fn get(&self, nb: &RemoteNotebook, path: &str) -> Result<(Vec<u8>, Rev)> {
        check_path(path)?;
        let file = self.dir(nb).join(FILES).join(path);
        match fs::read(&file) {
            Ok(bytes) => {
                let rev = Rev(hash_bytes(&bytes));
                Ok((bytes, rev))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::NotFound(path.to_string())),
            Err(e) => Err(Error::Io { path: file, source: e }),
        }
    }

    fn put(&self, nb: &RemoteNotebook, path: &str, bytes: &[u8], modified: i64, expected: Option<&Rev>) -> Result<Put> {
        check_path(path)?;
        let dir = self.dir(nb);
        let _lock = Lock::take(&dir)?;
        let file = dir.join(FILES).join(path);
        let current = self.current(&file)?;
        if current.as_ref() != expected {
            return Ok(Put::Moved(current));
        }
        let parent = file.parent().expect("file has a parent");
        fs::create_dir_all(parent).at(parent)?;
        let mut tmp = tempfile::NamedTempFile::new_in(parent).at(parent)?;
        tmp.write_all(bytes).at(tmp.path())?;
        tmp.as_file().sync_all().at(&file)?;
        tmp.persist(&file).map_err(|e| Error::Io {
            path: file.clone(),
            source: e.error,
        })?;
        let rev = Rev(hash_bytes(bytes));
        self.append_change(
            &dir,
            ChangeLine {
                seq: 0,
                path: path.to_string(),
                rev: rev.0.clone(),
                modified,
                size: bytes.len() as u64,
                removed: false,
            },
        )?;
        Ok(Put::Written(rev))
    }

    fn remove(&self, nb: &RemoteNotebook, path: &str, expected: &Rev) -> Result<Put> {
        check_path(path)?;
        let dir = self.dir(nb);
        let _lock = Lock::take(&dir)?;
        let file = dir.join(FILES).join(path);
        let current = self.current(&file)?;
        if current.as_ref() != Some(expected) {
            return Ok(Put::Moved(current));
        }
        fs::remove_file(&file).at(&file)?;
        self.append_change(
            &dir,
            ChangeLine {
                seq: 0,
                path: path.to_string(),
                rev: expected.0.clone(),
                modified: chrono::Utc::now().timestamp_millis(),
                size: 0,
                removed: true,
            },
        )?;
        Ok(Put::Written(expected.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, FolderBackend, RemoteNotebook) {
        let dir = tempfile::tempdir().unwrap();
        let b = FolderBackend::new(dir.path());
        let nb = b.open_notebook("Notes").unwrap();
        (dir, b, nb)
    }

    #[test]
    fn a_notebook_keeps_its_id() {
        let (_d, b, nb) = setup();
        assert_eq!(b.open_notebook("Notes").unwrap().id, nb.id);
        assert_ne!(b.open_notebook("Other").unwrap().id, nb.id);
        assert!(b.open_notebook("../escape").is_err());
    }

    #[test]
    fn writes_only_when_still_at_the_expected_revision() {
        let (_d, b, nb) = setup();
        let Put::Written(r1) = b.put(&nb, "2026/a.md", b"one", 1, None).unwrap() else {
            panic!()
        };
        // Creating again (expecting no file) is refused and reports the current revision.
        assert_eq!(
            b.put(&nb, "2026/a.md", b"other", 1, None).unwrap(),
            Put::Moved(Some(r1.clone()))
        );
        let Put::Written(r2) = b.put(&nb, "2026/a.md", b"two", 2, Some(&r1)).unwrap() else {
            panic!()
        };
        // A device that still thinks it's at r1 is told someone moved on.
        assert_eq!(
            b.put(&nb, "2026/a.md", b"stale", 3, Some(&r1)).unwrap(),
            Put::Moved(Some(r2.clone()))
        );
        assert_eq!(b.remove(&nb, "2026/a.md", &r1).unwrap(), Put::Moved(Some(r2.clone())));
        assert_eq!(b.get(&nb, "2026/a.md").unwrap(), (b"two".to_vec(), r2.clone()));
        assert_eq!(b.remove(&nb, "2026/a.md", &r2).unwrap(), Put::Written(r2.clone()));
        assert!(matches!(b.get(&nb, "2026/a.md"), Err(Error::NotFound(_))));
        assert_eq!(b.put(&nb, "2026/a.md", b"x", 4, Some(&r2)).unwrap(), Put::Moved(None));
    }

    #[test]
    fn list_and_change_feed() {
        let (_d, b, nb) = setup();
        let start = b.changes(&nb, None).unwrap();
        assert!(start.entries.is_empty() && !start.more);
        b.put(&nb, "a.md", b"a", 1, None).unwrap();
        let Put::Written(rb) = b.put(&nb, "img/b.png", b"\x89PNG", 1, None).unwrap() else {
            panic!()
        };
        b.put(&nb, "a.md", b"a2", 2, Some(&Rev(hash_bytes(b"a")))).unwrap();
        b.remove(&nb, "img/b.png", &rb).unwrap();

        let page = b.changes(&nb, Some(&start.cursor)).unwrap();
        let summary: Vec<_> = page.entries.iter().map(|e| (e.path.as_str(), e.removed)).collect();
        assert_eq!(summary, vec![("a.md", false), ("img/b.png", true)]);
        assert_eq!(page.entries[0].hash.as_deref(), Some(hash_bytes(b"a2").as_str()));
        // Nothing new since.
        assert!(b.changes(&nb, Some(&page.cursor)).unwrap().entries.is_empty());

        let listed: Vec<_> = b.list(&nb).unwrap().into_iter().map(|e| e.path).collect();
        assert_eq!(listed, vec!["a.md"]);
    }

    #[test]
    fn the_feed_pages() {
        let (_d, b, nb) = setup();
        let start = b.changes(&nb, None).unwrap().cursor;
        for i in 0..(PAGE + 3) {
            b.put(&nb, &format!("n{i}.md"), b"x", 1, None).unwrap();
        }
        let first = b.changes(&nb, Some(&start)).unwrap();
        assert_eq!((first.entries.len(), first.more), (PAGE, true));
        let second = b.changes(&nb, Some(&first.cursor)).unwrap();
        assert_eq!((second.entries.len(), second.more), (3, false));
    }

    #[test]
    fn refuses_paths_outside_the_notebook() {
        let (_d, b, nb) = setup();
        assert!(b.put(&nb, "../x.md", b"x", 1, None).is_err());
        assert!(b.get(&nb, "/etc/passwd").is_err());
    }

    #[test]
    fn concurrent_writers_never_both_win() {
        let (_d, b, nb) = setup();
        let b = std::sync::Arc::new(b);
        let Put::Written(base) = b.put(&nb, "a.md", b"base", 1, None).unwrap() else {
            panic!()
        };
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let (b, nb, base) = (b.clone(), nb.clone(), base.clone());
                std::thread::spawn(move || {
                    matches!(
                        b.put(&nb, "a.md", format!("device {i}").as_bytes(), 2, Some(&base))
                            .unwrap(),
                        Put::Written(_)
                    )
                })
            })
            .collect();
        let winners = handles.into_iter().map(|h| h.join().unwrap()).filter(|w| *w).count();
        assert_eq!(winners, 1);
    }
}
