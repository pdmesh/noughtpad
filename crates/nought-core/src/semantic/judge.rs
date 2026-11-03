//! Tier 2: small non-generative "judge" models.
//!
//! Embeddings find candidates broadly and cheaply; a judge reads two texts
//! *together* and returns scores, never prose, so it can't invent anything.
//! Judges only look at the handful of candidates the embeddings found.
//!
//! * `rerank`: how relevant each document is to a query (0–1).
//! * `nli`: whether a hypothesis follows from, contradicts, or is neutral to
//!   a premise (natural-language inference). Optional: measured on the sample
//!   notebook, the small NLI model we tried (DeBERTa-v3-xsmall) produced too
//!   many false contradictions on free-form notes, so it is not part of the
//!   default bundle. The interface stays so better models can be evaluated
//!   with `examples/judges.rs`.
//!
//! Without a judge, the heuristics from M2/M3 apply, so the app stays useful
//! with any subset of models.

use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nli {
    pub entail: f32,
    pub neutral: f32,
    pub contradict: f32,
}

pub trait Judge: Send {
    /// Relevance of each document to `query`, in 0..1.
    fn rerank(&mut self, query: &str, docs: &[String]) -> Result<Vec<f32>>;
    /// Whether an inference (NLI) model is available.
    fn has_nli(&self) -> bool;
    /// Inference probabilities for `(premise, hypothesis)` pairs.
    fn nli(&mut self, pairs: &[(String, String)]) -> Result<Vec<Nli>>;
}

/// Thresholds for interpreting a judge's scores. Measured per model pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JudgeSpec {
    /// A related-note candidate must reach this relevance to be shown.
    pub min_relevance: f32,
    /// At or above this relevance a connection is described as strong.
    pub strong_relevance: f32,
    /// Probability above which two notes are said to pull in different directions.
    pub contradiction: f32,
    /// Probability above which a later note is said to agree with / build on an earlier one.
    pub entailment: f32,
}

pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

pub fn softmax3(l: [f32; 3]) -> [f32; 3] {
    let m = l.iter().copied().fold(f32::MIN, f32::max);
    let e = l.map(|x| (x - m).exp());
    let sum: f32 = e.iter().sum();
    e.map(|x| x / sum)
}

/// A deterministic stand-in for tests: word overlap for relevance, and a
/// contradiction whenever the hypothesis negates or contrasts shared words.
pub struct LexicalJudge;

impl LexicalJudge {
    pub const SPEC: JudgeSpec = JudgeSpec {
        min_relevance: 0.2,
        strong_relevance: 0.6,
        contradiction: 0.5,
        entailment: 0.5,
    };
}

fn words(t: &str) -> std::collections::HashSet<String> {
    t.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 3)
        .map(|w| w.to_lowercase())
        .collect()
}

impl Judge for LexicalJudge {
    fn has_nli(&self) -> bool {
        true
    }

    fn rerank(&mut self, query: &str, docs: &[String]) -> Result<Vec<f32>> {
        let q = words(query);
        Ok(docs
            .iter()
            .map(|d| {
                let w = words(d);
                let inter = q.intersection(&w).count() as f32;
                inter / (q.len().min(w.len()).max(1) as f32)
            })
            .collect())
    }

    fn nli(&mut self, pairs: &[(String, String)]) -> Result<Vec<Nli>> {
        Ok(pairs
            .iter()
            .map(|(p, h)| {
                let overlap = !words(p).is_disjoint(&words(h));
                let lower = format!(" {} ", h.to_lowercase());
                let negates = [" not ", " isn't ", " doesn't ", " less ", " wrong ", " but "]
                    .iter()
                    .any(|k| lower.contains(k));
                match (overlap, negates) {
                    (true, true) => Nli {
                        entail: 0.1,
                        neutral: 0.2,
                        contradict: 0.7,
                    },
                    (true, false) => Nli {
                        entail: 0.7,
                        neutral: 0.2,
                        contradict: 0.1,
                    },
                    _ => Nli {
                        entail: 0.1,
                        neutral: 0.8,
                        contradict: 0.1,
                    },
                }
            })
            .collect())
    }
}
