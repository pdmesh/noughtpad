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
| 2026-11-02 | M2 semantic features are opt-in (one click, ~65 MB model download, local only). Related notes open on demand (⌘J or the top-bar button), never during capture. |
| 2026-11-02 | M3 is deterministic over local embeddings; AI-written monthly or topic syntheses move to M4 (bring-your-own LLM). |
| 2026-11-02 | Journal shows a GitHub-style activity map with streaks. It is informational only (no goals, reminders or badges), so it never turns capture into a chore. Its levels adapt to the writer's own quartiles. The colour ramp is one hue (the amber accent), validated as an ordinal ramp against both surfaces (light end 2.12:1 on the light surface, 2.56:1 on dark). |
| 2026-11-03 | Small judge models (reranker, NLI, Laya) evaluated against embeddings + rules, did not beat them, and were removed (details under M3.5). One default download (the embedding model). |
| 2026-11-03 | M4 language models are **always explicit opt-in**, on a Models page: download a local model (several may be downloaded) or add your own provider and key, then press **Use**. Downloading or saving a provider does not switch anything on. |
| 2026-11-03 | Built-in runtime: **mistral.rs** (Rust, MIT) running GGUF on CPU/Metal/CUDA. Default model **Qwen3-4B-Instruct-2507 Q4_K_M** (Apache-2.0, 2.5 GB). Bring-your-own via the **genai** crate (Anthropic, OpenAI, Gemini, Ollama, any OpenAI-compatible URL); keys live in the OS keychain. |
| 2026-11-03 | Before notes go to a remote provider, Nought lists them and the destination and asks; the user can untick notes. It asks the first time and again whenever the destination changes. Local models never ask, because nothing leaves the machine. |
| 2026-11-03 | No embedded third-party agent harness (e.g. pi): our Rust layer already routes models and keeps keys in the keychain. Chat gets a small built-in tool loop instead. Workflow files and cost display are dropped for now. |
| 2026-11-03 | Chat is a full page with the input docked at the bottom of the window; past chats are listed in a right-hand panel on the chat page (like Related; toggled from the top bar) and open as pages. Chats are saved to `.nought/chats/` (this replaces the Think page and "Kept"). Online models search and read notes through read-only tools bounded by the chat's scope; the user approves a chat's scope and destination once. Local models get the most relevant notes per question. Answers don't re-list sources, because the steps already show which notes were read. |
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
| UI kit | Small hand-written components on shared CSS tokens (`src/styles.css`); **cmdk** for anything with keyboard-driven lists (⌘K palette, label combobox); **lucide** icons. Never native widgets that can't be themed (e.g. `<datalist>`) | The surface is tiny and deliberately quiet. Reconsider shadcn/ui (Radix + Tailwind) when settings or dialog-heavy screens arrive | MIT |
| Command palette / quick search | **cmdk** | Standard, fast | MIT |
| Long lists | **TanStack Virtual**, **TanStack Query** | Chronological stream of 10k+ notes | MIT |
| Editor | **CodeMirror 6** + `@codemirror/lang-markdown` + live-preview decorations | Edits the raw Markdown text directly, so files are preserved byte-for-byte (G2). Obsidian uses it. Alt: TipTap/Milkdown (nicer WYSIWYG, but Markdown round-trips are lossy) | MIT |
| Markdown parsing (Rust) | **pulldown-cmark** + **gray_matter** (YAML frontmatter) | Fast, CommonMark/GFM | MIT |
| File watching | **notify** + `notify-debouncer-full` | Picks up edits made outside the app (Obsidian, git, sync) | CC0/MIT/Apache |
| Change detection | **blake3** content hashes | Re-embed only what changed | CC0/Apache |
| Database | **SQLite** via **rusqlite** (`bundled`, FTS5 on) | Single file, zero ops | MIT |
| Full-text search | **SQLite FTS5** (BM25, `porter unicode61` tokenizer) | Tier 0 search, explainable | Public domain |
| Vector search | Exact scan over an **in-memory flat index** (vectors persisted as BLOBs in `index.sqlite`, cached by chunk-text hash) | At personal scale (≤ ~100k chunks) an exact scan takes milliseconds. sqlite-vec also scans exhaustively, and it is pre-1.0 and a native extension, so it was dropped | — |
| Embeddings | **fastembed-rs** (ONNX Runtime, statically linked) with **BGE-small-en-v1.5, quantised** (`Qdrant/bge-small-en-v1.5-onnx-Q`, 384-d, ~65 MB, MIT). Downloaded on opt-in into the app data folder, never the vault. Models sit behind an `Embedder` trait with a per-model `ModelSpec` (thresholds, query/passage prefixes), so `multilingual-e5-small` can be swapped in later | In-process, CPU, capped at half the cores | Apache-2.0 (lib), MIT (model) |
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

### 6.6 Chat with your notes (on demand)
A chat is a page (⌘L, the sidebar's Chat item, "Ask about this note", or a theme page). It has a **scope**: the whole notebook, the current note and its
related notes, a theme, a month, open questions or a collection.
- **Online models** get four read-only tools, all bounded by the scope: `search_notes`, `read_note`,
  `notes_in_month` and `open_questions`. They take up to 6 rounds.
- **Local models**, and providers that reject tools, instead get the 6 most relevant notes for each
  question, chosen by the app.
- **Notes are numbered** the first time the model sees them and keep that number for the whole chat.
  Every search and read appears as a step that expands to the notes involved.
- **Answers** respond to the question directly and cite `[n]` only where pointing at a note helps.
- **Earlier turns:** the last 6 are sent as text.
- **Storage:** chats are saved as JSON in `.nought/chats/` and listed in the history panel on the right of the chat page. Notes
  are never written.
- **Consent:** before an online provider sees anything, the user approves this chat's scope and
  destination once per chat.

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

Reproduce with `cargo run --release -p nought-core --example bench -- 10000`.

**M2 status (2026-11-02): complete, verified with the real model** in the desktop app on the sample notebook. 33 notes were connected about 3 s after launch. Indexing runs at about 37 notes/s using 2 CPU threads. Related-note lookup takes about 3 ms and hybrid search about 25 ms, including the query embedding. On a hand-written 13-note test set, all 20 related suggestions were on the same topic.
- Background worker: notes are embedded only after 3 s without edits, newest first, in batches of 16, yielding between batches.
- Vectors are cached by `(model, chunk-text hash)`, so index rebuilds, renames and unchanged saves never re-run the model.
- Related notes score each note against all others with `0.5·mean-vector similarity + 0.5·best passage-pair similarity`. A note is kept only above the model's `min_related` and at least 1σ above that note's own similarity distribution, which filters "everything looks alike" notes. Small boosts apply for shared labels and the same source.
- Each related note shows the matching passage, the passage it matched in your note, shared labels, same source and time apart. Wording is cautious: "These ideas appear connected" / "Possibly related".
- Dismissals live in `.nought/user-state.sqlite`. They are not disposable and apply in both directions.
- Hybrid search fuses FTS5 and vector ranks (reciprocal rank fusion, k = 60). Meaning-only hits are marked "similar meaning", and label/type filters apply to them too.
- The CI `model` job downloads the real model and checks that the thresholds separate related from unrelated sentences. Measured: related pairs 0.66–0.88, unrelated pairs up to 0.57, so `min_related` is 0.60. This favours precision: two short sourdough notes with no shared vocabulary are missed rather than letting false links in. `examples/semantic.rs` reproduces the quality and speed check. The app logs `editor ready ms=…` on every launch.

---

**M3 status (2026-11-02): complete, verified with the real model on the sample notebook.**
- **Themes** use average-linkage merging over the mutual 6-nearest-neighbour graph of *mean-centred* note vectors, with a cut-off of 0.10.
  - Without centring, BGE rates generic notes as similar to everything (e.g. *The Mom Test* scored ≥ 0.60 against 9 of 12 VC notes), and single-link grouping chained unrelated topics.
  - Measured on the sample notebook: genuine members scored 0.13–0.48 centred; intruders −0.04–0.0.
- **Labels** steer merging. Groups whose main labels differ need +0.10 to merge; groups sharing a main label need 0.03 less. User metadata outranks embeddings.
- **Theme size:** at least 3 notes on at least 2 different days. Themes are only formed with at least 8 embedded notes.
- **Names** use class-based TF-IDF (unigrams and bigrams, English stopwords). If no word appears in two of the theme's notes, the label is used, then the latest note's first line.
- **Identity:** a recomputed group reuses an earlier theme id when member overlap (Jaccard) is at least 0.3.
- **Sections:**
  - *returning*: 3 or more active months;
  - *forming*: first note within the last 45 days;
  - *dormant*: no note for 90 days or more.
- **Influences:** within a theme, an earlier *source* note related (≥ `min_related`) to a later personal note.
- **Revisits:** an earlier *personal* note at least 7 days older. "May rethink" is shown when the later note uses contrast words ("but", "tension", "not sure"…).
- **Open questions:** questions with no later note at `strong_related` or above.
- **Recompute:** the worker recomputes when idle and the vectors changed, at most once a minute. Computing 37 notes takes about 1 ms; the full pairwise pass is O(n²), so it will need an approximate index well beyond about 20k notes.
- **UI:**
  - an *Ideas over time* page with 12-month bar charts (muted bars, current month in the accent, a tooltip per bar, an accessible summary);
  - a theme timeline with cues worded tentatively ("may build on", "returns to", "may rethink");
  - a "Part of a thread" link in the Related panel.
- Reproduce with `cargo run --release -p nought-core --features embedding-static --example ideas -- <model-dir> samples/vault 2026-11-02`.

**M3.5 status (2026-11-03): small "judge" models evaluated and removed.** Three kinds of small non-generative model were measured on the sample notebook against the current approach (embeddings + rules). None beat it.
- **Cross-encoder reranker** (`jina-reranker-v1-turbo-en`, int8, 38 MB):
  - **Search:** "getting fit" and "tools for thinking" returned nothing (embeddings found the right notes), "baking bread" went from 3 results to 1, and latency rose from about 15 ms to 30–125 ms.
  - **Related notes:** as a filter it removed genuine links (Tokyo ↔ Kyoto ryokan, sourdough ↔ third loaf) that embeddings had right.
- **Natural-language inference model** (`nli-deberta-v3-xsmall`, int8, 87 MB): it gave false contradictions on free-form notes, even when checked in both directions and on the closest sentence pairs. "Outlier companies return the fund" vs "one company returns the fund, ownership decides how much" scored 0.89.
- **Laya**, a "System 1" decision model (`convaiinnovations/laya`, ModernBERT-large, 421M, Apache-2.0):
  - **Test set:** 16 hand-labelled note pairs, two input formats, the English and multilingual checkpoints (`tools/eval/laya_eval.py`).
  - **Same topic:** best AUC 0.83 (English, plain text) against **0.95 for embeddings**. Most same-topic pairs scored below 0.5, including "Ownership may matter more…" vs "Ownership seems to matter more…".
  - **Pushback:** best AUC 0.72 against **0.75 for the word rule**. "Tension with my March note" scored 0.08–0.26.
  - **Speed:** 150–830 ms per pair on CPU, against under 1 ms for embeddings.
  - Its card says fine-tuning on one's own decisions is where it shines (0.36 → 0.77 on its benchmark). That needs a labelled dataset of note-pair judgements we don't have yet.
- **Decision:** the judge interface, adapters, extra ONNX Runtime/tokenizer dependencies and relation labels were removed to keep the codebase lean. The evaluation scripts stay in `tools/eval/`, and the code is in git history (`9d06f49`) if a better model appears. Non-LLM intelligence that *did* pay off: embeddings, deterministic temporal rules, and learning from the user's own feedback.

**Feedback learning (2026-11-03): built.** Related notes learn what *this* user considers related, with no new model and no approval queue.
- **Signals:** opening a suggestion from the Related panel counts as useful; "Not related" counts as not useful.
- **Storage:** both are stored in `.nought/user-state.sqlite` with the pair's features at that moment.
- **Features:** note similarity, best-passage similarity, shared labels, same source, time apart, both personal, and labels that don't overlap.
- **Model:** a class-balanced logistic regression, regularised towards a prior that *is* today's hand-tuned rule.
  - It switches on only after 3 or more opens and 3 or more dismissals.
  - Until then, behaviour is unchanged.
  - When active, candidates come from a slightly wider net (`min_related − 0.08`), and the model's probability (≥ 0.5) decides what is shown and in what order.
- **Visibility and control:** the panel says "Tuned to what you open and dismiss here". `⌘K → Reset what related notes learned` clears it; dismissals stay hidden.
- **Tests:** unit tests cover the prior and the learning. An integration test checks that feedback on other notes stops cross-topic look-alikes from appearing for a new note while same-topic suggestions remain.

**M4 status (2026-11-03): built and verified with a real local model.**

- **Model choice.** `tools/eval/slm_eval.py` asked each candidate the same 4 questions over the sample notebook on CPU (4 cores, no GPU, Q4_K_M; mistral.rs 0.9.5):

  | Model | Valid citations | Faithfulness | Output speed |
  |---|---|---|---|
  | **Qwen3-4B-Instruct-2507** | 4 / 4 answers, all claims cited | One mild stretch | 7.7 tok/s |
  | Qwen3.5-4B | 3 / 4 | Invented a causal link | 4.6 tok/s |
  | Qwen3-1.7B | 4 / 4, but lumped citations | Echoed the question, slipped into first person | 16 tok/s |
  | SmolLM3-3B | 0 / 4 | Padded | 10 tok/s |
  | Phi-4-mini | — | Its GGUF tokenizer doesn't load in mistral.rs | — |

  Qwen3-4B-Instruct is the recommended default. Qwen3-1.7B is offered for older machines and Qwen3.5-4B for GPUs.
- **Through the app's own runtime**, Qwen3-4B answered a 3-note question with correct `[1][2][3]` citations. It took about 15 s to load and 25 s to answer, on CPU.
- **Downloads** are pinned to a Hugging Face revision, checked against SHA-256, resumable (HTTP Range) and cancellable. A file that fails the checksum is deleted.
- **Privacy rules are enforced in the backend.** `think_ask` refuses a remote provider without consent for a new or changed destination, and reads only the note ids the user was shown.
- **Known limits:**
  - Local models read at most 6 notes per question, so there is no map-reduce over long scopes yet.
  - Without the system keychain (for example, Linux without Secret Service), keys are kept only until quit, and the form says so.

**Concept extraction (2026-11-03): evaluated, not shipped.** Scripts are in `tools/eval/`.
- **GLiNER** (`gliner_small-v2.1`, int8, 183 MB, Apache-2.0, about 20 ms per note):
  - It is excellent at *names*: places (Tokyo, Asakusa, Fushimi Inari), authors and book titles.
  - It is poor at *concepts*. At 0.5 it misses "power law", "fund outcome", "sourdough" and "half marathon", labels "Ownership" an organisation and "Zettelkasten" a book, and counts "someone"/"Nobody"/"I" as people.
  - At thresholds low enough to catch "Portfolio construction" it adds junk ("compare", "quote", "Idea").
  - Separately, `gline-rs` is built on ort rc.9 and cannot share a process with fastembed's rc.15. Shipping it would mean porting its pre- and post-processing.
- **Embedding key phrases** (KeyBERT-style with the existing BGE model, no download): about half the phrases are clean ("sourdough starter", "half marathon", "portfolio construction", "concentration beats diversification") and the rest are fragments ("law the job", "gion visit fushimi").
  - Too noisy to show per note.
  - Possibly useful later in aggregate (phrases that recur across notes) for a concept index or theme names.
- **Decision:** neither ships. Revisit concept timelines after M4, ideally with recurrence-based aggregation.

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
