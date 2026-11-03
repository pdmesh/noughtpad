//! Picking the notes a language model may read for one question (M4).
//!
//! The model only ever sees the notes chosen here, and the app shows the same
//! list to the user before anything leaves the machine. Selection is
//! deterministic and explainable: a scope (a month, a theme, one note…) plus,
//! for free questions, hybrid search.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::Semantic;
use crate::index::{ListQuery, NoteSummary};
use crate::Result;

/// What the question is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Scope {
    /// Search the whole notebook for notes relevant to the question.
    Notebook,
    /// One note and the notes related to it ("Explore this note").
    Note { id: String },
    /// A theme from Ideas over time, oldest note first.
    Theme { id: String },
    /// Every note in a calendar month (`YYYY-MM`).
    Month { month: String },
    /// A collection.
    Label { label: String },
    /// Questions the user asked themselves and hasn't returned to.
    OpenQuestions,
}

/// One note handed to the model, with its full body.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextNote {
    pub note: NoteSummary,
    pub body: String,
}

/// Keep at most `limit` items, spread evenly and keeping the first and last
/// (so a timeline keeps its start and end).
fn spread<T: Clone>(items: &[T], limit: usize) -> Vec<T> {
    if items.len() <= limit || limit == 0 {
        return items.iter().take(limit).cloned().collect();
    }
    if limit == 1 {
        return vec![items[items.len() - 1].clone()];
    }
    let step = (items.len() - 1) as f64 / (limit - 1) as f64;
    (0..limit)
        .map(|i| items[(i as f64 * step).round() as usize].clone())
        .collect()
}

impl Semantic {
    /// The notes (oldest first) a model may read to answer `question` within `scope`.
    pub fn context(&self, scope: &Scope, question: &str, limit: usize) -> Result<Vec<ContextNote>> {
        let summaries: Vec<NoteSummary> = match scope {
            Scope::Notebook => self
                .search(question, limit as u32)?
                .into_iter()
                .map(|h| h.note)
                .collect(),
            Scope::Note { id } => {
                let mut out: Vec<NoteSummary> = self.vault.summary(id)?.into_iter().collect();
                let related = self.related(id, limit.saturating_sub(1))?;
                out.extend(related.items.into_iter().map(|r| r.note));
                out
            }
            Scope::Theme { id } => match self.theme_timeline(id)? {
                Some(t) => spread(&t.entries.into_iter().map(|e| e.note).collect::<Vec<_>>(), limit),
                None => vec![],
            },
            Scope::Month { month } => {
                let log = self.vault.month_log(month)?;
                let mut notes: Vec<NoteSummary> = log.days.into_iter().flat_map(|d| d.notes).collect();
                notes.sort_by(|a, b| a.created.cmp(&b.created));
                spread(&notes, limit)
            }
            Scope::Label { label } => {
                let q = ListQuery {
                    label: Some(label.clone()),
                    limit: Some(500),
                    ..Default::default()
                };
                let mut notes = self.vault.list(&q)?;
                notes.sort_by(|a, b| a.created.cmp(&b.created));
                // A long collection: prefer the notes that match the question, if any do.
                if notes.len() > limit && !question.trim().is_empty() {
                    let hits: HashSet<String> = self
                        .search(&format!("#{label} {question}"), limit as u32)?
                        .into_iter()
                        .map(|h| h.note.id)
                        .collect();
                    if !hits.is_empty() {
                        notes.retain(|n| hits.contains(&n.id));
                    }
                }
                spread(&notes, limit)
            }
            Scope::OpenQuestions => {
                let overview = self.ideas_overview()?;
                let notes: Vec<NoteSummary> = overview.open_questions.into_iter().map(|q| q.note).collect();
                notes.into_iter().take(limit).collect()
            }
        };

        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for s in summaries {
            if !seen.insert(s.id.clone()) {
                continue;
            }
            match self.vault.get(&s.id) {
                Ok(doc) => out.push(ContextNote {
                    note: s,
                    body: doc.content,
                }),
                Err(crate::Error::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        out.sort_by(|a, b| a.note.created.cmp(&b.note.created));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::spread;

    #[test]
    fn spread_keeps_ends_and_order() {
        let v: Vec<u32> = (0..10).collect();
        assert_eq!(spread(&v, 4), vec![0, 3, 6, 9]);
        assert_eq!(spread(&v, 20), v);
        assert_eq!(spread(&v, 1), vec![9]);
        assert!(spread(&v, 0).is_empty());
    }
}
