//! Rough performance check on a synthetic vault.
//!
//! cargo run --release -p nought-core --example bench -- 10000

use std::time::Instant;

use nought_core::index::ListQuery;
use nought_core::Vault;

const WORDS: &[&str] = &[
    "ownership",
    "fund",
    "portfolio",
    "power",
    "law",
    "returns",
    "product",
    "customer",
    "interview",
    "travel",
    "kyoto",
    "presentation",
    "architecture",
    "markdown",
    "editor",
    "idea",
    "question",
    "thinking",
    "book",
    "decision",
];

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let dir = tempfile::tempdir().unwrap();
    for i in 0..n {
        let month = 1 + i % 12;
        let sub = dir.path().join(format!("2026/{month:02}"));
        std::fs::create_dir_all(&sub).unwrap();
        let body: Vec<&str> = (0..60).map(|k| WORDS[(i * 7 + k * 13) % WORDS.len()]).collect();
        let text = format!(
            "---\nid: N{i:08}\ncreated: 2026-{month:02}-{:02}T10:00:00+05:30\nlabels: [L{}]\n---\n\n{}\n",
            1 + i % 28,
            i % 15,
            body.join(" ")
        );
        std::fs::write(sub.join(format!("note-{i}.md")), text).unwrap();
    }
    let vault = Vault::open(dir.path()).unwrap();

    let t = Instant::now();
    let r = vault.rescan().unwrap();
    println!("first scan of {n} notes: {:?} ({} indexed)", t.elapsed(), r.indexed);

    let t = Instant::now();
    let r = vault.rescan().unwrap();
    println!("no-op rescan: {:?} ({} unchanged)", t.elapsed(), r.unchanged);

    let t = Instant::now();
    let hits = vault.search("ownership portf", 50).unwrap();
    println!("search: {:?} ({} hits)", t.elapsed(), hits.len());

    let t = Instant::now();
    let page = vault
        .list(&ListQuery {
            limit: Some(60),
            ..Default::default()
        })
        .unwrap();
    println!("stream page: {:?} ({} notes)", t.elapsed(), page.len());

    let t = Instant::now();
    let log = vault.month_log("2026-03").unwrap();
    println!("month log: {:?} ({} personal notes)", t.elapsed(), log.personal_count);

    let t = Instant::now();
    let doc = vault.create(Default::default()).unwrap();
    println!("create note: {:?} ({})", t.elapsed(), doc.path);
}
