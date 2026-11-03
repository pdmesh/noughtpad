//! Print themes, sections and a timeline for a notebook using the real model.
//!
//! cargo run --release -p nought-core --features embedding-static --example ideas -- <models-dir> <vault> [YYYY-MM-DD]

use std::sync::Arc;

use nought_core::index::NoteSummary;
use nought_core::semantic::model::{download_bundle, model_dir, FastEmbedder, DEFAULT_MODEL};
use nought_core::semantic::Semantic;
use nought_core::userstate::UserState;
use nought_core::Vault;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, vault_path) = (std::path::PathBuf::from(&args[0]), &args[1]);
    let today = args
        .get(2)
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    download_bundle(&dir, |_, _| {}).unwrap();

    // Work on a copy so the source folder is never touched.
    let tmp = tempfile::tempdir().unwrap();
    for e in walkdir::WalkDir::new(vault_path)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
    {
        let to = tmp.path().join(e.path().strip_prefix(vault_path).unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(e.path(), to).unwrap();
    }
    let vault = Arc::new(Vault::open(tmp.path()).unwrap());
    vault.rescan().unwrap();
    let sem = Semantic::new(vault.clone(), Arc::new(UserState::open_in_memory().unwrap())).unwrap();
    sem.set_embedder(Box::new(
        FastEmbedder::load(&DEFAULT_MODEL, &model_dir(&dir), 2).unwrap(),
    ))
    .unwrap();
    sem.index_now().unwrap();
    sem.refresh_ideas_at(today).unwrap();
    let o = sem.ideas_overview().unwrap();
    for (title, list) in [
        ("RETURNING", &o.returning),
        ("FORMING", &o.forming),
        ("DORMANT", &o.dormant),
        ("OTHER", &o.other),
    ] {
        println!("\n== {title}");
        for t in list {
            println!(
                "  {} | labels {:?} | {} notes ({} personal, {} source) | {} months | {} → {}",
                t.name,
                t.labels,
                t.note_count,
                t.personal_count,
                t.source_count,
                t.active_months,
                t.first_day,
                t.last_day
            );
        }
    }
    println!("\n== INFLUENCES");
    for i in &o.influences {
        println!(
            "  {} → {:?}",
            short(&i.source.title),
            i.later.iter().map(|n| short(&n.title)).collect::<Vec<_>>()
        );
    }
    println!("\n== OPEN QUESTIONS");
    for q in &o.open_questions {
        println!("  [{}] {}", q.note.day, short(&q.note.title));
    }
    for q in [
        "how much of the fund comes from big winners",
        "talking to users",
        "baking bread",
        "japan trip",
        "getting fit",
        "tools for thinking",
    ] {
        let t = std::time::Instant::now();
        let hits = sem.search(q, 4).unwrap();
        println!(
            "\n== SEARCH {q:?} ({:?})\n{}",
            t.elapsed(),
            hits.iter()
                .map(|h| format!(
                    "   {:.3} {:?} [{}] {}",
                    h.score,
                    h.matched,
                    h.note.labels.join(","),
                    short(&h.note.title)
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    print_related(&sem, &vault, "\"Ownership may matter more than I expected\"");
    print_related(&sem, &vault, "sourdough starter");
    print_related(&sem, &vault, "Tokyo itinerary draft");
    if let Some(t) = o.returning.first() {
        println!("\n== TIMELINE: {}", t.name);
        for e in sem.theme_timeline(&t.id).unwrap().unwrap().entries {
            println!(
                "  {} {:?} {}{}{}{}",
                e.note.day,
                e.note.kind,
                short(&e.note.title),
                e.influenced_by
                    .map(|n| format!("  ⟵ builds on: {}", short(&n.title)))
                    .unwrap_or_default(),
                e.revisits
                    .map(|n| format!("  ↺ returns to: {}", short(&n.title)))
                    .unwrap_or_default(),
                if e.may_revise { "  (may revise)" } else { "" }
            );
        }
    }
}

fn print_related(sem: &Semantic, vault: &Vault, needle: &str) {
    let hits = vault.search(needle, 1).unwrap();
    let Some(n): Option<&NoteSummary> = hits.first().map(|h| &h.note) else {
        return;
    };
    let t = std::time::Instant::now();
    let r = sem.related(&n.id, 8).unwrap();
    println!("\n== RELATED to «{}» ({:?})", short(&n.title), t.elapsed());
    for i in r.items {
        println!(
            "  {:.3} {:?} [{}] {}",
            i.score,
            i.strength,
            i.note.labels.join(","),
            short(&i.note.title)
        );
    }
}

fn short(s: &str) -> String {
    s.chars().take(50).collect()
}
