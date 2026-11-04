//! The change journal: notebook paths this device changed since they were
//! last synced, so a sync only looks at what changed. Fed by the watcher
//! (which sees the app's own writes and other tools' alike).
//!
//! A recorded path that no longer exists may have been a folder (the watcher
//! can't tell after the fact), so the engine treats it as a path prefix too.
//!
//! It is a shortcut, not the truth: the engine also compares every file's hash
//! with its base on start and every so often, which catches anything written
//! while the app was closed. Losing the journal costs time, never data.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use super::is_synced;
use crate::Result;

pub const JOURNAL_FILE: &str = "journal.sqlite";

pub struct Journal {
    conn: Mutex<Connection>,
}

/// A journal entry: the path and the order it was last recorded in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub path: String,
    pub seq: i64,
}

impl Journal {
    pub fn open(path: &Path) -> Result<Journal> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| crate::Error::Io {
                path: dir.to_path_buf(),
                source: e,
            })?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Journal> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Journal> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS journal (
                 path TEXT PRIMARY KEY,
                 seq  INTEGER NOT NULL
             );",
        )?;
        Ok(Journal { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Note that these paths changed. Paths that don't sync are ignored.
    pub fn record<'a>(&self, paths: impl IntoIterator<Item = &'a str>) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for p in paths.into_iter().filter(|p| is_synced(p)) {
            tx.execute(
                "INSERT INTO journal (path, seq)
                 VALUES (?1, (SELECT COALESCE(MAX(seq), 0) + 1 FROM journal))
                 ON CONFLICT(path) DO UPDATE SET seq = excluded.seq",
                params![p],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Everything changed since last synced, oldest first.
    pub fn pending(&self) -> Result<Vec<Pending>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT path, seq FROM journal ORDER BY seq")?;
        let rows = stmt.query_map([], |r| {
            Ok(Pending {
                path: r.get(0)?,
                seq: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// The engine synced `p`. If the path changed again since `pending` read
    /// it (a newer seq), the entry stays, so that change isn't lost.
    pub fn done(&self, p: &Pending) -> Result<()> {
        self.conn().execute(
            "DELETE FROM journal WHERE path = ?1 AND seq <= ?2",
            params![p.path, p.seq],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_synced_paths_once_in_order() {
        let j = Journal::open_in_memory().unwrap();
        j.record(["b.md", ".noughtpad/index.sqlite", "a.md", "b.md"]).unwrap();
        let p: Vec<_> = j.pending().unwrap().into_iter().map(|p| p.path).collect();
        assert_eq!(p, vec!["a.md", "b.md"]);
    }

    #[test]
    fn a_change_during_sync_is_kept() {
        let j = Journal::open_in_memory().unwrap();
        j.record(["a.md", "b.md"]).unwrap();
        let seen = j.pending().unwrap();
        // While syncing, a.md changes again.
        j.record(["a.md"]).unwrap();
        for p in &seen {
            j.done(p).unwrap();
        }
        let left: Vec<_> = j.pending().unwrap().into_iter().map(|p| p.path).collect();
        assert_eq!(left, vec!["a.md"]);
    }

    #[test]
    fn survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync").join(JOURNAL_FILE);
        Journal::open(&path).unwrap().record(["a.md"]).unwrap();
        assert_eq!(Journal::open(&path).unwrap().pending().unwrap().len(), 1);
    }
}
