//! Real-model check: indexing speed on a synthetic vault, and related-note
//! quality on a small hand-written notebook.
//!
//! cargo run --release -p noughtpad-core --features embedding-static --example semantic -- <model-dir> [notes]

use std::sync::Arc;
use std::time::Instant;

use noughtpad_core::semantic::model::{download, FastEmbedder, DEFAULT_MODEL};
use noughtpad_core::semantic::Semantic;
use noughtpad_core::userstate::UserState;
use noughtpad_core::vault::{EditableMeta, NewNote};
use noughtpad_core::Vault;

const NOTEBOOK: &[(&str, &str)] = &[
    ("VC", "Read about VC power-law returns. A handful of outlier companies return the entire fund."),
    ("VC", "Ownership seems to matter more than I thought. If one company returns the fund, entry ownership decides how much."),
    ("VC", "Portfolio construction: reserves for follow-ons, number of initial checks, concentration versus diversification."),
    ("VC", "Is 'good company' the right framework? The question is whether the investment can produce a meaningful fund outcome."),
    ("Product", "Customers describe what they want, not what they actually do. Ask about past behaviour, not hypotheticals."),
    ("Product", "User interviews for the editor: people complained about folders; nobody mentioned search."),
    ("Product", "Onboarding should get someone writing in under ten seconds. No setup screens."),
    ("Travel", "Tokyo itinerary: three days, Shibuya, Asakusa, day trip to Nikko."),
    ("Travel", "Kyoto: book a ryokan near Gion, visit Fushimi Inari early in the morning to avoid crowds."),
    ("Cooking", "Sourdough starter needs feeding twice a day at room temperature; the dough was too slack."),
    ("Cooking", "Bread hydration around 70% made a much better crumb than the last loaf."),
    ("Decaframe", "Presentation software should let slides flow like documents, with layouts chosen by content."),
    ("Decaframe", "Idea for organising slide templates by what the content says rather than visual style."),
];

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().expect("model dir"));
    let n: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(2000);
    download(&DEFAULT_MODEL, &dir, |_, _| {}).unwrap();
    let threads = std::thread::available_parallelism().map_or(2, |p| (p.get() / 2).clamp(1, 4));

    // ---- quality on a realistic notebook -------------------------------
    let tmp = tempfile::tempdir().unwrap();
    let vault = Arc::new(Vault::open(tmp.path()).unwrap());
    let ids: Vec<String> = NOTEBOOK
        .iter()
        .map(|(label, text)| {
            vault
                .create(NewNote {
                    content: text.to_string(),
                    meta: EditableMeta {
                        labels: vec![label.to_string()],
                        ..Default::default()
                    },
                })
                .unwrap()
                .id
        })
        .collect();
    let sem = Semantic::new(vault.clone(), Arc::new(UserState::open_in_memory().unwrap())).unwrap();
    sem.set_embedder(Box::new(FastEmbedder::load(&DEFAULT_MODEL, &dir, threads).unwrap()))
        .unwrap();
    sem.index_now().unwrap();
    let mut right = 0;
    let mut wrong = 0;
    for (i, id) in ids.iter().enumerate() {
        let r = sem.related(id, 5).unwrap();
        let mine = NOTEBOOK[i].0;
        println!("\n[{mine}] {}", &NOTEBOOK[i].1[..60.min(NOTEBOOK[i].1.len())]);
        for item in &r.items {
            let same = item.note.labels.iter().any(|l| l == mine);
            if same {
                right += 1
            } else {
                wrong += 1
            }
            println!(
                "   {} {:.3} {:?} [{}] {}",
                if same { "✓" } else { "✗" },
                item.score,
                item.strength,
                item.note.labels.join(","),
                &item.passage[..60.min(item.passage.len())]
            );
        }
    }
    println!("\nrelated suggestions in the same topic: {right}, other topic: {wrong}");
    for q in [
        "how much of the fund comes from big winners",
        "talking to users",
        "baking bread",
    ] {
        let hits = sem.search(q, 3).unwrap();
        println!(
            "search {q:?}: {:?}",
            hits.iter()
                .map(|h| (h.note.labels.join(","), format!("{:?}", h.matched)))
                .collect::<Vec<_>>()
        );
    }

    // ---- indexing speed on a larger synthetic vault --------------------
    let tmp = tempfile::tempdir().unwrap();
    let vault = Arc::new(Vault::open(tmp.path()).unwrap());
    for i in 0..n {
        let (label, text) = NOTEBOOK[i % NOTEBOOK.len()];
        let content = format!(
            "{text} Note {i}: further thoughts on {} and what it means for next steps.",
            label.to_lowercase()
        );
        vault
            .create(NewNote {
                content,
                meta: EditableMeta {
                    labels: vec![label.into()],
                    ..Default::default()
                },
            })
            .unwrap();
    }
    let sem = Semantic::new(vault.clone(), Arc::new(UserState::open_in_memory().unwrap())).unwrap();
    sem.set_embedder(Box::new(FastEmbedder::load(&DEFAULT_MODEL, &dir, threads).unwrap()))
        .unwrap();
    let t = Instant::now();
    sem.index_now().unwrap();
    let secs = t.elapsed().as_secs_f64();
    println!(
        "\nindexed {n} notes in {secs:.1}s ({:.0} notes/s, {threads} threads)",
        n as f64 / secs
    );
    let id = vault.list(&Default::default()).unwrap()[0].id.clone();
    let t = Instant::now();
    let r = sem.related(&id, 8).unwrap();
    println!("related for one note: {:?} ({} items)", t.elapsed(), r.items.len());
    let t = Instant::now();
    let hits = sem.search("fund outcomes", 20).unwrap();
    println!(
        "hybrid search incl. query embedding: {:?} ({} hits)",
        t.elapsed(),
        hits.len()
    );
}
