//! The note file format.
//!
//! A note is a Markdown file with optional, minimal YAML frontmatter:
//!
//! ```markdown
//! ---
//! id: 01J8ZK4M7Q3V9X2T6B5N8C1D0E
//! created: 2026-09-12T23:10:00+05:30
//! labels: [VC, Product]
//! source:
//!   title: The Mom Test
//!   url: https://example.com
//! ---
//!
//! Body text, stored exactly as the user typed it.
//! ```
//!
//! Guarantees:
//! * The body is never altered: `parse(render(n)).body == n.body` byte-for-byte.
//! * If metadata did not change, the original frontmatter text is written back
//!   verbatim (formatting, comments and key order survive).
//! * Unknown frontmatter keys are preserved when metadata is rewritten.

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use yaml_serde::{Mapping, Value};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    /// The user's own thought, question, idea or journal entry.
    Personal,
    /// Material connected to an external source (book, article, video…).
    Source,
}

impl NoteKind {
    pub fn as_str(self) -> &'static str {
        match self {
            NoteKind::Personal => "personal",
            NoteKind::Source => "source",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "personal" => Some(NoteKind::Personal),
            "source" | "external" => Some(NoteKind::Source),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Source {
    pub fn is_empty(&self) -> bool {
        blank(&self.title) && blank(&self.url)
    }
}

fn blank(s: &Option<String>) -> bool {
    s.as_deref().is_none_or(|s| s.trim().is_empty())
}

/// User-authored, stable metadata. Everything here is optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NoteMeta {
    /// Stable identity (ULID). `None` for files created outside Nought.
    pub id: Option<String>,
    pub created: Option<DateTime<FixedOffset>>,
    pub labels: Vec<String>,
    pub source: Option<Source>,
    /// Explicit override of the inferred kind. Usually absent.
    pub kind: Option<NoteKind>,
}

impl NoteMeta {
    /// Kind is inferred from the presence of a source unless explicitly set.
    pub fn effective_kind(&self) -> NoteKind {
        self.kind.unwrap_or(match &self.source {
            Some(s) if !s.is_empty() => NoteKind::Source,
            _ => NoteKind::Personal,
        })
    }

    /// Normalise labels: trim, drop empties, de-duplicate case-insensitively
    /// (first spelling wins), keep the user's order.
    pub fn normalise(&mut self) {
        let mut seen = std::collections::HashSet::new();
        self.labels = std::mem::take(&mut self.labels)
            .into_iter()
            .map(|l| l.trim().trim_start_matches('#').trim().to_string())
            .filter(|l| !l.is_empty() && seen.insert(l.to_lowercase()))
            .collect();
        if let Some(src) = &mut self.source {
            for f in [&mut src.title, &mut src.url] {
                if blank(f) {
                    *f = None;
                } else if let Some(v) = f {
                    *v = v.trim().to_string();
                }
            }
            if src.is_empty() {
                self.source = None;
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.id.is_none()
            && self.created.is_none()
            && self.labels.is_empty()
            && self.source.is_none()
            && self.kind.is_none()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub meta: NoteMeta,
    /// The Markdown body, exactly as written.
    pub body: String,
    /// Original frontmatter block (including both fences) and the meta it
    /// parsed to; used to write it back untouched when meta is unchanged.
    original: Option<(String, NoteMeta)>,
    /// Frontmatter keys Nought does not manage, preserved on rewrite.
    extra: Mapping,
}

impl Note {
    pub fn new(meta: NoteMeta, body: impl Into<String>) -> Self {
        Note {
            meta,
            body: body.into(),
            original: None,
            extra: Mapping::new(),
        }
    }

    /// Parse a note file. Never fails: invalid frontmatter is treated as body
    /// text so that no user content is ever hidden or lost.
    pub fn parse(text: &str) -> Self {
        match split_frontmatter(text) {
            Some((fm, body)) => match parse_meta(fm) {
                Ok((meta, extra)) => Note {
                    original: Some((text[..text.len() - body.len()].to_string(), meta.clone())),
                    meta,
                    body: body.to_string(),
                    extra,
                },
                Err(e) => {
                    tracing::warn!("ignoring invalid frontmatter: {e}");
                    Note::new(NoteMeta::default(), text)
                }
            },
            None => Note::new(NoteMeta::default(), text),
        }
    }

    /// Serialise to file text.
    pub fn render(&self) -> String {
        if let Some((raw, meta)) = &self.original {
            if *meta == self.meta {
                return format!("{raw}{}", self.body);
            }
        }
        if self.meta.is_empty() && self.extra.is_empty() {
            return self.body.clone();
        }
        let yaml = render_meta(&self.meta, &self.extra);
        // A blank line after the fence keeps the file pleasant to read; it is
        // only added for new notes whose body doesn't already start with one.
        let sep = if self.original.is_none() && !self.body.starts_with('\n') {
            "\n"
        } else {
            ""
        };
        format!("---\n{yaml}---\n{sep}{}", self.body)
    }

    /// Body without the leading blank line conventionally placed after frontmatter.
    pub fn content(&self) -> &str {
        self.body
            .strip_prefix("\r\n")
            .or_else(|| self.body.strip_prefix('\n'))
            .unwrap_or(&self.body)
    }

    /// A display title derived from the first non-empty line.
    pub fn title(&self) -> String {
        derive_title(self.content())
    }
}

/// First meaningful line, stripped of Markdown markers, max 120 chars.
pub fn derive_title(body: &str) -> String {
    let line = body
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty() && !l.starts_with("```") && !l.starts_with("---"))
        .unwrap_or("");
    let line = line.trim_start_matches(['#', '>', '-', '*', '+', ' ']);
    let line = line
        .strip_prefix("[ ] ")
        .or_else(|| line.strip_prefix("[x] "))
        .unwrap_or(line);
    let mut out: String = line.chars().take(120).collect();
    if line.chars().count() > 120 {
        out.push('…');
    }
    out
}

/// A filesystem-safe slug of at most `max_words` words.
pub fn slugify(title: &str, max_words: usize) -> String {
    let mut words = Vec::new();
    let mut cur = String::new();
    for ch in title.chars() {
        if ch.is_alphanumeric() {
            cur.extend(ch.to_lowercase());
        } else if ch == '\'' || ch == '’' {
            // "user's" -> "users"
        } else if !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
        if words.len() >= max_words {
            break;
        }
    }
    if !cur.is_empty() && words.len() < max_words {
        words.push(cur);
    }
    let slug = words.join("-");
    let slug: String = slug.chars().take(60).collect();
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "note".into()
    } else {
        slug
    }
}

fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let t = line.trim_end_matches(['\r', '\n']);
        if t == "---" || t == "..." {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    // Closing fence on the final line without trailing newline.
    None
}

fn parse_meta(fm: &str) -> Result<(NoteMeta, Mapping)> {
    let value: Value = if fm.trim().is_empty() {
        Value::Mapping(Mapping::new())
    } else {
        yaml_serde::from_str(fm).map_err(|e| Error::Frontmatter(e.to_string()))?
    };
    let Value::Mapping(map) = value else {
        return Err(Error::Frontmatter("frontmatter is not a mapping".into()));
    };
    let mut meta = NoteMeta::default();
    let mut extra = Mapping::new();
    for (k, v) in map {
        match k.as_str() {
            Some("id") => meta.id = scalar_string(&v),
            Some("created") => {
                meta.created = scalar_string(&v).and_then(|s| parse_datetime(&s));
            }
            Some("labels") => meta.labels = string_list(&v),
            Some("source") => meta.source = parse_source(&v),
            Some("type") => meta.kind = scalar_string(&v).and_then(|s| NoteKind::parse(&s)),
            _ => {
                extra.insert(k, v);
            }
        }
    }
    meta.normalise();
    Ok((meta, extra))
}

fn scalar_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn string_list(v: &Value) -> Vec<String> {
    match v {
        Value::Sequence(seq) => seq.iter().filter_map(scalar_string).collect(),
        // Be forgiving: `labels: VC, Product`
        Value::String(s) => s.split(',').map(str::to_string).collect(),
        _ => Vec::new(),
    }
}

fn parse_source(v: &Value) -> Option<Source> {
    match v {
        Value::Mapping(m) => Some(Source {
            title: m.get("title").and_then(scalar_string),
            url: m.get("url").and_then(scalar_string),
        }),
        // `source: The Mom Test` or `source: https://…`
        Value::String(s) if looks_like_url(s) => Some(Source {
            title: None,
            url: Some(s.clone()),
        }),
        Value::String(s) => Some(Source {
            title: Some(s.clone()),
            url: None,
        }),
        _ => None,
    }
}

pub(crate) fn looks_like_url(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("http://") || s.starts_with("https://")
}

/// Accepts RFC 3339, plus the date-only and naive forms people hand-write.
pub fn parse_datetime(s: &str) -> Option<DateTime<FixedOffset>> {
    let s = s.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt);
    }
    let local = |naive: chrono::NaiveDateTime| {
        use chrono::TimeZone;
        chrono::Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|d| d.fixed_offset())
    };
    for fmt in [
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M",
    ] {
        if let Ok(n) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return local(n);
        }
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| local(d.and_hms_opt(0, 0, 0)?))
}

fn render_meta(meta: &NoteMeta, extra: &Mapping) -> String {
    // Hand-written for known keys so the output is stable and minimal.
    let mut out = String::new();
    if let Some(id) = &meta.id {
        out.push_str(&format!("id: {}\n", yaml_scalar(id)));
    }
    if let Some(c) = &meta.created {
        out.push_str(&format!(
            "created: {}\n",
            c.to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
        ));
    }
    if !meta.labels.is_empty() {
        let items: Vec<String> = meta.labels.iter().map(|l| yaml_scalar(l)).collect();
        out.push_str(&format!("labels: [{}]\n", items.join(", ")));
    }
    if let Some(src) = &meta.source {
        out.push_str("source:\n");
        if let Some(t) = &src.title {
            out.push_str(&format!("  title: {}\n", yaml_scalar(t)));
        }
        if let Some(u) = &src.url {
            out.push_str(&format!("  url: {}\n", yaml_scalar(u)));
        }
    }
    if let Some(k) = meta.kind {
        out.push_str(&format!("type: {}\n", k.as_str()));
    }
    if !extra.is_empty() {
        if let Ok(s) = yaml_serde::to_string(&Value::Mapping(extra.clone())) {
            out.push_str(&s);
            if !out.ends_with('\n') {
                out.push('\n');
            }
        }
    }
    out
}

/// Quote a scalar only when YAML would otherwise misread it.
fn yaml_scalar(s: &str) -> String {
    let plain_ok = !s.is_empty()
        && s == s.trim()
        && !s.starts_with(|c: char| "-?:,[]{}#&*!|>'\"%@`".contains(c))
        && !s.contains(": ")
        && !s.contains(" #")
        && !s.contains(['[', ']', '{', '}', ','])
        && !matches!(
            s.to_ascii_lowercase().as_str(),
            "true" | "false" | "yes" | "no" | "null" | "~" | "on" | "off"
        )
        && s.parse::<f64>().is_err();
    if plain_ok {
        s.to_string()
    } else {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const SAMPLE: &str = "---\nid: 01J8ZK4M7Q3V9X2T6B5N8C1D0E\ncreated: 2026-09-12T23:10:00+05:30\nlabels:\n  - VC\n  - Product\nsource:\n  title: Optional source title\n  url: https://example.com\n# a comment the user wrote\nmood: curious\n---\n\nOwnership seems to matter more than I initially thought in VC.\n\nNeed to understand how this interacts with fund construction.\n";

    #[test]
    fn parses_known_fields() {
        let n = Note::parse(SAMPLE);
        assert_eq!(n.meta.id.as_deref(), Some("01J8ZK4M7Q3V9X2T6B5N8C1D0E"));
        assert_eq!(n.meta.labels, vec!["VC", "Product"]);
        assert_eq!(n.meta.effective_kind(), NoteKind::Source);
        assert_eq!(n.meta.created.unwrap().to_rfc3339(), "2026-09-12T23:10:00+05:30");
        assert_eq!(
            n.title(),
            "Ownership seems to matter more than I initially thought in VC."
        );
    }

    #[test]
    fn unchanged_meta_round_trips_byte_for_byte() {
        let n = Note::parse(SAMPLE);
        assert_eq!(n.render(), SAMPLE);
        let mut n2 = n.clone();
        n2.body.push_str("More.\n");
        assert_eq!(n2.render(), format!("{SAMPLE}More.\n"));
    }

    #[test]
    fn changed_meta_preserves_unknown_keys_and_body() {
        let mut n = Note::parse(SAMPLE);
        n.meta.labels.push("Decaframe".into());
        let out = n.render();
        assert!(out.contains("labels: [VC, Product, Decaframe]\n"));
        assert!(out.contains("mood: curious"));
        let back = Note::parse(&out);
        assert_eq!(back.body, n.body);
        assert_eq!(back.meta, n.meta);
    }

    #[test]
    fn plain_markdown_without_frontmatter() {
        let text = "# Just a file\n\nhello";
        let n = Note::parse(text);
        assert_eq!(n.meta, NoteMeta::default());
        assert_eq!(n.render(), text);
        assert_eq!(n.title(), "Just a file");
    }

    #[test]
    fn invalid_frontmatter_is_kept_as_body() {
        let text = "---\nfoo: [unclosed\n---\nbody";
        let n = Note::parse(text);
        assert_eq!(n.body, text);
        assert_eq!(n.render(), text);
    }

    #[test]
    fn new_note_render() {
        let meta = NoteMeta {
            id: Some("01ABC".into()),
            created: parse_datetime("2026-09-12T23:10:00+05:30"),
            labels: vec!["VC".into(), "yes".into(), "a: b".into()],
            source: None,
            kind: None,
        };
        let n = Note::new(meta.clone(), "Hello\n");
        let out = n.render();
        assert_eq!(
            out,
            "---\nid: 01ABC\ncreated: 2026-09-12T23:10:00+05:30\nlabels: [VC, \"yes\", \"a: b\"]\n---\n\nHello\n"
        );
        let back = Note::parse(&out);
        assert_eq!(back.meta, meta);
        assert_eq!(back.content(), "Hello\n");
    }

    #[test]
    fn forgiving_inputs() {
        let n = Note::parse("---\nlabels: VC, Product , vc\nsource: https://x.y\ncreated: 2026-01-02\n---\nx");
        assert_eq!(n.meta.labels, vec!["VC", "Product"]);
        assert_eq!(n.meta.source.unwrap().url.as_deref(), Some("https://x.y"));
        assert!(n.meta.created.is_some());
    }

    #[test]
    fn crlf_frontmatter() {
        let text = "---\r\nid: X\r\n---\r\nbody\r\n";
        let n = Note::parse(text);
        assert_eq!(n.meta.id.as_deref(), Some("X"));
        assert_eq!(n.render(), text);
    }

    #[test]
    fn slugs() {
        assert_eq!(
            slugify("Ownership seems to matter more than I thought", 6),
            "ownership-seems-to-matter-more-than"
        );
        assert_eq!(slugify("  ", 6), "note");
        assert_eq!(slugify("The user's café — notes!", 6), "the-users-café-notes");
    }

    #[test]
    fn titles() {
        assert_eq!(derive_title("\n\n## Heading here\nmore"), "Heading here");
        assert_eq!(derive_title("- [ ] buy milk"), "buy milk");
        assert_eq!(derive_title("> quoted"), "quoted");
    }
}
