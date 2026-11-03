//! Embedding models behind one small trait, so the model and runtime can be
//! swapped without touching the data model.

use crate::Result;

/// Static facts about an embedding model, including the similarity
/// thresholds that make sense for it (they differ a lot between models).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelSpec {
    /// Stable identifier stored with every vector.
    pub id: &'static str,
    pub dims: usize,
    pub query_prefix: &'static str,
    pub passage_prefix: &'static str,
    /// Below this cosine similarity two notes are never shown as related.
    pub min_related: f32,
    /// At or above this, a connection is described as strong.
    pub strong_related: f32,
    /// Minimum similarity for a meaning-only search hit.
    pub min_search: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    Query,
    Passage,
}

pub trait Embedder: Send {
    fn spec(&self) -> &ModelSpec;
    /// Embed texts. Returned vectors are L2-normalised.
    fn embed(&mut self, texts: &[String], kind: TextKind) -> Result<Vec<Vec<f32>>>;
}

pub fn normalise(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        v.iter_mut().for_each(|x| *x /= n);
    }
}

pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A deterministic, dependency-free embedder based on hashed word and
/// word-pair features. It only captures *lexical* overlap, so it is used in
/// tests and as a stand-in where no model is available, never as the
/// product's semantic model.
pub struct HashEmbedder {
    spec: ModelSpec,
}

impl Default for HashEmbedder {
    fn default() -> Self {
        HashEmbedder {
            spec: ModelSpec {
                id: "hash-256",
                dims: 256,
                query_prefix: "",
                passage_prefix: "",
                min_related: 0.2,
                strong_related: 0.5,
                min_search: 0.15,
            },
        }
    }
}

impl Embedder for HashEmbedder {
    fn spec(&self) -> &ModelSpec {
        &self.spec
    }

    fn embed(&mut self, texts: &[String], _kind: TextKind) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| {
                let words: Vec<String> = t
                    .split(|c: char| !c.is_alphanumeric())
                    .filter(|w| w.len() > 2)
                    .map(|w| {
                        let w = w.to_lowercase();
                        w.strip_suffix('s').map(str::to_string).unwrap_or(w)
                    })
                    .collect();
                let mut v = vec![0f32; self.spec.dims];
                let mut add = |key: &str, weight: f32| {
                    let h = blake3::hash(key.as_bytes());
                    let b = h.as_bytes();
                    let idx = u16::from_le_bytes([b[0], b[1]]) as usize % self.spec.dims;
                    v[idx] += if b[2] & 1 == 0 { weight } else { -weight };
                };
                for w in &words {
                    add(w, 1.0);
                }
                for pair in words.windows(2) {
                    add(&format!("{} {}", pair[0], pair[1]), 0.5);
                }
                normalise(&mut v);
                v
            })
            .collect())
    }
}
