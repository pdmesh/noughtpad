//! The derived, disposable index (`.nought/index.sqlite`).
//!
//! Everything in here can be rebuilt from the Markdown vault. User judgements
//! about derived data (dismissed connections etc.) will live in a separate
//! store — never here.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::note::{NoteKind, Source};
use crate::Result;

const SCHEMA_VERSION: i64 = 3;
const EXCERPT_CHARS: usize = 320;

/// What the vault hands to the index for one note.
#[derive(Debug, Clone)]
pub struct IndexedNote {
    pub id: String,
    pub path: String,
    pub created: DateTime<FixedOffset>,
    pub modified_ms: i64,
    pub kind: NoteKind,
    pub title: String,
    /// True when `title` is an explicit frontmatter title, not the first line.
    pub has_title: bool,
    pub body: String,
    pub labels: Vec<String>,
    pub source: Option<Source>,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub path: String,
    pub title: String,
    pub has_title: bool,
    pub excerpt: String,
    pub created: String,
    pub day: String,
    pub kind: NoteKind,
    pub labels: Vec<String>,
    pub source: Option<Source>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub note: NoteSummary,
    /// Snippet with matches wrapped in U+0002 … U+0003 (the UI renders them).
    pub snippet: String,
    pub score: f64,
    /// Whether the note matched the words, the meaning, or both.
    pub matched: MatchKind,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MatchKind {
    Text,
    Meaning,
    Both,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DaySummary {
    pub day: String,
    pub notes: Vec<NoteSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonthLog {
    pub month: String,
    pub personal_count: i64,
    pub source_count: i64,
    pub active_days: i64,
    pub top_labels: Vec<LabelCount>,
    pub days: Vec<DaySummary>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ListQuery {
    pub label: Option<String>,
    pub kind: Option<NoteKind>,
    /// Inclusive `YYYY-MM-DD` bounds on the note's local creation day.
    pub from_day: Option<String>,
    pub to_day: Option<String>,
    /// Keyset pagination: only notes created strictly before this instant (ms).
    pub before_ms: Option<i64>,
    pub limit: Option<u32>,
}

pub struct Index {
    conn: Connection,
}

impl Index {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != SCHEMA_VERSION {
            // The index is disposable: on any schema change, drop and rebuild.
            conn.execute_batch(
                "DROP TABLE IF EXISTS note_labels;
                 DROP TABLE IF EXISTS notes_fts;
                 DROP TABLE IF EXISTS notes;",
            )?;
            conn.execute_batch(SCHEMA)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Index { conn })
    }

    /// Insert or replace a note and its labels and FTS row.
    pub fn upsert(&mut self, n: &IndexedNote) -> Result<()> {
        self.upsert_many(std::slice::from_ref(n))
    }

    /// Upsert several notes in one transaction (much faster for scans).
    pub fn upsert_many(&mut self, notes: &[IndexedNote]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for n in notes {
            upsert_in(&tx, n)?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn upsert_in(tx: &rusqlite::Transaction, n: &IndexedNote) -> Result<()> {
    // A note may have moved (renamed file) or its path may have been reused.
    // FTS rows share the note's rowid, so every delete here is an index lookup.
    for (sql, key) in [
        ("SELECT rowid FROM notes WHERE id = ?1", &n.id),
        ("SELECT rowid FROM notes WHERE path = ?1", &n.path),
    ] {
        if let Some(rowid) = tx.query_row(sql, [key], |r| r.get::<_, i64>(0)).optional()? {
            delete_rowid(tx, rowid)?;
        }
    }
    let (src_title, src_url) = match &n.source {
        Some(s) => (s.title.clone(), s.url.clone()),
        None => (None, None),
    };
    tx.execute(
        "INSERT INTO notes (id, path, created, created_ms, day, modified_ms, kind, title, body,
                            source_title, source_url, hash, has_title)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            n.id,
            n.path,
            n.created.to_rfc3339(),
            n.created.timestamp_millis(),
            n.created.format("%Y-%m-%d").to_string(),
            n.modified_ms,
            n.kind.as_str(),
            n.title,
            n.body,
            src_title,
            src_url,
            n.hash,
            n.has_title,
        ],
    )?;
    // Capture before inserting labels, which have rowids of their own.
    let rowid = tx.last_insert_rowid();
    for (pos, label) in n.labels.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO note_labels (note_id, label, label_key, pos) VALUES (?1, ?2, ?3, ?4)",
            params![n.id, label, label.to_lowercase(), pos as i64],
        )?;
    }
    let source_text = [src_title.as_deref(), src_url.as_deref()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
    tx.execute(
        "INSERT INTO notes_fts (rowid, note_id, title, body, source, labels) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![rowid, n.id, n.title, n.body, source_text, n.labels.join(" ")],
    )?;
    Ok(())
}

impl Index {
    /// Raw connection for sibling modules (the semantic store shares this file).
    pub(crate) fn conn(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn remove_path(&mut self, path: &str) -> Result<bool> {
        let tx = self.conn.transaction()?;
        let rowid: Option<i64> = tx
            .query_row("SELECT rowid FROM notes WHERE path = ?1", [path], |r| r.get(0))
            .optional()?;
        if let Some(rowid) = rowid {
            delete_rowid(&tx, rowid)?;
        }
        tx.commit()?;
        Ok(rowid.is_some())
    }

    /// `path -> (hash, modified_ms)` for incremental rescans.
    pub fn fingerprints(&self) -> Result<HashMap<String, (String, i64)>> {
        let mut stmt = self.conn.prepare("SELECT path, hash, modified_ms FROM notes")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn path_of(&self, id: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT path FROM notes WHERE id = ?1", [id], |r| r.get(0))
            .optional()?)
    }

    pub fn hash_of_path(&self, path: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT hash FROM notes WHERE path = ?1", [path], |r| r.get(0))
            .optional()?)
    }

    pub fn count(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?)
    }

    pub fn get(&self, id: &str) -> Result<Option<NoteSummary>> {
        let sql = format!("SELECT {SUMMARY_COLS} FROM notes n WHERE n.id = ?1");
        Ok(self.conn.query_row(&sql, [id], summary_from_row).optional()?)
    }

    /// Chronological stream, newest first.
    pub fn list(&self, q: &ListQuery) -> Result<Vec<NoteSummary>> {
        let mut where_ = Vec::new();
        let mut args: Vec<rusqlite::types::Value> = Vec::new();
        push_filters(q, &mut where_, &mut args);
        let where_sql = if where_.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_.join(" AND "))
        };
        let limit = q.limit.unwrap_or(100).min(1000);
        let sql = format!(
            "SELECT {SUMMARY_COLS} FROM notes n {where_sql} ORDER BY n.created_ms DESC, n.id DESC LIMIT {limit}"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args), summary_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Full-text search. Supports `label:VC` / `#VC`, `type:personal|source`,
    /// `"exact phrases"`, and prefix matching on the last word.
    pub fn search(&self, input: &str, limit: u32) -> Result<Vec<SearchHit>> {
        let parsed = parse_query(input);
        let q = ListQuery {
            label: parsed.label.clone(),
            kind: parsed.kind,
            limit: Some(limit),
            ..Default::default()
        };
        let Some(fts) = parsed.fts else {
            // Filters only: behave like a filtered list.
            return Ok(self
                .list(&q)?
                .into_iter()
                .map(|note| SearchHit {
                    matched: MatchKind::Text,
                    snippet: note.excerpt.clone(),
                    note,
                    score: 0.0,
                })
                .collect());
        };
        let mut where_ = vec!["notes_fts MATCH ?".to_string()];
        let mut args: Vec<rusqlite::types::Value> = vec![fts.into()];
        push_filters(&q, &mut where_, &mut args);
        let sql = format!(
            "SELECT {SUMMARY_COLS},
                    snippet(notes_fts, 2, char(2), char(3), '…', 16) AS snip,
                    bm25(notes_fts, 0.0, 5.0, 1.0, 2.0, 3.0) AS rank
             FROM notes_fts JOIN notes n ON n.rowid = notes_fts.rowid
             WHERE {}
             ORDER BY rank ASC, n.created_ms DESC
             LIMIT {}",
            where_.join(" AND "),
            limit.min(500)
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args), |r| {
            let note = summary_from_row(r)?;
            let snip: String = r.get("snip")?;
            let rank: f64 = r.get("rank")?;
            // If the match was only in title/labels/source the body snippet has no marks.
            let snippet = if snip.contains('\u{2}') {
                snip
            } else {
                note.excerpt.clone()
            };
            Ok(SearchHit {
                matched: MatchKind::Text,
                note,
                snippet,
                score: -rank,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// All labels with counts; most used first.
    pub fn labels(&self) -> Result<Vec<LabelCount>> {
        let mut stmt = self.conn.prepare(
            "SELECT MIN(label), COUNT(*) AS c FROM note_labels GROUP BY label_key ORDER BY c DESC, label_key ASC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(LabelCount {
                label: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Notes grouped by local day, newest day first.
    pub fn days(&self, q: &ListQuery) -> Result<Vec<DaySummary>> {
        let mut q = q.clone();
        q.limit = Some(q.limit.unwrap_or(1000));
        Ok(group_by_day(self.list(&q)?))
    }

    /// A deterministic monthly log over personal notes (`month` = `YYYY-MM`).
    pub fn month_log(&self, month: &str) -> Result<MonthLog> {
        let from = format!("{month}-01");
        let to = format!("{month}-31");
        let counts = |kind: NoteKind| -> Result<i64> {
            Ok(self.conn.query_row(
                "SELECT COUNT(*) FROM notes WHERE day BETWEEN ?1 AND ?2 AND kind = ?3",
                params![from, to, kind.as_str()],
                |r| r.get(0),
            )?)
        };
        let personal_count = counts(NoteKind::Personal)?;
        let source_count = counts(NoteKind::Source)?;
        let active_days = self.conn.query_row(
            "SELECT COUNT(DISTINCT day) FROM notes WHERE day BETWEEN ?1 AND ?2",
            params![from, to],
            |r| r.get(0),
        )?;
        let mut stmt = self.conn.prepare(
            "SELECT MIN(l.label), COUNT(*) AS c FROM note_labels l JOIN notes n ON n.id = l.note_id
             WHERE n.day BETWEEN ?1 AND ?2 GROUP BY l.label_key ORDER BY c DESC, l.label_key LIMIT 8",
        )?;
        let top_labels = stmt
            .query_map(params![from, to], |r| {
                Ok(LabelCount {
                    label: r.get(0)?,
                    count: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let days = self.days(&ListQuery {
            kind: Some(NoteKind::Personal),
            from_day: Some(from.clone()),
            to_day: Some(to.clone()),
            ..Default::default()
        })?;
        Ok(MonthLog {
            month: month.to_string(),
            personal_count,
            source_count,
            active_days,
            top_labels,
            days,
        })
    }

    /// Distinct months that have notes, newest first (`YYYY-MM`).
    pub fn months(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT substr(day, 1, 7) AS m FROM notes ORDER BY m DESC")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn clear(&mut self) -> Result<()> {
        self.conn
            .execute_batch("DELETE FROM notes_fts; DELETE FROM note_labels; DELETE FROM notes;")?;
        Ok(())
    }
}

const SCHEMA: &str = "
CREATE TABLE notes (
    rowid        INTEGER PRIMARY KEY,
    id           TEXT NOT NULL UNIQUE,
    path         TEXT NOT NULL UNIQUE,
    created      TEXT NOT NULL,
    created_ms   INTEGER NOT NULL,
    day          TEXT NOT NULL,
    modified_ms  INTEGER NOT NULL,
    kind         TEXT NOT NULL,
    title        TEXT NOT NULL,
    has_title    INTEGER NOT NULL DEFAULT 0,
    body         TEXT NOT NULL,
    source_title TEXT,
    source_url   TEXT,
    hash         TEXT NOT NULL
);
CREATE INDEX notes_created ON notes (created_ms DESC);
CREATE INDEX notes_day ON notes (day);
CREATE TABLE note_labels (
    note_id   TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    label     TEXT NOT NULL,
    label_key TEXT NOT NULL,
    pos       INTEGER NOT NULL,
    PRIMARY KEY (note_id, label_key)
);
CREATE INDEX note_labels_key ON note_labels (label_key);
CREATE VIRTUAL TABLE notes_fts USING fts5(
    note_id UNINDEXED, title, body, source, labels,
    tokenize = 'porter unicode61 remove_diacritics 2'
);
";

const SUMMARY_COLS: &str = "n.id, n.path, n.title, n.body, n.created, n.day, n.kind, n.source_title, n.source_url,
    (SELECT json_group_array(label) FROM (SELECT label FROM note_labels WHERE note_id = n.id ORDER BY pos)) AS labels,
    n.has_title";

fn delete_rowid(tx: &rusqlite::Transaction, rowid: i64) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM notes_fts WHERE rowid = ?1", [rowid])?;
    tx.execute(
        "DELETE FROM note_labels WHERE note_id = (SELECT id FROM notes WHERE rowid = ?1)",
        [rowid],
    )?;
    tx.execute("DELETE FROM notes WHERE rowid = ?1", [rowid])?;
    Ok(())
}

fn summary_from_row(r: &Row) -> rusqlite::Result<NoteSummary> {
    let body: String = r.get(3)?;
    let kind: String = r.get(6)?;
    let src_title: Option<String> = r.get(7)?;
    let src_url: Option<String> = r.get(8)?;
    let labels: String = r.get(9)?;
    Ok(NoteSummary {
        id: r.get(0)?,
        path: r.get(1)?,
        title: r.get(2)?,
        has_title: r.get(10)?,
        excerpt: excerpt(&body),
        created: r.get(4)?,
        day: r.get(5)?,
        kind: NoteKind::parse(&kind).unwrap_or(NoteKind::Personal),
        labels: serde_json::from_str(&labels).unwrap_or_default(),
        source: if src_title.is_some() || src_url.is_some() {
            Some(Source {
                title: src_title,
                url: src_url,
            })
        } else {
            None
        },
    })
}

fn excerpt(body: &str) -> String {
    let trimmed = body.trim();
    let mut out: String = trimmed.chars().take(EXCERPT_CHARS).collect();
    if trimmed.chars().count() > EXCERPT_CHARS {
        out.push('…');
    }
    out
}

fn push_filters(q: &ListQuery, where_: &mut Vec<String>, args: &mut Vec<rusqlite::types::Value>) {
    if let Some(label) = &q.label {
        where_.push("EXISTS (SELECT 1 FROM note_labels l WHERE l.note_id = n.id AND l.label_key = ?)".into());
        args.push(label.to_lowercase().into());
    }
    if let Some(kind) = q.kind {
        where_.push("n.kind = ?".into());
        args.push(kind.as_str().to_string().into());
    }
    if let Some(d) = &q.from_day {
        where_.push("n.day >= ?".into());
        args.push(d.clone().into());
    }
    if let Some(d) = &q.to_day {
        where_.push("n.day <= ?".into());
        args.push(d.clone().into());
    }
    if let Some(ms) = q.before_ms {
        where_.push("n.created_ms < ?".into());
        args.push(ms.into());
    }
}

fn group_by_day(notes: Vec<NoteSummary>) -> Vec<DaySummary> {
    let mut days: Vec<DaySummary> = Vec::new();
    for n in notes {
        match days.last_mut() {
            Some(d) if d.day == n.day => d.notes.push(n),
            _ => days.push(DaySummary {
                day: n.day.clone(),
                notes: vec![n],
            }),
        }
    }
    days
}

/// A search query split into a safe FTS5 expression, free text and filters.
#[derive(Debug, Default, PartialEq)]
pub struct ParsedQuery {
    pub fts: Option<String>,
    /// The words and phrases without filters, for semantic matching.
    pub text: String,
    pub label: Option<String>,
    pub kind: Option<NoteKind>,
}

/// Turn free text into a safe FTS5 expression plus structured filters.
pub fn parse_query(input: &str) -> ParsedQuery {
    let mut out = ParsedQuery::default();
    let mut terms: Vec<String> = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut chars = input.chars().peekable();
    let mut trailing_word = false;
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '"' {
            chars.next();
            let phrase: String = chars.by_ref().take_while(|&c| c != '"').collect();
            if !phrase.trim().is_empty() {
                terms.push(format!("\"{}\"", phrase.trim().replace('"', "")));
                words.push(phrase.trim().to_string());
            }
            trailing_word = false;
            continue;
        }
        let word: String = std::iter::from_fn(|| chars.next_if(|c| !c.is_whitespace())).collect();
        let lower = word.to_lowercase();
        if let Some(l) = lower
            .strip_prefix("label:")
            .map(|_| &word[6..])
            .or_else(|| word.strip_prefix('#'))
        {
            if !l.is_empty() {
                out.label = Some(l.to_string());
            }
            trailing_word = false;
        } else if let Some(k) = lower.strip_prefix("type:") {
            out.kind = NoteKind::parse(k);
            trailing_word = false;
        } else {
            let clean: String = word
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-')
                .collect();
            let clean = clean.trim_matches(['-', '\'']).to_string();
            if !clean.is_empty() {
                terms.push(format!("\"{clean}\""));
                words.push(word.clone());
                trailing_word = true;
            }
        }
    }
    if trailing_word && !input.ends_with(char::is_whitespace) {
        if let Some(last) = terms.last_mut() {
            last.push('*');
        }
    }
    out.text = words.join(" ");
    if !terms.is_empty() {
        out.fts = Some(terms.join(" "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note::parse_datetime;

    fn note(id: &str, created: &str, body: &str, labels: &[&str], source: Option<&str>) -> IndexedNote {
        IndexedNote {
            id: id.into(),
            path: format!("{id}.md"),
            created: parse_datetime(created).unwrap(),
            modified_ms: 0,
            kind: if source.is_some() {
                NoteKind::Source
            } else {
                NoteKind::Personal
            },
            title: crate::note::derive_title(body),
            has_title: false,
            body: body.into(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            source: source.map(|t| Source {
                title: Some(t.into()),
                url: None,
            }),
            hash: id.into(),
        }
    }

    fn fixture() -> Index {
        let mut ix = Index::open_in_memory().unwrap();
        for n in [
            note(
                "a",
                "2026-01-10T09:00:00+05:30",
                "Read about VC power-law returns.",
                &["VC"],
                Some("Venture Deals"),
            ),
            note(
                "b",
                "2026-03-02T22:00:00+05:30",
                "Ownership may matter more than I expected.",
                &["VC", "Product"],
                None,
            ),
            note(
                "c",
                "2026-03-02T23:30:00+05:30",
                "Tokyo and Kyoto itinerary ideas",
                &["Travel"],
                None,
            ),
            note(
                "d",
                "2026-09-12T08:00:00+05:30",
                "Is good company the right VC framework?",
                &["vc"],
                None,
            ),
        ] {
            ix.upsert(&n).unwrap();
        }
        ix
    }

    #[test]
    fn list_is_newest_first_with_labels() {
        let ix = fixture();
        let all = ix.list(&ListQuery::default()).unwrap();
        assert_eq!(
            all.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            ["d", "c", "b", "a"]
        );
        assert_eq!(all[2].labels, ["VC", "Product"]);
        let vc = ix
            .list(&ListQuery {
                label: Some("vc".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(vc.len(), 3);
        let page = ix
            .list(&ListQuery {
                before_ms: Some(all[1].created_ms()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page[0].id, "b");
    }

    impl NoteSummary {
        fn created_ms(&self) -> i64 {
            parse_datetime(&self.created).unwrap().timestamp_millis()
        }
    }

    #[test]
    fn search_ranks_and_filters() {
        let ix = fixture();
        let hits = ix.search("ownership", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("\u{2}Ownership\u{3}"));
        // Prefix match on last word while typing.
        assert_eq!(ix.search("owner", 10).unwrap().len(), 1);
        // Porter stemming.
        assert_eq!(ix.search("returning ", 10).unwrap()[0].note.id, "a");
        // Source title is searchable.
        assert_eq!(ix.search("venture deals", 10).unwrap()[0].note.id, "a");
        // Filters.
        assert_eq!(ix.search("label:Travel", 10).unwrap()[0].note.id, "c");
        assert_eq!(ix.search("#vc type:personal", 10).unwrap().len(), 2);
        assert_eq!(ix.search("vc type:source", 10).unwrap()[0].note.id, "a");
        // Hostile input doesn't break FTS syntax.
        assert!(ix.search("AND OR ( \" * :", 10).is_ok());
        assert!(ix.search("\"power-law returns\"", 10).unwrap().len() == 1);
    }

    #[test]
    fn labels_merge_case() {
        let ix = fixture();
        let labels = ix.labels().unwrap();
        assert_eq!(
            labels[0],
            LabelCount {
                label: "VC".into(),
                count: 3
            }
        );
    }

    #[test]
    fn journal_and_month_log() {
        let ix = fixture();
        let days = ix
            .days(&ListQuery {
                kind: Some(NoteKind::Personal),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            days.iter().map(|d| d.day.as_str()).collect::<Vec<_>>(),
            ["2026-09-12", "2026-03-02"]
        );
        assert_eq!(days[1].notes.len(), 2);
        let log = ix.month_log("2026-03").unwrap();
        assert_eq!(log.personal_count, 2);
        assert_eq!(log.active_days, 1);
        assert_eq!(log.top_labels.len(), 3);
        assert_eq!(ix.months().unwrap(), ["2026-09", "2026-03", "2026-01"]);
    }

    #[test]
    fn upsert_replaces_and_remove() {
        let mut ix = fixture();
        let mut b = note(
            "b",
            "2026-03-02T22:00:00+05:30",
            "Changed text about fund construction",
            &[],
            None,
        );
        b.path = "moved/b.md".into();
        ix.upsert(&b).unwrap();
        assert_eq!(ix.count().unwrap(), 4);
        assert!(ix.search("ownership", 10).unwrap().is_empty());
        assert_eq!(ix.search("fund", 10).unwrap()[0].note.path, "moved/b.md");
        assert!(ix.remove_path("moved/b.md").unwrap());
        assert_eq!(ix.count().unwrap(), 3);
        assert!(ix.search("fund", 10).unwrap().is_empty());
    }

    #[test]
    fn query_parser() {
        assert_eq!(parse_query("vc own").fts.as_deref(), Some("\"vc\" \"own\"*"));
        assert_eq!(parse_query("vc own ").fts.as_deref(), Some("\"vc\" \"own\""));
        let p = parse_query("label:Deep-Work \"fund outcome\"");
        assert_eq!(p.label.as_deref(), Some("Deep-Work"));
        assert_eq!(p.fts.as_deref(), Some("\"fund outcome\""));
        assert_eq!(parse_query("   ").fts, None);
    }
}
