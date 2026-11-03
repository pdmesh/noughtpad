//! Learning what *this* user considers related, from their own feedback.
//!
//! Signals (no approval queue, nothing to maintain):
//! * opening a suggested note from the Related panel → useful (positive);
//! * "Not related" → not useful (negative).
//!
//! The model is a logistic regression over a handful of pair features. Its
//! prior is today's hand-tuned rule, and training is regularised towards that
//! prior, so a few clicks nudge it rather than swing it. It only switches on
//! once there is feedback of both kinds.

use serde::{Deserialize, Serialize};

/// Features describing a (note, suggestion) pair. Kept small and readable so
/// the learned weights stay explainable.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PairFeatures {
    /// Similarity of the two notes as a whole (embedding).
    pub note_sim: f32,
    /// Similarity of the best-matching passages.
    pub passage_sim: f32,
    /// Shared labels, 0..1 (three or more = 1).
    pub shared_labels: f32,
    /// Both notes cite the same source.
    pub same_source: f32,
    /// Time apart, log-scaled to 0..1 (a year or more = 1).
    pub time_apart: f32,
    /// Both are the user's own (personal) notes.
    pub both_personal: f32,
    /// Both are labelled, with no label in common.
    pub label_mismatch: f32,
}

pub const N: usize = 7;

impl PairFeatures {
    pub fn to_vec(self) -> [f32; N] {
        [
            self.note_sim,
            self.passage_sim,
            self.shared_labels,
            self.same_source,
            self.time_apart,
            self.both_personal,
            self.label_mismatch,
        ]
    }

    pub fn time_feature(days: i64) -> f32 {
        ((1.0 + days.unsigned_abs() as f32).ln() / 366f32.ln()).min(1.0)
    }
}

/// Weights + bias of the logistic model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    pub w: [f32; N],
    pub b: f32,
}

/// Minimum feedback of each kind before personalisation switches on.
pub const MIN_EACH: usize = 3;
/// Pull towards the prior (larger = learns more slowly).
const LAMBDA: f32 = 0.05;

impl Model {
    /// The hand-tuned rule expressed as a logistic model:
    /// show if `0.5·note_sim + 0.5·passage_sim + 0.06·shared + 0.03·same_source ≥ threshold`.
    pub fn prior(threshold: f32) -> Self {
        let k = 20.0;
        Model {
            w: [0.5 * k, 0.5 * k, 0.06 * k, 0.03 * k, 0.0, 0.0, 0.0],
            b: -threshold * k,
        }
    }

    pub fn probability(&self, f: &PairFeatures) -> f32 {
        let x = f.to_vec();
        let z: f32 = self.w.iter().zip(x).map(|(w, x)| w * x).sum::<f32>() + self.b;
        1.0 / (1.0 + (-z).exp())
    }

    /// Fit to `(features, useful)` examples, regularised towards `prior`.
    /// Returns `None` until there is enough feedback of both kinds.
    pub fn fit(prior: Model, examples: &[(PairFeatures, bool)]) -> Option<Model> {
        let pos = examples.iter().filter(|e| e.1).count();
        let neg = examples.len() - pos;
        if pos < MIN_EACH || neg < MIN_EACH {
            return None;
        }
        // Balance the classes so many dismissals don't drown a few opens.
        let (wp, wn) = (0.5 / pos as f32, 0.5 / neg as f32);
        let mut m = prior;
        let lr = 2.0;
        for _ in 0..400 {
            let mut gw = [0f32; N];
            let mut gb = 0f32;
            for (f, y) in examples {
                let err = m.probability(f) - if *y { 1.0 } else { 0.0 };
                let weight = if *y { wp } else { wn };
                for (g, x) in gw.iter_mut().zip(f.to_vec()) {
                    *g += weight * err * x;
                }
                gb += weight * err;
            }
            for ((w, g), p) in m.w.iter_mut().zip(gw).zip(prior.w) {
                *w -= lr * (g + LAMBDA * (*w - p));
            }
            m.b -= lr * (gb + LAMBDA * (m.b - prior.b));
        }
        Some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(sim: f32, shared: f32, mismatch: f32) -> PairFeatures {
        PairFeatures {
            note_sim: sim,
            passage_sim: sim,
            shared_labels: shared,
            label_mismatch: mismatch,
            both_personal: 1.0,
            ..Default::default()
        }
    }

    #[test]
    fn prior_matches_the_hand_tuned_rule() {
        let p = Model::prior(0.60);
        assert!(p.probability(&f(0.62, 0.0, 0.0)) > 0.5);
        assert!(p.probability(&f(0.58, 0.0, 0.0)) < 0.5);
        // A shared label nudges a borderline pair over.
        assert!(p.probability(&f(0.59, 1.0 / 3.0, 0.0)) > 0.5);
    }

    #[test]
    fn needs_feedback_of_both_kinds() {
        let ex = vec![(f(0.7, 0.0, 0.0), true); 10];
        assert!(Model::fit(Model::prior(0.6), &ex).is_none());
    }

    #[test]
    fn learns_that_this_user_dislikes_cross_label_suggestions() {
        let prior = Model::prior(0.60);
        // Opens: same-label pairs. Dismissals: similar-looking pairs across labels.
        let mut ex = Vec::new();
        for s in [0.62, 0.66, 0.7, 0.74] {
            ex.push((f(s, 1.0 / 3.0, 0.0), true));
            ex.push((f(s, 0.0, 1.0), false));
        }
        let m = Model::fit(prior, &ex).unwrap();
        let cross = f(0.68, 0.0, 1.0);
        let same = f(0.64, 1.0 / 3.0, 0.0);
        assert!(prior.probability(&cross) > 0.5, "shown before learning");
        assert!(
            m.probability(&cross) < 0.5,
            "hidden after learning: {}",
            m.probability(&cross)
        );
        assert!(m.probability(&same) > 0.5);
        // Unlabelled pairs (no evidence either way) behave much as before.
        let plain = f(0.66, 0.0, 0.0);
        assert!(m.probability(&plain) > 0.5);
    }
}
