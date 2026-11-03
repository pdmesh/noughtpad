//! Ask a downloaded local model one grounded question (manual check).
//!
//! cargo run --release -p noughtpad-llm --features local --example ask -- <models-dir> <model-id>

use std::sync::atomic::AtomicBool;

use noughtpad_llm::catalog;
use noughtpad_llm::local::LocalRuntime;
use noughtpad_llm::prompt::{self, ContextNote};
use noughtpad_llm::ChatRequest;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let models = std::path::PathBuf::from(&args[1]);
    let m = catalog::find(&args[2]).expect("model id from the catalog");
    let note = |day: &str, body: &str, source: Option<&str>| ContextNote {
        id: day.into(),
        day: day.into(),
        title: None,
        source: source.map(Into::into),
        labels: vec!["VC".into()],
        body: body.into(),
    };
    let notes = [
        note("2026-01-14", "Read about VC power-law returns. A handful of outliers return the fund.", Some("Venture Deals")),
        note("2026-03-03", "Ownership may matter more than I expected. If one company returns the fund, entry ownership decides how much it returns.", None),
        note("2026-05-22", "Talked to an angel friend: she thinks ownership targets make funds pass on the best rounds. Tension with my March note.", None),
    ];
    let numbered: Vec<(usize, ContextNote)> = notes.iter().cloned().enumerate().map(|(i, n)| (i + 1, n)).collect();
    let req = ChatRequest {
        system: prompt::system(false),
        history: vec![],
        user: prompt::with_notes("How has my thinking about ownership changed?", &numbered),
        max_tokens: 300,
        temperature: 0.2,
    };
    let rt = LocalRuntime::default();
    let t = std::time::Instant::now();
    let mut out = String::new();
    let usage = rt
        .chat(&models, m, &req, &AtomicBool::new(false), &mut |s| {
            print!("{s}");
            out.push_str(s);
        })
        .await
        .expect("answer");
    println!("\n\n{usage:?} total {:.1}s", t.elapsed().as_secs_f32());
    println!("cited: {:?}", prompt::citations(&out, notes.len()));
}
