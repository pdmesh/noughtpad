//! In-memory flat vector index.
//!
//! A personal notebook has thousands to tens of thousands of chunks; an exact
//! scan over normalised vectors takes a few milliseconds and needs no
//! approximate-index tuning or native extension.

use std::collections::HashMap;

use super::embedder::{dot, normalise};

#[derive(Debug, Default)]
pub(crate) struct NoteVectors {
    /// `(chunk ord, vector)`
    pub chunks: Vec<(u32, Vec<f32>)>,
    /// Normalised mean of the chunk vectors: the note as a whole.
    pub mean: Vec<f32>,
}

#[derive(Debug, Default)]
pub(crate) struct VectorIndex {
    pub notes: HashMap<String, NoteVectors>,
}

impl VectorIndex {
    pub fn from_rows(rows: Vec<(String, u32, Vec<f32>)>) -> Self {
        let mut ix = VectorIndex::default();
        let mut grouped: HashMap<String, Vec<(u32, Vec<f32>)>> = HashMap::new();
        for (id, ord, v) in rows {
            grouped.entry(id).or_default().push((ord, v));
        }
        for (id, chunks) in grouped {
            ix.set(id, chunks);
        }
        ix
    }

    pub fn set(&mut self, id: String, chunks: Vec<(u32, Vec<f32>)>) {
        if chunks.is_empty() {
            self.notes.remove(&id);
            return;
        }
        let dims = chunks[0].1.len();
        let mut mean = vec![0f32; dims];
        for (_, v) in &chunks {
            mean.iter_mut().zip(v).for_each(|(m, x)| *m += x);
        }
        normalise(&mut mean);
        self.notes.insert(id, NoteVectors { chunks, mean });
    }

    pub fn remove(&mut self, id: &str) {
        self.notes.remove(id);
    }

    /// Best-matching chunk per note for a query vector, best first.
    pub fn search(&self, q: &[f32], limit: usize) -> Vec<(String, u32, f32)> {
        let mut hits: Vec<(String, u32, f32)> = self
            .notes
            .iter()
            .filter_map(|(id, nv)| {
                nv.chunks
                    .iter()
                    .map(|(ord, v)| (*ord, dot(q, v)))
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(ord, s)| (id.clone(), ord, s))
            })
            .collect();
        hits.sort_by(|a, b| b.2.total_cmp(&a.2));
        hits.truncate(limit);
        hits
    }
}
