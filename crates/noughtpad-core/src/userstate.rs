//! The user's judgements about derived data (e.g. "these notes are not
//! related"). Unlike the index this is NOT disposable, and it is keyed by
//! stable note ids, so it survives index rebuilds and model changes.
//!
//! The truth is a set of append-only logs, one per device:
//! `.noughtpad/user-state/<device-id>.jsonl`. A device only ever appends to its
//! own log, so syncing the folder (Noughtpad Sync, Dropbox, Syncthing) never
//! sees two writers on one file. On open, every device's log is replayed in
//! time order into an in-memory database that answers the queries; the newest
//! event for a key wins. See `docs/DESIGN.md` §12.6.
//!
//! Before M6 this was a single SQLite file, `.noughtpad/user-state.sqlite`. It
//! is ignored now (there were no users to carry over).

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::error::IoContext;
use crate::Result;

/// Folder (inside `.noughtpad/`) holding one log per device.
pub const USER_STATE_DIR: &str = "user-state";

/// One line of a device's log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Op {
    Dismiss {
        a: String,
        b: String,
    },
    Undismiss {
        a: String,
        b: String,
    },
    Feedback {
        a: String,
        b: String,
        useful: bool,
        model: String,
        features: String,
    },
    Unfeedback {
        a: String,
        b: String,
        useful: bool,
    },
    ClearFeedback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Event {
    /// Milliseconds since the epoch, on the writing device's clock.
    at: i64,
    #[serde(flatten)]
    op: Op,
}

pub struct UserState {
    conn: Mutex<Connection>,
    /// The log folder and this device's log; `None` for in-memory test stores.
    logs: Option<(PathBuf, PathBuf)>,
}

fn pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS dismissed_relations (
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
     );";

impl UserState {
    /// Open the user-state of the notebook whose data folder is `data_dir`
    /// (`<vault>/.noughtpad`), writing as `device_id`.
    pub fn open(data_dir: &Path, device_id: &str) -> Result<Self> {
        let dir = data_dir.join(USER_STATE_DIR);
        fs::create_dir_all(&dir).at(&dir)?;
        let own = dir.join(format!("{device_id}.jsonl"));
        let state = UserState {
            conn: Mutex::new(Self::fresh()?),
            logs: Some((dir, own)),
        };
        state.reload()?;
        Ok(state)
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(UserState {
            conn: Mutex::new(Self::fresh()?),
            logs: None,
        })
    }

    fn fresh() -> Result<Connection> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(conn)
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Re-read every device's log (e.g. after sync brought in another device's
    /// changes). Events are replayed in time order; ties break by device id
    /// and then by order within a log, so every device gets the same result.
    pub fn reload(&self) -> Result<()> {
        let Some((dir, _)) = &self.logs else { return Ok(()) };
        let mut events: Vec<(i64, String, usize, Op)> = Vec::new();
        for entry in fs::read_dir(dir).at(dir)? {
            let path = entry.at(dir)?.path();
            let Some(device) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".jsonl"))
                .map(str::to_string)
            else {
                continue;
            };
            let text = fs::read_to_string(&path).at(&path)?;
            for (n, line) in text.lines().enumerate() {
                // A torn last line (crash mid-append) or a newer format is skipped.
                match serde_json::from_str::<Event>(line) {
                    Ok(e) => events.push((e.at, device.clone(), n, e.op)),
                    Err(_) if line.trim().is_empty() => {}
                    Err(e) => tracing::warn!("skipping user-state line {}:{}: {e}", path.display(), n + 1),
                }
            }
        }
        events.sort_by(|x, y| (x.0, &x.1, x.2).cmp(&(y.0, &y.1, y.2)));
        let conn = Self::fresh()?;
        for (at, _, _, op) in &events {
            apply(&conn, *at, op)?;
        }
        *self.conn() = conn;
        Ok(())
    }

    /// Append to this device's log, then apply to the in-memory view.
    fn record(&self, op: Op) -> Result<()> {
        let event = Event { at: now_ms(), op };
        if let Some((_, own)) = &self.logs {
            let mut line = serde_json::to_string(&event).expect("events serialise");
            line.push('\n');
            let mut f = OpenOptions::new().create(true).append(true).open(own).at(own)?;
            // One write per line, so a crash leaves at most one torn last line.
            f.write_all(line.as_bytes()).at(own)?;
            f.sync_data().at(own)?;
        }
        apply(&self.conn(), event.at, &event.op)
    }

    /// Record that two notes should not be shown as related (either direction).
    pub fn dismiss(&self, a: &str, b: &str) -> Result<()> {
        let (a, b) = pair(a, b);
        self.record(Op::Dismiss {
            a: a.into(),
            b: b.into(),
        })
    }

    pub fn undismiss(&self, a: &str, b: &str) -> Result<()> {
        let (a, b) = pair(a, b);
        self.record(Op::Undismiss {
            a: a.into(),
            b: b.into(),
        })
    }

    /// Record feedback on a suggestion (latest wins for the same pair and kind).
    pub fn record_feedback(&self, a: &str, b: &str, useful: bool, model: &str, features: &str) -> Result<()> {
        let (a, b) = pair(a, b);
        self.record(Op::Feedback {
            a: a.into(),
            b: b.into(),
            useful,
            model: model.into(),
            features: features.into(),
        })
    }

    pub fn remove_feedback(&self, a: &str, b: &str, useful: bool) -> Result<()> {
        let (a, b) = pair(a, b);
        self.record(Op::Unfeedback {
            a: a.into(),
            b: b.into(),
            useful,
        })
    }

    /// `(features JSON, useful)` recorded with `model`.
    pub fn feedback(&self, model: &str) -> Result<Vec<(String, bool)>> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT features, useful FROM relation_feedback WHERE model = ?1")?;
        let rows = stmt.query_map([model], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn clear_feedback(&self) -> Result<()> {
        self.record(Op::ClearFeedback)
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

fn apply(conn: &Connection, at: i64, op: &Op) -> Result<()> {
    match op {
        Op::Dismiss { a, b } => conn.execute(
            "INSERT OR REPLACE INTO dismissed_relations (a, b, at) VALUES (?1, ?2, ?3)",
            params![a, b, at],
        )?,
        Op::Undismiss { a, b } => {
            conn.execute("DELETE FROM dismissed_relations WHERE a = ?1 AND b = ?2", params![a, b])?
        }
        Op::Feedback {
            a,
            b,
            useful,
            model,
            features,
        } => conn.execute(
            "INSERT OR REPLACE INTO relation_feedback (a, b, useful, model, features, at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![a, b, useful, model, features, at],
        )?,
        Op::Unfeedback { a, b, useful } => conn.execute(
            "DELETE FROM relation_feedback WHERE a = ?1 AND b = ?2 AND useful = ?3",
            params![a, b, useful],
        )?,
        Op::ClearFeedback => conn.execute("DELETE FROM relation_feedback", [])?,
    };
    Ok(())
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

    #[test]
    fn survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        {
            let s = UserState::open(dir.path(), "DEV1").unwrap();
            s.dismiss("a", "b").unwrap();
            s.record_feedback("a", "c", true, "m", "{}").unwrap();
        }
        let s = UserState::open(dir.path(), "DEV1").unwrap();
        assert!(s.dismissed_for("a").unwrap().contains("b"));
        assert_eq!(s.feedback("m").unwrap(), vec![("{}".to_string(), true)]);
    }

    #[test]
    fn each_device_writes_only_its_own_log_and_sees_all() {
        let dir = tempfile::tempdir().unwrap();
        let mac = UserState::open(dir.path(), "MAC").unwrap();
        let pc = UserState::open(dir.path(), "PC").unwrap();
        mac.dismiss("a", "b").unwrap();
        pc.dismiss("c", "d").unwrap();
        let logs: Vec<_> = fs::read_dir(dir.path().join(USER_STATE_DIR))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(logs.len(), 2);
        let mac_log = fs::read_to_string(dir.path().join(USER_STATE_DIR).join("MAC.jsonl")).unwrap();
        assert!(mac_log.contains("\"a\"") && !mac_log.contains("\"c\""));
        mac.reload().unwrap();
        assert!(mac.dismissed_for("c").unwrap().contains("d"));
    }

    #[test]
    fn the_newest_event_wins_across_devices() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join(USER_STATE_DIR);
        fs::create_dir_all(&logs).unwrap();
        // PC dismissed at t=100; MAC undid it later at t=200, then PC dismissed again at t=300.
        fs::write(
            logs.join("PC.jsonl"),
            "{\"at\":100,\"op\":\"dismiss\",\"a\":\"a\",\"b\":\"b\"}\n{\"at\":300,\"op\":\"dismiss\",\"a\":\"x\",\"b\":\"y\"}\n",
        )
        .unwrap();
        fs::write(
            logs.join("MAC.jsonl"),
            "{\"at\":200,\"op\":\"undismiss\",\"a\":\"a\",\"b\":\"b\"}\n",
        )
        .unwrap();
        let s = UserState::open(dir.path(), "MAC").unwrap();
        assert!(s.dismissed_for("a").unwrap().is_empty());
        assert!(s.dismissed_for("x").unwrap().contains("y"));
    }

    #[test]
    fn a_torn_last_line_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join(USER_STATE_DIR);
        fs::create_dir_all(&logs).unwrap();
        fs::write(
            logs.join("DEV1.jsonl"),
            "{\"at\":1,\"op\":\"dismiss\",\"a\":\"a\",\"b\":\"b\"}\n{\"at\":2,\"op\":\"dism",
        )
        .unwrap();
        let s = UserState::open(dir.path(), "DEV1").unwrap();
        assert!(s.dismissed_for("a").unwrap().contains("b"));
    }

    #[test]
    fn clearing_feedback_keeps_dismissals() {
        let dir = tempfile::tempdir().unwrap();
        let s = UserState::open(dir.path(), "DEV1").unwrap();
        s.dismiss("a", "b").unwrap();
        s.record_feedback("a", "c", true, "m", "{}").unwrap();
        s.clear_feedback().unwrap();
        let s = UserState::open(dir.path(), "DEV1").unwrap();
        assert!(s.feedback("m").unwrap().is_empty());
        assert!(s.dismissed_for("a").unwrap().contains("b"));
    }
}
