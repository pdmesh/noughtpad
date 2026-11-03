//! Temporal discovery (M3): themes and how they move through time.
//!
//! Everything here is deterministic over the note vectors from M2. No language
//! model writes anything:
//!
//! * **Themes** are groups of notes built by average-linkage merging over the
//!   mutual nearest-neighbour graph of *mean-centred* vectors: groups join only
//!   while their notes stay related on average.
//! * **Names** are the most distinctive words of a theme (class-based TF-IDF).
//! * **Identity** is kept across recomputations by member overlap, so a theme
//!   keeps its history while notes are added.
//! * **Signals**: recurrence across months, new and dormant themes, earlier
//!   source notes that later personal notes build on, notes that return to or
//!   may revise an earlier thought, and questions never returned to.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Months, NaiveDate};
use serde::Serialize;

use super::embedder::dot;
use crate::index::NoteSummary;
use crate::note::NoteKind;

/// Neighbours considered per note when forming themes.
const K: usize = 6;
/// A theme needs at least this many notes…
const MIN_THEME: usize = 3;
/// …written on at least this many different days.
const MIN_DAYS: usize = 2;
/// A later note "returns to" an earlier one only if written this many days later.
const REVISIT_GAP_DAYS: i64 = 7;
const FORMING_DAYS: i64 = 45;
/// Average similarity (of mean-centred vectors) a theme's notes must keep.
/// Measured on the sample notebook with BGE-small: members 0.13–0.48,
/// look-alike intruders −0.04–0.0.
const CENTRED_MIN: f32 = 0.10;
/// The user's labels are the strongest signal of what they consider separate:
/// groups whose main labels differ need this much more similarity to merge,
/// and groups sharing a main label need slightly less.
const LABEL_PENALTY: f32 = 0.10;
const LABEL_BONUS: f32 = 0.03;
/// Centring needs a reasonable sample of the notebook.
const MIN_NOTES_FOR_THEMES: usize = 8;
const DORMANT_DAYS: i64 = 90;

/// The note data temporal analysis needs (from the index).
#[derive(Debug, Clone)]
pub(crate) struct NoteRow {
    pub id: String,
    pub day: NaiveDate,
    pub created_ms: i64,
    pub kind: NoteKind,
    pub text: String,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ThemeData {
    pub id: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub labels: Vec<String>,
    /// Member note indices, oldest first.
    pub members: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Link {
    pub to: usize,
    pub score: f32,
}

/// The computed model: themes plus per-note temporal links.
#[derive(Debug, Default)]
pub(crate) struct Model {
    pub version: u64,
    pub today: Option<NaiveDate>,
    pub notes: Vec<NoteRow>,
    pub themes: Vec<ThemeData>,
    pub note_theme: HashMap<String, usize>,
    /// Personal note -> the earlier source note it most likely builds on.
    pub influenced_by: HashMap<usize, Link>,
    /// Personal note -> the earlier personal note it returns to.
    pub revisits: HashMap<usize, Link>,
    pub may_revise: HashSet<usize>,
    pub questions: HashSet<usize>,
    /// Questions with no later closely-related note, newest first.
    pub open_questions: Vec<usize>,
}

pub(crate) struct Thresholds {
    pub related: f32,
    pub strong: f32,
}

/// Build the model. `means[i]` is the vector of `notes[i]` (normalised);
/// `previous` maps earlier theme ids to their member note ids.
pub(crate) fn compute(
    notes: Vec<NoteRow>,
    means: &[Vec<f32>],
    previous: &HashMap<String, HashSet<String>>,
    t: &Thresholds,
    today: NaiveDate,
    version: u64,
) -> Model {
    let n = notes.len();
    // Themes are found on mean-centred vectors: subtracting what all notes have
    // in common (the "it's English prose about work" direction) leaves what is
    // specific to each note, so generic notes no longer look alike.
    let groups = if n >= MIN_NOTES_FOR_THEMES {
        let centred = centre(means);
        let neighbours = knn(&centred, CENTRED_MIN - LABEL_BONUS);
        let labels: Vec<HashSet<String>> = notes
            .iter()
            .map(|n| n.labels.iter().map(|l| l.to_lowercase()).collect())
            .collect();
        cluster(&neighbours, &centred, &labels, CENTRED_MIN)
    } else {
        Vec::new()
    };

    let mut groups: Vec<Vec<usize>> = groups
        .into_iter()
        .filter(|g| g.len() >= MIN_THEME && g.iter().map(|&i| notes[i].day).collect::<HashSet<_>>().len() >= MIN_DAYS)
        .collect();
    for g in &mut groups {
        g.sort_by_key(|&i| notes[i].created_ms);
    }

    let keywords = keywords(&notes, &groups);
    let ids = assign_ids(&notes, &groups, previous);
    let mut themes: Vec<ThemeData> = groups
        .into_iter()
        .zip(keywords)
        .zip(ids)
        .map(|((members, keywords), id)| {
            let labels = dominant_labels(&notes, &members);
            // Prefer distinctive words; fall back to the user's label, then the latest note.
            let name = if !keywords.is_empty() {
                keywords.iter().take(3).cloned().collect::<Vec<_>>().join(" · ")
            } else if !labels.is_empty() {
                labels.join(" · ")
            } else {
                let last = &notes[*members.last().unwrap()].text;
                last.lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .chars()
                    .take(40)
                    .collect()
            };
            ThemeData {
                id,
                name,
                labels,
                keywords,
                members,
            }
        })
        .collect();
    themes.sort_by(|a, b| b.members.len().cmp(&a.members.len()).then(a.id.cmp(&b.id)));

    let mut model = Model {
        version,
        today: Some(today),
        ..Default::default()
    };
    for (ti, th) in themes.iter().enumerate() {
        for &m in &th.members {
            model.note_theme.insert(notes[m].id.clone(), ti);
        }
    }

    // Links inside each theme: influences and revisits.
    for th in &themes {
        for (pos, &p) in th.members.iter().enumerate() {
            if notes[p].kind != NoteKind::Personal {
                continue;
            }
            let earlier = &th.members[..pos];
            let best = |kind: NoteKind, min_gap: i64| {
                earlier
                    .iter()
                    .filter(|&&q| notes[q].kind == kind && (notes[p].day - notes[q].day).num_days() >= min_gap)
                    .map(|&q| Link {
                        to: q,
                        score: dot(&means[p], &means[q]),
                    })
                    .filter(|l| l.score >= t.related)
                    .max_by(|a, b| a.score.total_cmp(&b.score))
            };
            if let Some(l) = best(NoteKind::Source, 0) {
                model.influenced_by.insert(p, l);
            }
            if let Some(l) = best(NoteKind::Personal, REVISIT_GAP_DAYS) {
                model.revisits.insert(p, l);
                if signals_revision(&notes[p].text) {
                    model.may_revise.insert(p);
                }
            }
        }
    }

    // Questions, and whether anything later picked them up.
    for i in 0..n {
        if notes[i].kind == NoteKind::Personal && is_question(&notes[i].text) {
            model.questions.insert(i);
            let answered = (0..n).any(|j| {
                notes[j].created_ms > notes[i].created_ms
                    && (notes[j].day - notes[i].day).num_days() >= 1
                    && dot(&means[i], &means[j]) >= t.strong
            });
            if !answered {
                model.open_questions.push(i);
            }
        }
    }
    model
        .open_questions
        .sort_by_key(|&i| std::cmp::Reverse(notes[i].created_ms));

    model.notes = notes;
    model.themes = themes;
    model
}

fn centre(means: &[Vec<f32>]) -> Vec<Vec<f32>> {
    let dims = means.first().map_or(0, Vec::len);
    let mut mu = vec![0f32; dims];
    for m in means {
        mu.iter_mut().zip(m).for_each(|(x, y)| *x += y / means.len() as f32);
    }
    means
        .iter()
        .map(|m| {
            let mut v: Vec<f32> = m.iter().zip(&mu).map(|(a, b)| a - b).collect();
            super::embedder::normalise(&mut v);
            v
        })
        .collect()
}

/// For each note, its top-K neighbours with similarity ≥ `min` (symmetric pass).
fn knn(means: &[Vec<f32>], min: f32) -> Vec<Vec<(usize, f32)>> {
    let n = means.len();
    let mut top: Vec<Vec<(usize, f32)>> = vec![Vec::with_capacity(K + 1); n];
    let push = |list: &mut Vec<(usize, f32)>, j: usize, s: f32| {
        if list.len() < K || s > list[list.len() - 1].1 {
            let at = list.partition_point(|&(_, x)| x >= s);
            list.insert(at, (j, s));
            list.truncate(K);
        }
    };
    for i in 0..n {
        for j in (i + 1)..n {
            let s = dot(&means[i], &means[j]);
            if s >= min {
                push(&mut top[i], j, s);
                push(&mut top[j], i, s);
            }
        }
    }
    top
}

/// Average-linkage agglomeration over the mutual-kNN graph: candidate pairs
/// are visited strongest first, and two groups merge only if the *average*
/// similarity between their notes stays above `min`. Unlike connected
/// components, one coincidental link can't chain two themes together.
fn cluster(
    neighbours: &[Vec<(usize, f32)>],
    means: &[Vec<f32>],
    labels: &[HashSet<String>],
    min: f32,
) -> Vec<Vec<usize>> {
    let n = means.len();
    let mut edges: Vec<(f32, usize, usize)> = Vec::new();
    for i in 0..n {
        for &(j, s) in &neighbours[i] {
            let mutual = neighbours[j].iter().any(|&(k, _)| k == i);
            if i < j && mutual && s >= min - LABEL_BONUS {
                edges.push((s, i, j));
            }
        }
    }
    edges.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let mut parent: Vec<usize> = (0..n).collect();
    let mut size = vec![1usize; n];
    let mut sum: Vec<Vec<f32>> = means.to_vec();
    // Label counts per group, for the label-aware merge threshold.
    let mut label_counts: Vec<HashMap<String, usize>> = labels
        .iter()
        .map(|ls| ls.iter().map(|l| (l.clone(), 1)).collect())
        .collect();
    let main_labels = |counts: &HashMap<String, usize>, size: usize| -> HashSet<String> {
        counts
            .iter()
            .filter(|(_, &c)| c * 2 >= size)
            .map(|(l, _)| l.clone())
            .collect()
    };
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for (_, i, j) in edges {
        let (a, b) = (find(&mut parent, i), find(&mut parent, j));
        if a == b {
            continue;
        }
        // Mean pairwise similarity between the groups (vectors are normalised).
        let avg = dot(&sum[a], &sum[b]) / (size[a] * size[b]) as f32;
        let (la, lb) = (
            main_labels(&label_counts[a], size[a]),
            main_labels(&label_counts[b], size[b]),
        );
        let threshold = if la.is_empty() || lb.is_empty() {
            min
        } else if la.is_disjoint(&lb) {
            min + LABEL_PENALTY
        } else {
            min - LABEL_BONUS
        };
        if avg >= threshold {
            let (keep, gone) = if size[a] >= size[b] { (a, b) } else { (b, a) };
            parent[gone] = keep;
            size[keep] += size[gone];
            let add = std::mem::take(&mut sum[gone]);
            sum[keep].iter_mut().zip(add).for_each(|(x, y)| *x += y);
            for (l, c) in std::mem::take(&mut label_counts[gone]) {
                *label_counts[keep].entry(l).or_default() += c;
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().collect();
    out.sort_by_key(|g| g.iter().min().copied());
    out
}

/// Reuse an earlier theme id when a new group overlaps it enough (Jaccard ≥ 0.3).
fn assign_ids(notes: &[NoteRow], groups: &[Vec<usize>], previous: &HashMap<String, HashSet<String>>) -> Vec<String> {
    let mut pairs: Vec<(f32, usize, &String)> = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        let members: HashSet<&str> = g.iter().map(|&i| notes[i].id.as_str()).collect();
        for (pid, prev) in previous {
            let inter = prev.iter().filter(|id| members.contains(id.as_str())).count();
            let union = members.len() + prev.len() - inter;
            let j = inter as f32 / union.max(1) as f32;
            if j >= 0.3 {
                pairs.push((j, gi, pid));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.2.cmp(b.2)));
    let mut ids: Vec<Option<String>> = vec![None; groups.len()];
    let mut used = HashSet::new();
    for (_, gi, pid) in pairs {
        if ids[gi].is_none() && used.insert(pid.clone()) {
            ids[gi] = Some(pid.clone());
        }
    }
    ids.into_iter()
        .map(|id| id.unwrap_or_else(|| format!("t-{}", ulid::Ulid::new().to_string().to_lowercase())))
        .collect()
}

fn dominant_labels(notes: &[NoteRow], members: &[usize]) -> Vec<String> {
    let mut counts: HashMap<String, (String, usize)> = HashMap::new();
    for &m in members {
        for l in &notes[m].labels {
            let e = counts.entry(l.to_lowercase()).or_insert((l.clone(), 0));
            e.1 += 1;
        }
    }
    let mut v: Vec<(String, usize)> = counts.into_values().filter(|(_, c)| *c * 2 >= members.len()).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().map(|(l, _)| l).take(3).collect()
}

// ---- naming -----------------------------------------------------------------

/// English stopwords; other languages plug in their own list later.
const STOPWORDS: &str = "a about above after again against all also am an and any are as at be because been before being below between both but by can could did do does doing down during each even few for from further get got had has have having he her here hers him his how i if in into is it its itself just know like make made many may me might more most much must my no nor not now of off on once only or other our out over own really same she should so some still such than that the their them then there these they thing things think this those through to too under until up us very was way we well were what when where which while who why will with would yet you your yours one two three first next last new day days time week today tomorrow need needs want maybe seems seem lot bit read note notes idea ideas thought thoughts";

fn tokens(text: &str) -> Vec<String> {
    use std::sync::OnceLock;
    static STOP: OnceLock<HashSet<&'static str>> = OnceLock::new();
    let stop = STOP.get_or_init(|| STOPWORDS.split_whitespace().collect());
    let words: Vec<String> = text
        .split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '\''))
        .map(|w| w.trim_matches(|c| c == '-' || c == '\'').to_lowercase())
        .collect();
    let mut out = Vec::new();
    let mut prev: Option<String> = None;
    for w in words {
        let keep = w.chars().count() >= 3 && !stop.contains(w.as_str()) && !w.chars().all(|c| c.is_ascii_digit());
        if keep {
            let w = singular(&w);
            if let Some(p) = &prev {
                out.push(format!("{p} {w}"));
            }
            out.push(w.clone());
            prev = Some(w);
        } else {
            prev = None;
        }
    }
    out
}

fn singular(w: &str) -> String {
    if w.len() > 4 && w.ends_with("ies") {
        format!("{}y", &w[..w.len() - 3])
    } else if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") && !w.ends_with("is") {
        w[..w.len() - 1].to_string()
    } else {
        w.to_string()
    }
}

/// Class-based TF-IDF (as in BERTopic): terms frequent in a theme but rare
/// across the notebook. A term must appear in at least two of the theme's notes.
fn keywords(notes: &[NoteRow], groups: &[Vec<usize>]) -> Vec<Vec<String>> {
    let per_note: Vec<Vec<String>> = notes.iter().map(|n| tokens(&n.text)).collect();
    let mut global: HashMap<&str, usize> = HashMap::new();
    for t in per_note.iter().flatten() {
        *global.entry(t.as_str()).or_default() += 1;
    }
    let avg_terms = per_note.iter().map(Vec::len).sum::<usize>() as f32 / groups.len().max(1) as f32;
    groups
        .iter()
        .map(|g| {
            let mut tf: HashMap<&str, usize> = HashMap::new();
            let mut df: HashMap<&str, usize> = HashMap::new();
            for &m in g {
                let mut seen = HashSet::new();
                for t in &per_note[m] {
                    *tf.entry(t.as_str()).or_default() += 1;
                    if seen.insert(t.as_str()) {
                        *df.entry(t.as_str()).or_default() += 1;
                    }
                }
            }
            let mut scored: Vec<(f32, &str)> = tf
                .iter()
                .filter(|(t, _)| df[*t] >= 2)
                .map(|(t, &c)| {
                    let bigram_bonus = if t.contains(' ') { 1.5 } else { 1.0 };
                    let s = c as f32 * (1.0 + avg_terms / global[t] as f32).ln() * bigram_bonus;
                    (s, *t)
                })
                .collect();
            scored.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)));
            let mut chosen: Vec<String> = Vec::new();
            for (_, t) in scored {
                // Skip words already covered by a chosen phrase, and vice versa.
                let overlaps = chosen
                    .iter()
                    .any(|c| c.split(' ').any(|w| t.split(' ').any(|x| x == w)));
                if !overlaps {
                    chosen.push(t.to_string());
                }
                if chosen.len() == 5 {
                    break;
                }
            }
            chosen
        })
        .collect()
}

// ---- text signals -------------------------------------------------------------

pub(crate) fn is_question(text: &str) -> bool {
    let lower = text.to_lowercase();
    let asks = text.char_indices().any(|(i, c)| {
        c == '?'
            && text[..i]
                .chars()
                .last()
                .is_some_and(|p| p.is_alphanumeric() || p == '"' || p == ')')
    });
    asks || [
        "open question",
        "not sure",
        "need to understand",
        "need to figure out",
        "i wonder",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

/// Words that suggest a note pushes back on, or rethinks, something earlier.
fn signals_revision(text: &str) -> bool {
    let lower = format!(" {} ", text.to_lowercase());
    [
        " but ",
        " however",
        "tension",
        "disagree",
        "changed my mind",
        "rethink",
        "not sure",
        "actually",
        "on the other hand",
        "contradict",
        "wrong",
        "question whether",
        "not just",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

// ---- views --------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MonthCount {
    pub month: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSummary {
    pub id: String,
    pub name: String,
    pub keywords: Vec<String>,
    pub labels: Vec<String>,
    pub note_count: usize,
    pub personal_count: usize,
    pub source_count: usize,
    pub first_day: String,
    pub last_day: String,
    pub active_months: usize,
    /// The last 12 calendar months, oldest first.
    pub months: Vec<MonthCount>,
}

impl Model {
    pub fn summary(&self, ti: usize) -> ThemeSummary {
        let th = &self.themes[ti];
        let days: Vec<NaiveDate> = th.members.iter().map(|&m| self.notes[m].day).collect();
        let month_key = |d: NaiveDate| format!("{:04}-{:02}", d.year(), d.month());
        let mut by_month: HashMap<String, usize> = HashMap::new();
        for d in &days {
            *by_month.entry(month_key(*d)).or_default() += 1;
        }
        let today = self.today.unwrap_or_else(|| *days.last().unwrap());
        let this_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
        let months = (0..12)
            .rev()
            .map(|back| {
                let m = this_month - Months::new(back);
                let key = month_key(m);
                MonthCount {
                    count: by_month.get(&key).copied().unwrap_or(0),
                    month: key,
                }
            })
            .collect();
        let personal = th
            .members
            .iter()
            .filter(|&&m| self.notes[m].kind == NoteKind::Personal)
            .count();
        ThemeSummary {
            id: th.id.clone(),
            name: th.name.clone(),
            keywords: th.keywords.clone(),
            labels: th.labels.clone(),
            note_count: th.members.len(),
            personal_count: personal,
            source_count: th.members.len() - personal,
            first_day: days.first().map(|d| d.to_string()).unwrap_or_default(),
            last_day: days.last().map(|d| d.to_string()).unwrap_or_default(),
            active_months: by_month.len(),
            months,
        }
    }

    /// Group themes into the overview's sections (indices into `themes`).
    pub fn sections(&self) -> Sections {
        let today = self.today.unwrap_or_default();
        let mut s = Sections::default();
        for (ti, th) in self.themes.iter().enumerate() {
            let first = self.notes[th.members[0]].day;
            let last = self.notes[*th.members.last().unwrap()].day;
            let months: HashSet<(i32, u32)> = th
                .members
                .iter()
                .map(|&m| (self.notes[m].day.year(), self.notes[m].day.month()))
                .collect();
            if (today - first).num_days() <= FORMING_DAYS {
                s.forming.push(ti);
            } else if (today - last).num_days() >= DORMANT_DAYS {
                s.dormant.push(ti);
            } else if months.len() >= 3 {
                s.returning.push(ti);
            } else {
                s.other.push(ti);
            }
        }
        let months_of = |ti: &usize| {
            self.themes[*ti]
                .members
                .iter()
                .map(|&m| (self.notes[m].day.year(), self.notes[m].day.month()))
                .collect::<HashSet<_>>()
                .len()
        };
        let last_of = |ti: &usize| self.notes[*self.themes[*ti].members.last().unwrap()].created_ms;
        s.returning
            .sort_by(|a, b| months_of(b).cmp(&months_of(a)).then(last_of(b).cmp(&last_of(a))));
        for v in [&mut s.forming, &mut s.dormant, &mut s.other] {
            v.sort_by_key(|ti| std::cmp::Reverse(last_of(ti)));
        }
        s
    }

    /// Source notes that later personal notes appear to build on, most
    /// influential first: `(source index, later personal indices)`.
    pub fn influences(&self) -> Vec<(usize, Vec<usize>)> {
        let mut by_source: HashMap<usize, Vec<usize>> = HashMap::new();
        for (&p, l) in &self.influenced_by {
            by_source.entry(l.to).or_default().push(p);
        }
        let mut v: Vec<(usize, Vec<usize>)> = by_source
            .into_iter()
            .map(|(s, mut ps)| {
                ps.sort_by_key(|&p| self.notes[p].created_ms);
                (s, ps)
            })
            .collect();
        v.sort_by(|a, b| {
            b.1.len()
                .cmp(&a.1.len())
                .then(self.notes[b.0].created_ms.cmp(&self.notes[a.0].created_ms))
        });
        v
    }
}

#[derive(Debug, Default)]
pub(crate) struct Sections {
    pub returning: Vec<usize>,
    pub forming: Vec<usize>,
    pub dormant: Vec<usize>,
    pub other: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Influence {
    pub source: NoteSummary,
    pub later: Vec<NoteSummary>,
    pub theme_id: Option<String>,
    pub theme_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OpenQuestion {
    pub note: NoteSummary,
    pub theme_id: Option<String>,
    pub theme_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IdeasOverview {
    /// False when semantic features are off.
    pub enabled: bool,
    /// True when notes changed since this was computed (an update is on its way).
    pub stale: bool,
    pub returning: Vec<ThemeSummary>,
    pub forming: Vec<ThemeSummary>,
    pub dormant: Vec<ThemeSummary>,
    pub other: Vec<ThemeSummary>,
    pub influences: Vec<Influence>,
    pub open_questions: Vec<OpenQuestion>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    pub note: NoteSummary,
    /// An earlier source note this one may build on.
    pub influenced_by: Option<NoteSummary>,
    /// An earlier personal note this one returns to.
    pub revisits: Option<NoteSummary>,
    /// The note returns to an earlier thought with words that suggest rethinking it.
    pub may_revise: bool,
    pub is_question: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThemeTimeline {
    pub theme: ThemeSummary,
    /// Oldest first.
    pub entries: Vec<TimelineEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_tokens() {
        let t = tokens("Ownership seems to matter: fund returns and the power-law of returns.");
        assert!(t.contains(&"ownership".to_string()));
        assert!(t.contains(&"return".to_string()));
        assert!(t.contains(&"power-law".to_string()));
        assert!(t.contains(&"fund return".to_string()));
        assert!(!t.iter().any(|w| w == "the" || w == "seems"));
    }

    #[test]
    fn question_and_revision_signals() {
        assert!(is_question("Is good company the right framework?"));
        assert!(is_question("Open question: concentration or diversification"));
        assert!(!is_question("See https://example.com/?q=1 for details"));
        assert!(signals_revision("Tension with my March note."));
        assert!(!signals_revision("Ownership matters a lot."));
    }
}
