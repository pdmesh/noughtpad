# Nought — Design Proposal (v0.1, draft for discussion)

> A local-first notebook where you capture anything without friction, and which
> gradually reveals the shape of your own ideas.

Source: *Frictionless Personal Knowledge Notebook — working product thesis (Sep/Nov 2026)*.
This document turns that thesis into a concrete architecture and build plan. Items marked
**[Decision needed]** are open questions for the product owner.

---

## 0. Decisions log

| Date | Decision |
|---|---|
| 2026-11-02 | Desktop only for v1. No mobile app. |
| 2026-11-02 | No migration or import from other tools in v1. Nought starts its own vault. Files stay Obsidian-*readable*. |
| 2026-11-02 | AI tiers: deterministic → local embeddings → small local **non-generative** models (e.g. Laya-class decision/classifier models) → **bring-your-own LLM** (key or endpoint), opt-in, only for advanced tasks. |
| 2026-11-02 | Licence: **FSL-1.1-ALv2** (source-available). Any use except a competing commercial product; each release converts to Apache-2.0 after 2 years. Contributions require a licence grant to the owner (CONTRIBUTING.md) so commercial editions stay possible. ELv2 was rejected because it only restricts hosted services, not desktop resale. |
| 2026-11-02 | English first; language-pluggable architecture (§11). |
| 2026-11-02 | Source metadata entered manually in v1; no web clipper. |
| 2026-11-02 | Distraction-free capture: metadata/chrome hidden, revealed on hover/focus or a toggle. |
| 2026-11-02 | Primary user is the product owner (dogfooding). |
| 2026-11-02 | Desktop shell: **Tauri 2** with a Rust core. |
| 2026-11-02 | Title is optional (`title:` in frontmatter), revealed on hover like labels and source. Without one, the first meaningful line is the display title. Filenames take their slug from the title once, at creation, and are never renamed. |
| 2026-11-02 | No note-type control in the UI: a note with a source is a source note, otherwise personal. A hand-written `type:` in frontmatter is still respected. |
| 2026-11-02 | Note lists (All notes, Collections, Journal) scroll infinitely: keyset pages of 100 and windowed rendering, so the page stays light at any vault size. |
| 2026-11-02 | The Write page shows only the editor. The note stream moves to its own "All notes" page; a palette toggle can put it back under the editor. |
| 2026-11-02 | Extensions reuse the CodeMirror 6 and unified (remark/rehype) ecosystems; no bespoke plugin ecosystem. |

---

## 1. Goals distilled

| # | Goal | Design consequence |
|---|------|--------------------|
| G1 | Capture is instant and blank-page | Editor opens in < 300 ms, focus in the text area, no required fields, global hotkey quick-capture |
| G2 | The user owns the thought | Markdown files are the source of truth; AI never writes into user notes |
| G3 | AI is invisible during capture | Indexing runs in a background worker; no AI UI on the capture surface |
| G4 | Useful with zero model config | Tier 0 (FTS, labels, dates, journal) is a complete product |
| G5 | Time is first-class | Every derived fact carries timestamps; themes have histories |
| G6 | Inferences are tentative, inspectable, reversible | Relations have confidence + rationale + evidence links + user dismiss state |
| G7 | Derived state is disposable | `index.sqlite` can be deleted and rebuilt from the vault |
| G8 | Light on resources | No always-on LLM; incremental, throttled, idle-time indexing |

Non-goals for v1: graph-first UI, Zettelkasten workflow, approval queues, auto-summaries
dashboard, collaboration, custom sync server.

---

## 2. Architecture overview

```
┌───────────────────────── Desktop app (Tauri 2) ──────────────────────────┐
│  UI (React + TypeScript)                                                 │
│   Notebook/Capture · Journal · Collections · Connections · Ideas over    │
│   time · Thinking workspace · Search palette                             │
│                          │  Tauri commands / events                      │
│  ────────────────────────┼───────────────────────────────────────────── │
│  nought-core (Rust library, UI-agnostic)                                 │
│   vault ─ watcher ─ parser ─ indexer ─ search ─ intelligence ─ ai        │
└───────────────┬──────────────────────────────────────┬───────────────────┘
                │                                      │
        Markdown vault (truth)                 .nought/ (app data)
        ~/Notes/**/*.md                         index.sqlite   (disposable)
                                                user-state.db  (NOT disposable:
                                                  dismissals, confirmations)
                                                artifacts/*.md (AI syntheses the
                                                  user chose to keep)
```

Key idea: **three kinds of data, three lifetimes.**

1. **Authored** — the user's `.md` notes. Never touched by AI.
2. **Derived** — FTS, embeddings, concepts, candidate relations, themes. Rebuildable; safe to delete.
3. **User judgement about derived data** — "dismiss this connection", "yes, these are related",
   kept syntheses. Not rebuildable, so it is stored separately from the disposable index and
   keyed by stable note IDs + content hashes, so it survives a reindex.

`nought-core` is a plain Rust crate, so the same engine can later power a CLI, an MCP server
(agent access, as HomeKB/Hypha do), or a mobile build without the desktop UI.

---

## 3. Recommended open-source stack

| Concern | Choice | Why / alternatives | Licence |
|---|---|---|---|
| Desktop shell | **Tauri 2** | Small binary, fast startup, low idle RAM (G1, G8); iOS/Android targets exist for a later capture app. Alt: Electron (all-TypeScript, heavier) | MIT/Apache-2.0 |
| UI | **React + TypeScript + Vite** | Largest ecosystem | MIT |
| UI kit | **shadcn/ui (Radix) + Tailwind** | Accessible primitives, owned code, calm aesthetic | MIT |
| Command palette / quick search | **cmdk** | Standard, fast | MIT |
| Long lists | **TanStack Virtual**, **TanStack Query** | Chronological stream of 10k+ notes | MIT |
| Editor | **CodeMirror 6** + `@codemirror/lang-markdown` + live-preview decorations | Edits the raw Markdown text directly, so files are preserved byte-for-byte (G2). Obsidian uses it. Alt: TipTap/Milkdown (nicer WYSIWYG, but Markdown round-trips are lossy) | MIT |
| Markdown parsing (Rust) | **pulldown-cmark** + **gray_matter** (YAML frontmatter) | Fast, CommonMark/GFM | MIT |
| File watching | **notify** + `notify-debouncer-full` | Picks up edits made outside the app (Obsidian, git, sync) | CC0/MIT/Apache |
| Change detection | **blake3** content hashes | Re-embed only what changed | CC0/Apache |
| Database | **SQLite** via **rusqlite** (`bundled`, FTS5 on) | Single file, zero ops | MIT |
| Full-text search | **SQLite FTS5** (BM25, `porter unicode61` tokenizer) | Tier 0 search, explainable | Public domain |
| Vector search | **sqlite-vec** | Vectors live in the same SQLite file; brute-force KNN is fine at personal scale | MIT/Apache-2.0 |
| Embeddings | **fastembed-rs** (ONNX Runtime) with `bge-small-en-v1.5` or `nomic-embed-text-v1.5`; `multilingual-e5-small` if non-English | In-process, CPU, roughly 30–130 MB models. Check each model's licence | Apache-2.0 (lib) |
| Clustering / themes | **HDBSCAN** (`hdbscan` crate) on embeddings + c-TF-IDF keywords (BERTopic-style, deterministic) | Themes without an LLM; an LLM only *names* them, optionally | MIT/Apache |
| Small local classifier (optional, Tier 2) | Encoder models (ModernBERT-class, e.g. **Laya**, ~400M, Apache-2.0) run via **ort** (ONNX Runtime) | Non-generative, so it can't invent text. Used for yes/no and label decisions (§6.5) | Apache-2.0 / MIT |
| LLM, bring your own (optional, Tier 3) | One `LlmProvider` trait: OpenAI-compatible HTTP (covers Ollama, llama.cpp, LM Studio, OpenRouter) and Anthropic native | Explicit opt-in. A per-request banner shows which notes leave the machine | — |
| Markdown ↔ HTML on paste/copy | **unified**: `rehype-parse` → `rehype-remark` → `remark-stringify` (HTML→MD); `remark-parse` → `remark-gfm`/`remark-math` → `remark-rehype` → `rehype-katex` (MD→HTML) | Clipboard behaves the way it does in Obsidian (§7.1). One ecosystem for paste, copy and export | MIT |
| Diagrams / math | **mermaid**, **KaTeX** | Rendered in the editor's live preview | MIT |
| Background jobs | Tokio task queue inside core, persisted in SQLite (`jobs` table) | No extra service | MIT |
| Migrations | **rusqlite_migration** | Simple | Apache-2.0 |
| Global quick-capture hotkey | `tauri-plugin-global-shortcut` | Capture from anywhere (G1) | MIT/Apache |
| Tests | `cargo test`, **Vitest**, **Playwright** | — | MIT/Apache |

All of these use permissive licences, so they are compatible with FSL distribution. Avoid GPL/AGPL
dependencies; copyleft terms conflict with FSL's competing-use restriction. Model weights get a separate licence check (thesis §19.5).

---

## 4. Vault format

**Proposal: one file per note**, grouped by month, with minimal frontmatter.

```
~/Notes/                         ← user-chosen folder (can be an Obsidian vault)
  2026/09/2026-09-12-2310-ownership-vc.md
  2026/09/2026-09-14-0912-the-mom-test.md
  .nought/                       ← app data (hidden, git-ignorable)
```

```markdown
---
id: 01J8ZK4M7Q3V9X2T6B5N8C1D0E   # ULID: sortable by time, stable across renames
created: 2026-09-12T23:10:00+05:30
labels: [VC, Product]
source:                          # optional; its presence → type: source
  title: The Mom Test
  url: https://example.com
---

Ownership seems to matter more than I initially thought in VC.
Need to understand how this interacts with fund construction.
```

- `type` is **inferred** (source present → `source`, else `personal`) and only written if the user
  overrides it. That keeps frontmatter minimal.
- The filename slug comes from the first line, but the ULID is the identity, so renaming or moving a
  file never breaks derived data.
- Obsidian-compatible: YAML frontmatter, `labels` mirrored as `tags`-style values if desired, and
  `[[wikilinks]]` and `#tags` in the body are parsed as optional extra signals.
- Files without `id` (e.g. an existing Obsidian vault) are indexed using path + created-time;
  an ID is added only if the user opts in. **[Decision needed: Obsidian compatibility, Q3]**

Why not a single daily file? Per-note files give a stable unit for embeddings, relations and
evidence links. A "Daily stream" is a **view** (thesis §6: journal is a view, not a store).

---

## 5. Derived index schema (`.nought/index.sqlite`, disposable)

```sql
notes(id TEXT PK, path, created, modified, type, source_title, source_url,
      title, body, content_hash, word_count)
note_labels(note_id, label)                      -- Gmail-style, many-to-many
notes_fts  USING fts5(title, body, source_title, labels, content='notes')
chunks(id PK, note_id, ord, text, char_start, char_end, hash)
chunk_vec  USING vec0(embedding float[384])      -- sqlite-vec; dim depends on model
concepts(id PK, name, first_seen, last_seen)
note_concepts(note_id, concept_id, weight, extractor)          -- tf-idf | slm
relations(id PK, src_note, dst_note, kind, confidence, rationale,
          evidence_json, method, model, model_version, created, updated)
          -- kind ∈ similar|elaborates|supports|applies_to|conflicts|raises_question|synthesises
themes(id PK, label, keywords_json, method, model_version, computed_at)
theme_members(theme_id, note_id, score)
theme_timeline(theme_id, period, note_count, first_note, notable_notes_json)
jobs(id PK, kind, payload, state, attempts, run_after)
meta(key, value)                                  -- embedding model id, schema version
```

`.nought/user-state.db` (persistent, small):

```sql
relation_feedback(src_note, dst_note, kind, verdict, at)   -- confirmed|dismissed
theme_feedback(theme_key, verdict, renamed_to, at)
kept_artifacts(id, path, kind, created, cites_json)        -- syntheses saved to .nought/artifacts
```

Swapping the embedding model means a column in `meta` changes and the vectors are rebuilt.
Nothing else in the data model depends on the model (thesis §19.4).

---

## 6. Pipelines

### 6.1 Capture → index (Tier 0, synchronous and cheap)
1. The editor autosaves (debounced 500 ms) as an atomic write (temp file + rename).
2. The watcher event updates the `notes` row and `notes_fts` immediately, so search is current within about 1 s.
3. A `embed(note_id)` job is enqueued for later.

### 6.2 Background intelligence (Tier 1+, idle and throttled)
Runs only when the app is idle for N seconds or the machine is on AC power, with one worker thread
and a low thread priority.
- **Chunk** by paragraph/heading (notes are short, so most notes are a single chunk).
- **Embed** changed chunks (hash-diffed).
- **Candidate relations:** kNN per note, then a score combining cosine similarity, shared
  labels/source, shared concepts and temporal proximity, then a threshold of ≤ 5 visible per note.
  Kind defaults to `similar`. An optional SLM classifies it into elaborates/supports/conflicts/etc.
  and adds a one-line rationale.
- **Concepts:** deterministic noun-phrase + c-TF-IDF extraction first; SLM extraction optional.
- **Themes:** periodic (e.g. nightly or on opening "Ideas over time") HDBSCAN over note embeddings.
  Cluster identity is matched across runs by member overlap (Jaccard), so a theme keeps its
  history while its membership changes.

### 6.3 Search (hybrid)
FTS5 BM25 and vector KNN are fused with **Reciprocal Rank Fusion**, then re-ranked with label/source
matches and recency. It degrades to FTS-only when there are no embeddings. Each result shows
*why* it matched: highlighted terms, or "semantically similar to …".

### 6.4 Temporal signals (mostly deterministic)
For each theme or concept: first appearance, monthly counts, recurrence streaks, dormant→revived
(a gap of more than N months, then a new note), external-before-personal ordering (a source note
precedes personal notes in the same theme, which suggests a *possible influence*), and
unresolved questions (notes containing `?` or question phrasing with no later note in the same
theme that elaborates them).

An LLM is used only to *narrate* these signals on demand, and always with citations.

### 6.5 Small non-generative models (Tier 2)
Encoder/decision models such as Laya output a label or a score, never prose. That suits the
"quiet librarian" role: they cannot rewrite or hallucinate content. Candidate tasks:

| Task | Input | Output |
|---|---|---|
| Relation typing | (note A, note B) pair | similar / elaborates / supports / conflicts / raises-question + score |
| Is this an open question? | note | yes/no (feeds "unresolved questions") |
| Stance shift | (earlier note, later note) in the same theme | same / refined / reversed |
| Theme membership check | (theme keywords, note) | belongs / doesn't (stabilises clusters) |
| Should we escalate to the LLM? | task context | yes/no (keeps BYO-LLM calls rare) |

The interface is `trait Classifier { fn classify(&self, inputs, labels) -> Scores }`, which lets
models be swapped. The fallback is zero-shot via embedding similarity to label descriptions, so
Tier 2 is never required. The candidate model gets evaluated on a hand-labelled set of my own
note pairs before it is adopted. Model claims so far come from secondary write-ups.

### 6.6 Thinking workspace (on demand)
Retrieval-augmented: hybrid search over the notebook, then the top-k notes with their dates, then
the LLM, with a prompt that requires `[[note-id]]` citations. Answers are rendered with clickable
citations, and nothing is saved unless the user clicks **Keep**, which writes to
`.nought/artifacts/`, never to the vault. A banner shows which provider (local or remote) will see
which notes before a remote call.

---

## 7. Screens (MVP shape)

### 7.1 Editor: Obsidian-style live preview on CodeMirror 6
- **Live preview:** Markdown renders inline (headings, bold, links, task checkboxes, images,
  tables). The raw syntax reappears only on the line the cursor is on. A source-mode toggle is
  available. The file on disk is exactly what you typed.
- **Paste:** plain text or Markdown is inserted as-is. HTML from a browser or Docs is converted
  to Markdown (unified: rehype-remark + GFM tables). Pasting an image saves it to `attachments/` and inserts
  `![](…)`. Pasting a URL over selected text makes a link.
- **Copy:** puts the Markdown text on the clipboard as `text/plain`, plus rendered HTML as
  `text/html`. Pasting into a Markdown tool gives Markdown; pasting into email or Docs gives
  formatted text.
- **Fenced-block renderers:** ```` ```mermaid ````, `$$math$$` (KaTeX) and syntax-highlighted code
  are rendered as widgets when the cursor is outside them. This is the same idea as Obsidian's
  code-block processors.
- **Plugins: piggyback on existing open ecosystems; no Nought-specific plugin ecosystem.**
  Obsidian's plugins can't run because they depend on Obsidian's closed API. Instead, Nought
  extensions are packages from two large, open, MIT-licensed ecosystems that the app already uses:
  1. **CodeMirror 6 extensions:** editing behaviour such as keymaps, autocomplete, vim mode, decorations and
     linting (e.g. `@replit/codemirror-vim`, `@codemirror/autocomplete`).
  2. **unified plugins (remark/rehype):** Markdown syntax and rendering such as math, callouts/directives,
     wiki-links, footnotes, emoji and diagram renderers (e.g. `remark-wiki-link`, `remark-directive`,
     `rehype-mermaid`). Hundreds of these exist, and they power MDX, Docusaurus, Astro and GitHub-style
     renderers.

  The only Nought-specific piece is a thin registry of **block renderers**
  (`{ languages, render(code, el) }`, see `src/editor/blocks.ts`) for fenced blocks shown in live
  preview, such as mermaid and math. A later step lets a block renderer delegate to a rehype plugin, so
  that preview, copy and export share one rendering path.
  Tiptap/ProseMirror were considered. Their ecosystem is large but built around a rich-text
  document model, which would make Markdown a lossy export format instead of the source of truth.
- Reference implementations to study: SilverBullet (MIT, CM6 live preview) and Zettlr (GPL;
  ideas only, no code reuse).

### 7.2 Screens
- **Write (home):** just the blank page. No list, panel or AI under it, and clicking anywhere on the
  page puts the cursor in the text. **Distraction-free by default:** the sidebar, the
  "source · labels" strip and the toolbar are hidden. They fade in when the pointer is near the
  edges or over the note header, or with a toggle button or `⌘\`. Nothing appears while you type.
  `⌘K` opens search/palette and `⌘N` starts a new note. A global hotkey brings the app forward
  on a new note. Anyone who wants the recent-notes list under the editor can switch it on from the
  palette ("Show note list under the editor"). It is off by default.
- **All notes:** the chronological stream of every note, personal and source, grouped by day.
  Clicking a note opens it on the Write page.
- **Journal:** personal notes by day, week or month. The monthly log is assembled
  deterministically; "Reflect on this month" is an explicit button that calls the AI.
- **Collections:** a label sidebar (like Gmail). Clicking a label filters the stream.
- **Connections** (per note, opened on demand via "Explore this note"): a readable list grouped by
  relation kind, each item with a reason, confidence wording and Dismiss/Confirm.
- **Ideas over time:** a theme list with sparklines, then a theme timeline (the thesis's
  January→September example) with evidence notes, plus the "You keep returning to",
  "Dormant ideas" and "Influences" sections.
- **Think:** a chat-style workspace that is visually separate from the notebook.

---

## 8. Build plan (maps to thesis §20)

| Milestone | Scope | Exit criterion |
|---|---|---|
| **M0 Skeleton** | Tauri + React scaffold, `nought-core` crate, CI (lint/test), vault picker | App opens a folder |
| **M1 Genuinely good notebook** | CM6 editor, autosave, frontmatter, stream view, labels, source field, FTS5 search, watcher, journal (daily/monthly, deterministic), collections, quick-capture hotkey | Dogfood daily; open→typing < 300 ms |
| **M2 Quiet retrieval** | fastembed + sqlite-vec, job queue, hybrid search, Connections panel with reasons, user-state feedback | Rediscovers forgotten notes |
| **M3 Temporal discovery** | Themes (HDBSCAN + c-TF-IDF), theme identity over time, Ideas-over-time view, dormant/recurring/influence signals | Shows something a plain notebook would not |
| **M4 Thinking workspace** | `LlmProvider` (Ollama/llama.cpp/Claude), cited RAG, Explore-this-note actions, kept artifacts, optional SLM relation typing | Trusted thinking partner |

Each milestone ships something that is usable on its own. M1 needs no ML.

**M1 status (2026-11-02): complete.** Measured on a synthetic 10,000-note vault (release build, Linux):

| Measure | Result |
|---|---|
| Process start to focused, editable page | ~1.2–1.4 s, mostly webview start-up. The vault scan runs in the background and never delays capture |
| First full index | ~0.8 s |
| Startup rescan of an unchanged vault | ~40 ms |
| Full-text search | ~20 ms |
| Create a note (write + index) | ~2 ms |

Reproduce with `cargo run --release -p nought-core --example bench -- 10000`. The app logs `editor ready ms=…` on every launch.

---

## 9. Risks I would watch from day one

- **Capture friction creep:** add a performance budget test (time-to-type) to CI.
- **Noisy connections:** keep thresholds conservative and show few links. Track the dismiss rate
  as the main quality metric.
- **Theme instability:** clusters that reshuffle every run destroy "ideas over time". Theme identity
  matching (§6.2) is a core algorithm, not polish. Prototype it early on real notes.
- **Sync:** v1 relies on the user's folder sync (iCloud/Dropbox/Syncthing/git). Because the index is
  local and disposable, every device just reindexes. Conflicts are handled at the file level only.

---

## 10. Open questions

None blocking M1.

---

## 11. Language extensibility (English first)

- Each note gets a `lang` field in the index, detected with **whatlang** (MIT) and never written to
  the file.
- FTS5 tokenizer chosen per language: `porter unicode61` for English, `unicode61
  remove_diacritics 2` for other Latin scripts, `trigram` for CJK. Notes go into per-language FTS
  tables behind one search API.
- The embedding model is recorded in `meta`. Moving to multilingual means switching to
  `multilingual-e5-small` and running a reindex, with no schema change.
- Stopwords and keyword extraction sit behind a `LanguagePack` trait. English ships first.
- UI strings go through i18n keys from day one (`i18next`), even though only English ships.
