//! Calibrate judge models on a notebook. For every note pair the embeddings
//! consider plausible (similarity ≥ 0.5) it prints the embedding similarity,
//! the reranker relevance in both directions and, with `--nli`, the inference
//! model's contradiction/entailment probabilities in both directions.
//! Pairs whose notes share a label are marked `Y` as a rough "truly related".
//!
//! cargo run --release -p nought-core --features embedding-static --example judges -- <models-dir> <vault> [--nli]

use nought_core::note::Note;
use nought_core::semantic::embedder::{dot, Embedder, TextKind};
use nought_core::semantic::judge::Judge;
use nought_core::semantic::model::{
    bundle_dirs, download_bundle, download_optional, FastEmbedder, FastJudge, DEFAULT_MODEL, NLI,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let models = std::path::PathBuf::from(&args[0]);
    let with_nli = args.iter().any(|a| a == "--nli");
    download_bundle(&models, |_, _| {}).unwrap();
    if with_nli {
        download_optional(&NLI, &models, |_, _| {}).unwrap();
    }
    let (edir, _, _) = bundle_dirs(&models);
    let mut emb = FastEmbedder::load(&DEFAULT_MODEL, &edir, 2).unwrap();
    let mut judge = FastJudge::load(&models, 2).unwrap();

    let mut notes: Vec<(String, String, Vec<String>)> = Vec::new(); // (created, text, labels)
    for e in walkdir::WalkDir::new(&args[1])
        .into_iter()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
    {
        let n = Note::parse(&std::fs::read_to_string(e.path()).unwrap());
        let created = n.meta.created.map(|c| c.to_rfc3339()).unwrap_or_default();
        notes.push((created, n.content().trim().to_string(), n.meta.labels.clone()));
    }
    notes.sort();
    let texts: Vec<String> = notes.iter().map(|n| n.1.clone()).collect();
    let v = emb.embed(&texts, TextKind::Passage).unwrap();

    let started = std::time::Instant::now();
    let mut rows = Vec::new();
    for i in 0..notes.len() {
        for j in (i + 1)..notes.len() {
            let sim = dot(&v[i], &v[j]);
            if sim < 0.50 {
                continue;
            }
            let ab = judge.rerank(&texts[i], std::slice::from_ref(&texts[j])).unwrap()[0];
            let ba = judge.rerank(&texts[j], std::slice::from_ref(&texts[i])).unwrap()[0];
            let contra = if judge.has_nli() {
                let n = judge
                    .nli(&[
                        (texts[i].clone(), texts[j].clone()),
                        (texts[j].clone(), texts[i].clone()),
                    ])
                    .unwrap();
                Some((n[0].contradict, n[1].contradict))
            } else {
                None
            };
            let same = notes[i].2.iter().any(|l| notes[j].2.contains(l));
            rows.push((sim, (ab + ba) / 2.0, ab, ba, contra, same, i, j));
        }
    }
    eprintln!("{} pairs judged in {:?}", rows.len(), started.elapsed());
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    let short = |s: &str| s.chars().take(40).collect::<String>().replace('\n', " ");
    println!("same embed  relevance (a→b b→a)  contra(a→b b→a) | earlier → later");
    for (sim, rel, ab, ba, contra, same, i, j) in rows {
        let c = contra.map_or(String::from("   -    -  "), |(x, y)| format!("{x:.2} {y:.2}"));
        println!(
            "{}    {sim:.3}  {rel:.3} ({ab:.2} {ba:.2})       {c} | {} → {}",
            if same { "Y" } else { "-" },
            short(&texts[i]),
            short(&texts[j])
        );
    }
}
