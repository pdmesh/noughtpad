//! Prompts for chatting with the user's own notes, and citation parsing.
//!
//! Notes get a number the first time the model sees them in a chat (`[1]`,
//! `[2]`…), and keep it for the rest of the chat. The app shows which notes
//! were read, so answers refer to a number only when it helps.

use serde::{Deserialize, Serialize};

/// One note given to the model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextNote {
    pub id: String,
    /// `YYYY-MM-DD`
    pub day: String,
    pub title: Option<String>,
    /// Title of the source (book, article…) if this note is about one.
    pub source: Option<String>,
    pub labels: Vec<String>,
    pub body: String,
}

/// Longest body (in characters) sent per note when the model reads it in full.
pub const NOTE_CHARS: usize = 2_000;
/// Snippet length in search results.
pub const SNIPPET_CHARS: usize = 220;

const BASE: &str = "You help a person think with their own notebook. \
Answer the question directly and concisely, using only what their notes say. \
The person can already see which notes you looked at, so don't list sources, \
don't summarise each note in turn, and don't repeat the question. \
Refer to a note by its number in square brackets, like [3], only where pointing \
at a specific note helps. \
If the notes don't cover something, say so plainly instead of guessing. \
Speak to the person in the second person (\"you wrote…\"). \
Short paragraphs or a few bullets; no headings, no preamble. \
Never invent facts, quotes, dates or sources. \
Notes marked \"from <source>\" record what the person read; \"your own note\" is their own thinking.";

const WITH_TOOLS: &str = " Use the tools to find and read notes before answering: \
search with a few different phrasings if the first search misses, and read a note \
in full when its snippet isn't enough. Don't describe your tool use in the answer.";

/// The system prompt; `tools` when the model can search and read notes itself.
pub fn system(tools: bool) -> String {
    if tools {
        format!("{BASE}{WITH_TOOLS}")
    } else {
        BASE.to_string()
    }
}

fn header(n: usize, note: &ContextNote) -> String {
    let mut out = format!("[{n}] {}", note.day);
    match &note.source {
        Some(s) => out.push_str(&format!(" · from {s}")),
        None => out.push_str(" · your own note"),
    }
    if !note.labels.is_empty() {
        out.push_str(&format!(" · labels: {}", note.labels.join(", ")));
    }
    if let Some(t) = note.title.as_deref().filter(|t| !t.trim().is_empty()) {
        out.push_str(&format!(" · title: {t}"));
    }
    out
}

/// A note in full (clipped to [`NOTE_CHARS`]).
pub fn full(n: usize, note: &ContextNote) -> String {
    format!("{}\n{}", header(n, note), clip(note.body.trim(), NOTE_CHARS))
}

/// One line per note, for search results.
pub fn snippet(n: usize, note: &ContextNote) -> String {
    let flat: String = note.body.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut = clip(&flat, SNIPPET_CHARS);
    let more = if cut.len() < flat.len() { "…" } else { "" };
    format!("{}\n{cut}{more}", header(n, note))
}

/// The user message when the app chose the notes (local models, or providers
/// without tool support): the notes in full, then the question.
pub fn with_notes(question: &str, notes: &[(usize, ContextNote)]) -> String {
    if notes.is_empty() {
        return format!("(No notes matched this question.)\n\nQuestion: {}", question.trim());
    }
    let mut out = String::from("Notes:\n\n");
    for (n, note) in notes {
        out.push_str(&full(*n, note));
        out.push_str("\n\n");
    }
    out.push_str("Question: ");
    out.push_str(question.trim());
    out
}

fn clip(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        None => s,
        Some((cut, _)) => {
            let head = &s[..cut];
            head.rfind(char::is_whitespace).map_or(head, |i| &head[..i])
        }
    }
}

/// Note numbers cited in `text`, in first-mention order, ignoring numbers
/// outside `1..=n`. Accepts `[2]`, `[1, 3]` and `[1][3]`.
pub fn citations(text: &str, n: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        let Some(close) = rest.find(']') else { break };
        let inner = &rest[..close];
        if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit() || c == ',' || c == ' ') {
            for part in inner.split(',') {
                if let Ok(k) = part.trim().parse::<usize>() {
                    if (1..=n).contains(&k) && !out.contains(&k) {
                        out.push(k);
                    }
                }
            }
        }
        rest = &rest[close + 1..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(day: &str, body: &str, source: Option<&str>) -> ContextNote {
        ContextNote {
            id: day.into(),
            day: day.into(),
            title: None,
            source: source.map(Into::into),
            labels: vec!["VC".into()],
            body: body.into(),
        }
    }

    #[test]
    fn notes_keep_their_numbers_and_kind() {
        let m = with_notes(
            " What changed? ",
            &[
                (3, note("2026-01-14", "Power law.", Some("Venture Deals"))),
                (7, note("2026-03-03", "Ownership matters.", None)),
            ],
        );
        assert!(m.contains("[3] 2026-01-14 · from Venture Deals · labels: VC\nPower law."));
        assert!(m.contains("[7] 2026-03-03 · your own note"));
        assert!(m.ends_with("Question: What changed?"));
    }

    #[test]
    fn clips_and_flattens() {
        let body = "word\n".repeat(2_000);
        assert!(full(1, &note("2026-01-01", &body, None)).len() < NOTE_CHARS + 100);
        let s = snippet(1, &note("2026-01-01", &body, None));
        assert!(s.ends_with('…') && !s.lines().nth(1).unwrap().contains('\n'));
    }

    #[test]
    fn tool_prompt_mentions_tools_and_no_source_lists() {
        assert!(system(true).contains("Use the tools"));
        assert!(!system(false).contains("tools"));
        assert!(system(false).contains("don't list sources"));
    }

    #[test]
    fn parses_citations() {
        assert_eq!(
            citations("A [2]. B [1][3], C [1, 4] and [9] [x] [ ]", 4),
            vec![2, 1, 3, 4]
        );
        assert_eq!(citations("unclosed [2", 4), Vec::<usize>::new());
    }
}
