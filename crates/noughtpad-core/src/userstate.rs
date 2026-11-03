//! The user's judgements about derived data (e.g. "these notes are not
//! related"). Unlike the index this is NOT disposable: it lives in its own
//! file, `.noughtpad/user-state.sqlite`, keyed by stable note ids, and survives
//! index rebuilds and model changes.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::Result;

pub const USER_STATE_FILE: &str = "user-state.sqlite";

pub struct UserState {
    conn: Mutex<Connection>,
}

fn pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

impl UserState {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS dismissed_relations (
                 a  TEXT NOT NULL,
                 b  TEXT NOT NULL,
                 at INTEGER NOT NULL,
                 PRIMARY KEY (a, b)
             );
             -- Implicit feedback on related-note suggestions, with the pair's
             -- features at that moment (so learning survives index rebuilds).
             CREATE TABLE IF NOT EXISTS relation_feedback (
                 a        TEXT NOT NULL,
                 b        TEXT NOT NULL,
                 useful   INTEGER NOT NULL,
                 model    TEXT NOT NULL,
                 features TEXT NOT NULL,
                 at       INTEGER NOT NULL,
                 PRIMARY KEY (a, b, useful)
             );",
        )?;
        Ok(UserState { conn: Mutex::new(conn) })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Record that two notes should not be shown as related (either direction).
    pub fn dismiss(&self, a: &str, b: &str) -> Result<()> {
        let (a, b) = pair(a, b);
        self.conn().execute(
            "INSERT OR REPLACE INTO dismissed_relations (a, b, at) VALUES (?1, ?2, ?3)",
            params![a, b, chrono::Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn undismiss(&self, a: &str, b: &str) -> Result<()> {
        let (a, b) = pair(a, b);
        self.conn()
            .execute("DELETE FROM dismissed_relations WHERE a = ?1 AND b = ?2", params![a, b])?;
        Ok(())
    }

    /// Record feedback on a suggestion (latest wins for the same pair and kind).
    pub fn record_feedback(&self, a: &str, b: &str, useful: bool, model: &str, features: &str) -> Result<()> {
        let (a, b) = pair(a, b);
        self.conn().execute(
            "INSERT OR REPLACE INTO relation_feedback (a, b, useful, model, features, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![a, b, useful, model, features, chrono::Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn remove_feedback(&self, a: &str, b: &str, useful: bool) -> Result<()> {
        let (a, b) = pair(a, b);
        self.conn().execute(
            "DELETE FROM relation_feedback WHERE a = ?1 AND b = ?2 AND useful = ?3",
            params![a, b, useful],
        )?;
        Ok(())
    }

    /// `(features JSON, useful)` recorded with `model`.
    pub fn feedback(&self, model: &str) -> Result<Vec<(String, bool)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT features, useful FROM relation_feedback WHERE model = ?1")?;
        let rows = stmt.query_map([model], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn clear_feedback(&self) -> Result<()> {
        self.conn().execute("DELETE FROM relation_feedback", [])?;
        Ok(())
    }

    /// Notes the user dismissed as unrelated to `id`.
    pub fn dismissed_for(&self, id: &str) -> Result<HashSet<String>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT CASE WHEN a = ?1 THEN b ELSE a END FROM dismissed_relations WHERE a = ?1 OR b = ?1")?;
        let rows = stmt.query_map([id], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dismissals_are_symmetric_and_reversible() {
        let s = UserState::open_in_memory().unwrap();
        s.dismiss("b", "a").unwrap();
        assert!(s.dismissed_for("a").unwrap().contains("b"));
        assert!(s.dismissed_for("b").unwrap().contains("a"));
        s.undismiss("a", "b").unwrap();
        assert!(s.dismissed_for("a").unwrap().is_empty());
    }
}
