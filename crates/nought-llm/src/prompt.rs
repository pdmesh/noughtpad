//! Grounded prompts over the user's own notes, and citation parsing.
//!
//! Notes are numbered `[1]..[n]` in the prompt; the model must cite every
//! claim with those numbers, and the app turns them back into note links.

use serde::{Deserialize, Serialize};

/// One note given to the model as context.
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

/// Longest body (in characters) sent per note; the rest is cut at a word boundary.
pub const NOTE_CHARS: usize = 1_400;

pub const SYSTEM: &str = "You help a person think with their own notebook. \
Use ONLY the numbered notes you are given. \
Cite every claim with the note's number in square brackets, like [2] or [1][3]. \
If the notes don't answer something, say so plainly instead of guessing. \
Speak to the person in the second person (\"you wrote…\", \"you asked…\"). \
Be brief and concrete: short paragraphs or a few bullet points, no headings, no preamble. \
Never invent facts, quotes, dates or sources. \
Notes marked \"from <source>\" record what the person read; notes marked \"your own note\" are their own thinking.";

/// The user message: numbered notes, then the question.
pub fn user_message(question: &str, notes: &[ContextNote]) -> String {
    let mut out = String::from("Notes, oldest first:\n\n");
    for (i, n) in notes.iter().enumerate() {
        out.push_str(&format!("[{}] {}", i + 1, n.day));
        match &n.source {
            Some(s) => out.push_str(&format!(" · from {s}")),
            None => out.push_str(" · your own note"),
        }
        if !n.labels.is_empty() {
            out.push_str(&format!(" · labels: {}", n.labels.join(", ")));
        }
        out.push('\n');
        if let Some(t) = n.title.as_deref().filter(|t| !t.trim().is_empty()) {
            out.push_str(&format!("Title: {t}\n"));
        }
        out.push_str(clip(n.body.trim(), NOTE_CHARS));
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

/// Note numbers (1-based) cited in `text`, in first-mention order, ignoring
/// numbers outside `1..=n`. Accepts `[2]`, `[1, 3]` and `[1][3]`.
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
    fn numbers_notes_and_marks_sources() {
        let m = user_message(
            " What changed? ",
            &[
                note("2026-01-14", "Power law.", Some("Venture Deals")),
                note("2026-03-03", "Ownership matters.", None),
            ],
        );
        assert!(m.contains("[1] 2026-01-14 · from Venture Deals · labels: VC\nPower law."));
        assert!(m.contains("[2] 2026-03-03 · your own note"));
        assert!(m.ends_with("Question: What changed?"));
    }

    #[test]
    fn clips_long_notes_at_a_word() {
        let body = "word ".repeat(1_000);
        let m = user_message("q", &[note("2026-01-01", &body, None)]);
        assert!(m.len() < NOTE_CHARS + 200);
        assert!(!m.contains("wor\n"));
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
