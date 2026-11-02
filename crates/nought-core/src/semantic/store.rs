//! Persistence for chunks and vectors, in the same (disposable) index file.
//!
//! Vectors are cached by `(model, hash of chunk text)`, so rebuilding the
//! notes index, renaming files or re-saving an unchanged note never re-runs
//! the model.

use rusqlite::{params, Connection, OptionalExtension};

use super::chunk::Chunk;
use crate::Result;

pub(crate) fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chunks (
             note_id   TEXT NOT NULL,
             ord       INTEGER NOT NULL,
             text      TEXT NOT NULL,
             text_hash TEXT NOT NULL,
             PRIMARY KEY (note_id, ord)
         );
         CREATE TABLE IF NOT EXISTS embed_state (
             note_id TEXT NOT NULL,
             model   TEXT NOT NULL,
             hash    TEXT NOT NULL,
             PRIMARY KEY (note_id, model)
         );
         CREATE TABLE IF NOT EXISTS vec_cache (
             model     TEXT NOT NULL,
             text_hash TEXT NOT NULL,
             vec       BLOB NOT NULL,
             PRIMARY KEY (model, text_hash)
         );",
    )?;
    Ok(())
}

pub(crate) struct PendingNote {
    pub id: String,
    pub hash: String,
    pub title: Option<String>,
    pub body: String,
}

/// Notes whose vectors are missing or stale, newest first. Notes modified
/// after `quiet_before_ms` are skipped so we don't embed while someone types.
pub(crate) fn pending(conn: &Connection, model: &str, quiet_before_ms: i64, limit: usize) -> Result<Vec<PendingNote>> {
    let mut stmt = conn.prepare(
        "SELECT n.id, n.hash, CASE WHEN n.has_title THEN n.title END, n.body
         FROM notes n LEFT JOIN embed_state e ON e.note_id = n.id AND e.model = ?1
         WHERE (e.hash IS NULL OR e.hash != n.hash) AND n.modified_ms < ?2
         ORDER BY n.created_ms DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![model, quiet_before_ms, limit as i64], |r| {
        Ok(PendingNote {
            id: r.get(0)?,
            hash: r.get(1)?,
            title: r.get(2)?,
            body: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// `(embedded, total)` notes for progress reporting.
pub(crate) fn progress(conn: &Connection, model: &str) -> Result<(i64, i64)> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?;
    let done: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notes n JOIN embed_state e ON e.note_id = n.id AND e.model = ?1 AND e.hash = n.hash",
        [model],
        |r| r.get(0),
    )?;
    Ok((done, total))
}

pub(crate) fn cached(conn: &Connection, model: &str, text_hash: &str) -> Result<Option<Vec<f32>>> {
    Ok(conn
        .query_row(
            "SELECT vec FROM vec_cache WHERE model = ?1 AND text_hash = ?2",
            params![model, text_hash],
            |r| r.get::<_, Vec<u8>>(0),
        )
        .optional()?
        .map(|b| from_blob(&b)))
}

/// Replace a note's chunks and record its vectors (all in one transaction).
pub(crate) fn save(
    conn: &mut Connection,
    model: &str,
    note_id: &str,
    note_hash: &str,
    chunks: &[(Chunk, String, Vec<f32>)],
) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM chunks WHERE note_id = ?1", [note_id])?;
    for (c, text_hash, vec) in chunks {
        tx.execute(
            "INSERT INTO chunks (note_id, ord, text, text_hash) VALUES (?1, ?2, ?3, ?4)",
            params![note_id, c.ord, c.text, text_hash],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO vec_cache (model, text_hash, vec) VALUES (?1, ?2, ?3)",
            params![model, text_hash, to_blob(vec)],
        )?;
    }
    tx.execute(
        "INSERT OR REPLACE INTO embed_state (note_id, model, hash) VALUES (?1, ?2, ?3)",
        params![note_id, model, note_hash],
    )?;
    tx.commit()?;
    Ok(())
}

/// Every embedded chunk for a model: `(note_id, ord, vector)`.
pub(crate) fn load_all(conn: &Connection, model: &str) -> Result<Vec<(String, u32, Vec<f32>)>> {
    let mut stmt = conn.prepare(
        "SELECT c.note_id, c.ord, v.vec FROM chunks c
         JOIN embed_state e ON e.note_id = c.note_id AND e.model = ?1
         JOIN notes n ON n.id = c.note_id
         JOIN vec_cache v ON v.model = ?1 AND v.text_hash = c.text_hash
         ORDER BY c.note_id, c.ord",
    )?;
    let rows = stmt.query_map([model], |r| {
        Ok((r.get(0)?, r.get(1)?, from_blob(&r.get::<_, Vec<u8>>(2)?)))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn chunk_text(conn: &Connection, note_id: &str, ord: u32) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT text FROM chunks WHERE note_id = ?1 AND ord = ?2",
            params![note_id, ord],
            |r| r.get(0),
        )
        .optional()?)
}

/// Remove chunks of deleted notes and vectors nothing refers to.
/// Returns the ids of notes that disappeared.
pub(crate) fn sweep(conn: &mut Connection) -> Result<Vec<String>> {
    let tx = conn.transaction()?;
    let gone: Vec<String> = {
        let mut stmt =
            tx.prepare("SELECT DISTINCT note_id FROM embed_state WHERE note_id NOT IN (SELECT id FROM notes)")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    tx.execute("DELETE FROM chunks WHERE note_id NOT IN (SELECT id FROM notes)", [])?;
    tx.execute(
        "DELETE FROM embed_state WHERE note_id NOT IN (SELECT id FROM notes)",
        [],
    )?;
    tx.execute(
        "DELETE FROM vec_cache WHERE NOT EXISTS
            (SELECT 1 FROM chunks c WHERE c.text_hash = vec_cache.text_hash)",
        [],
    )?;
    tx.commit()?;
    Ok(gone)
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_blob(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
