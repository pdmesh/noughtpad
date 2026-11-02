//! Split a note into passages for embedding.
//!
//! Most notes are a few sentences and become a single chunk. Longer notes are
//! split on paragraph boundaries, merging short paragraphs, so each chunk is a
//! coherent passage that can be quoted back as the reason for a connection.

/// Soft target for a chunk, in characters (~250 tokens for English).
const TARGET: usize = 900;
/// Paragraphs longer than this are split on sentence boundaries.
const MAX: usize = 1400;

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub ord: u32,
    pub text: String,
}

pub fn chunk(title: Option<&str>, body: &str) -> Vec<Chunk> {
    let mut pieces: Vec<String> = Vec::new();
    for para in paragraphs(body) {
        if para.chars().count() > MAX {
            pieces.extend(split_sentences(&para, TARGET));
        } else {
            pieces.push(para);
        }
    }

    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for p in pieces {
        if !cur.is_empty() && cur.chars().count() + p.chars().count() > TARGET {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str("\n\n");
        }
        cur.push_str(&p);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    // An explicit title gives the first passage its context.
    if let Some(t) = title.map(str::trim).filter(|t| !t.is_empty()) {
        match out.first_mut() {
            Some(first) => *first = format!("{t}\n\n{first}"),
            None => out.push(t.to_string()),
        }
    }
    out.into_iter()
        .enumerate()
        .map(|(i, text)| Chunk { ord: i as u32, text })
        .collect()
}

/// Paragraphs with light Markdown cleanup; fenced code stays one block.
fn paragraphs(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut in_fence = false;
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_fence = !in_fence;
            cur.push(line);
            continue;
        }
        if !in_fence && t.is_empty() {
            if !cur.is_empty() {
                out.push(clean(&cur.join("\n")));
                cur.clear();
            }
        } else {
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        out.push(clean(&cur.join("\n")));
    }
    out.retain(|p| !p.trim().is_empty());
    out
}

/// Drop Markdown syntax that carries no meaning (image links, heading marks).
fn clean(p: &str) -> String {
    p.lines()
        .map(|l| {
            let l = l.trim_end();
            let l = l.trim_start_matches(['#', '>']).trim_start();
            l.replace("- [ ] ", "").replace("- [x] ", "")
        })
        .filter(|l| !(l.starts_with("![") && l.ends_with(')')))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn split_sentences(p: &str, target: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, ch) in p.char_indices() {
        cur.push(ch);
        let boundary = matches!(ch, '.' | '!' | '?') && p[i + ch.len_utf8()..].starts_with(char::is_whitespace);
        if (boundary && cur.chars().count() >= target) || cur.chars().count() >= MAX {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_note_is_one_chunk() {
        let c = chunk(
            None,
            "## Ownership\n\nOwnership seems to matter more.\n\n- [ ] check maths\n",
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "Ownership\n\nOwnership seems to matter more.\n\ncheck maths");
    }

    #[test]
    fn title_prefixes_first_chunk() {
        let c = chunk(Some("Fund maths"), "body");
        assert_eq!(c[0].text, "Fund maths\n\nbody");
        assert_eq!(chunk(Some("Only a title"), "")[0].text, "Only a title");
        assert!(chunk(None, "  \n\n ").is_empty());
    }

    #[test]
    fn long_notes_split_on_paragraphs_and_sentences() {
        let para = "This is a sentence about portfolio construction. ".repeat(12);
        let body = format!("{para}\n\n{para}\n\n{para}");
        let c = chunk(None, &body);
        assert!(c.len() >= 3, "{}", c.len());
        assert!(c.iter().all(|c| c.text.chars().count() <= MAX + 60));
        let huge = "word ".repeat(1000);
        assert!(chunk(None, &huge).len() >= 4);
    }
}
