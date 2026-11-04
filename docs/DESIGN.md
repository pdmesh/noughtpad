# Noughtpad — Design Proposal (v0.1, draft for discussion)

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
| 2026-11-02 | No migration or import from other tools in v1. Noughtpad starts its own vault. Files stay Obsidian-*readable*. |
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
| 2026-11-03 | Before notes go to a remote provider, Noughtpad lists them and the destination and asks; the user can untick notes. It asks the first time and again whenever the destination changes. Local models never ask, because nothing leaves the machine. |
| 2026-11-03 | No embedded third-party agent harness (e.g. pi): our Rust layer already routes models and keeps keys in the keychain. Chat gets a small built-in tool loop instead. Workflow files and cost display are dropped for now. |
| 2026-11-03 | Chat is a full page with the input docked at the bottom of the window; past chats are listed in a right-hand panel on the chat page (like Related; toggled from the top bar) and open as pages. Chats are saved to `.noughtpad/chats/` (this replaces the Think page and "Kept"). Online models search and read notes through read-only tools bounded by the chat's scope; the user approves a chat's scope and destination once. Local models get the most relevant notes per question. Answers don't re-list sources, because the steps already show which notes were read. |
| 2026-11-03 | M5 "ship it to yourself": release workflow (macOS with Metal, Windows, Linux; signing optional), Settings page, capture-speed budget in CI. Kept answers and workflow files are dropped. Plan in `ROADMAP.md`. |
| 2026-11-03 | Auto-update through `tauri-plugin-updater` from GitHub Releases. It only checks when asked (Settings → Updates, or Noughtpad → Check for Updates… on macOS) and installs only when the user presses **Install and restart**; there is no background check and no pop-up. Every update bundle is signed with Noughtpad's updater key and checked against the public key built into the app. Updates go out when a draft release is **published**. |
| 2026-11-03 | Built-in two-way sync (Google Drive everywhere; iCloud on Apple devices only, as iCloud has no API elsewhere) and iOS/Android apps are planned as a paid, proprietary add-on. The core stays FSL, with an open `SyncBackend` interface. |
| 2026-11-03 | Window lifecycle follows each platform. On macOS, closing the window hides it and the app keeps running (Dock icon reopens, ⌘Q quits), so ⌘⇧Space capture is instant instead of waiting ~350 ms for WKWebView. On Windows and Linux, closing quits. "Open at login" (off by default) starts hidden on macOS and minimised elsewhere. |
| 2026-11-03 | Sync design (§12): three-way merge per file against a stored base copy, eager rules (two additions at the same spot are both kept; for title/source the newer edit wins), a conflict copy only when both devices changed the same lines. Edit beats delete. Notes, images and other files, user-state, chats and trash sync; the index, sync state and app settings never do. User-state moves to per-device logs. Google Drive first (`drive.file`, Testing mode while dogfooding); iCloud after the signing decision. Engine and backends live in a private repo (`pdmesh/noughtpad-sync`); the open core has the `SyncBackend` interface. |
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
│  noughtpad-core (Rust library, UI-agnostic)                                 │
│   vault ─ watcher ─ parser ─ indexer ─ search ─ intelligence ─ ai        │
└───────────────┬──────────────────────────────────────┬───────────────────┘
                │                                      │
        Markdown vault (truth)                 .noughtpad/ (app data)
        ~/Notes/**/*.md                         index.sqlite   (disposable)
                                                user-state     (NOT disposable:
                                                  dismissals, confirmations;
                                                  per-device logs from M6, §12.6)
                                                artifacts/*.md (AI syntheses the
                                                  user chose to keep)
```

Key idea: **three kinds of data, three lifetimes.**

1. **Authored** — the user's `.md` notes. Never touched by AI.
2. **Derived** — FTS, embeddings, concepts, candidate relations, themes. Rebuildable; safe to delete.
3. **User judgement about derived data** — "dismiss this connection", "yes, these are related",
   kept syntheses. Not rebuildable, so it is stored separately from the disposable index and
   keyed by stable note IDs + content hashes, so it survives a reindex.

`noughtpad-core` is a plain Rust crate, so the same engine can later power a CLI, an MCP server
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
  .noughtpad/                       ← app data (hidden, git-ignorable)
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

## 5. Derived index schema (`.noughtpad/index.sqlite`, disposable)

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

`.noughtpad/user-state.db` (persistent, small; as built it is `user-state.sqlite`, and for M6 sync
the truth moves to per-device logs replayed into memory, §12.6):

```sql
relation_feedback(src_note, dst_note, kind, verdict, at)   -- confirmed|dismissed
theme_feedback(theme_key, verdict, renamed_to, at)
kept_artifacts(id, path, kind, created, cites_json)        -- syntheses saved to .noughtpad/artifacts
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
- **Storage:** chats are saved as JSON in `.noughtpad/chats/` and listed in the history panel on the right of the chat page. Notes
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
- **Plugins: piggyback on existing open ecosystems; no Noughtpad-specific plugin ecosystem.**
  Obsidian's plugins can't run because they depend on Obsidian's closed API. Instead, Noughtpad
  extensions are packages from two large, open, MIT-licensed ecosystems that the app already uses:
  1. **CodeMirror 6 extensions:** editing behaviour such as keymaps, autocomplete, vim mode, decorations and
     linting (e.g. `@replit/codemirror-vim`, `@codemirror/autocomplete`).
  2. **unified plugins (remark/rehype):** Markdown syntax and rendering such as math, callouts/directives,
     wiki-links, footnotes, emoji and diagram renderers (e.g. `remark-wiki-link`, `remark-directive`,
     `rehype-mermaid`). Hundreds of these exist, and they power MDX, Docusaurus, Astro and GitHub-style
     renderers.

  The only Noughtpad-specific piece is a thin registry of **block renderers**
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
| **M0 Skeleton** | Tauri + React scaffold, `noughtpad-core` crate, CI (lint/test), vault picker | App opens a folder |
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

Reproduce with `cargo run --release -p noughtpad-core --example bench -- 10000`.

**M5 measurements (2026-11-03, Apple Silicon Mac, release build with Metal):**

| Measure | Result |
|---|---|
| Process start to focused editor (`editor ready ms=`), sample notebook | 874 ms cold, 362–394 ms warm before the bundle split; 345–366 ms warm after it. Over the 300 ms goal (G1), see below |
| First full index, 10,000 notes | ~2.9 s |
| Startup rescan of an unchanged vault | ~25 ms |
| Full-text search | ~7 ms |
| Create a note (write + index) | ~5 ms |

Where the warm launch goes (the `setup` and `page load` log lines): ~175 ms until Tauri's setup,
which already includes creating the window and the WKWebView; ~75 ms until the page starts loading;
~110 ms of JavaScript until the editor is focused. A `sample` profile puts most of the first part in
`-[WKWebView initWithFrame:]` (WebKit's GPU process, OpenGL renderer queries, loading system
libraries), which the app can't speed up.

The startup bundle was split anyway: the Markdown↔HTML pipeline, KaTeX and every page but writing
now load after the editor is ready (main chunk 1.6 MB → 0.6 MB). `pnpm perf` dropped from 109 ms to
81 ms; the real app gained ~20 ms. So 300 ms from a cold process isn't reachable on WKWebView. The
budget that matters is quick capture into an app that's already running.

Later the same day (after the window-lifecycle change, the menu bar and the lazy-page fix) warm
launches measured `setup` ~350–370 ms and `editor ready` ~545–600 ms. The extra ~180 ms is all
before Tauri's setup, i.e. window and WKWebView creation. It's the same with the menu bar removed
and with the window created visible, and `pnpm perf` is unchanged (81 ms), so it isn't from those
changes; the cause is unknown. Decision (2026-11-03): under 1 s from a cold process is fine; optimise when there's an easy win. Quick capture into the running app (⌘⇧Space,
then typing at once) lost no keystrokes.

**Local model on Metal (2026-11-03, Apple M4, 16 GB, Qwen3 4B Instruct Q4_K_M):**

| Measure | Result |
|---|---|
| Download through the app, 2.5 GB, quit at 0.8 GB and resumed | ~5 min in all; resumed from 0.8 GB |
| Model load (`examples/ask`) | 19 s from disk, ~5 s when the file is cached |
| Model load in the app (warm-up when Chat opens) | 17 s |
| `examples/ask`: 315-token prompt, 64-token answer | 3.4 s, ~19 tok/s overall including prefill (CPU: ~8 tok/s decode on 4 cores). A longer run is needed for a pure decode rate |
| In the app, first answer after launch (whole notebook, 6 notes) | 23 s from sending, ~11 s of it waiting for the load |
| In the app, follow-up | 6 s |

In the app both answers cited their notes; the `ask` example's answers had no `[n]` markers
(`cited: []`, two runs), so its prompt or settings differ and it's not a citation check.

The model doesn't fit when another app holds GPU memory: with Ollama keeping `qwen3.5:4b` loaded
(7.5 GB, because Ollama gave it a 131k context), mistral.rs saw 1.2 GB free and refused to load.
The app now says so in plain words (`local::load_error`).

**Ollama (`qwen3.5:4b`, same Mac):** the tool loop works: search, then three note reads, then an
answer with citations. First answer 1 min 43 s over five requests, including ~30 s to load the model;
decode 18–22 tok/s, prefill ~208 tok/s. Most of the time is the model's own thinking.

**Keychain:** a provider key is stored under service `app.noughtpad.llm`, account = the provider's
id, and `config.json` holds none. Deleting the provider deletes the item. Reading it back after a
rebuild (ad-hoc signed, so macOS may ask) is untested.

**macOS menu bar:** the app replaces Tauri's default menu (`src-tauri/src/menu.rs`): Settings… ⌘,,
File → New Note ⌘N, View → Show or Hide Sidebar `⌘\`, Search Notes ⌘K, Related Notes ⌘J, Chat ⌘L,
and a Go menu for each page. Each item emits a `menu` event; the UI runs the same command as the
shortcut and ignores the second one when the page and the menu both see a key (checked: `⌘\` and ⌘K
fire once). Edit → Undo/Cut/Copy/Paste are the standard items: paste of Markdown right after launch
renders, and copy puts Markdown text and HTML on the clipboard.

**Auto-update:** the app asks
`https://github.com/pdmesh/noughtpad-releases/releases/latest/download/latest.json` (a public repo
that holds only installers, since this one is private; moved there 2026-11-04, so 0.1.1 installs
can't follow and need one manual install), which GitHub serves
from the newest published (not draft, not pre-release) release. `latest.json` lists, per platform,
the version, the bundle's URL and its signature. The Release workflow makes these only when the
`TAURI_SIGNING_PRIVATE_KEY` secret is set (it passes `--config src-tauri/tauri.updater.conf.json`,
which turns on `createUpdaterArtifacts`), so local builds and forks without the key still build.
Bundles: macOS `Noughtpad.app.tar.gz` (Apple Silicon only, as that's the only Mac build), Windows
the NSIS `.exe` (per user, no admin prompt), Linux the `.AppImage`; each with a `.sig`. The updater
compares versions, so each release needs a higher `version` in the root `Cargo.toml` (the app takes
its version from there). On macOS the install replaces the `.app` in place and restarts it, so the
app must sit somewhere writable (/Applications is, a mounted DMG isn't).

The updater key's public half is in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`); the
private half and its password exist only with the user and in the GitHub secrets. Losing either
means existing installs can't take updates any more and need one manual reinstall with a new key.

What was seen with release v0.1.1 (2026-11-03, published):
- The Release run took 25 min on macOS, 31 min on Linux and 53 min on Windows. The macOS log says
  `using TAURI_SIGNING_PRIVATE_KEY` and `…_PASSWORD`, so the secrets work.
- The Mac update bundle is named `Noughtpad_aarch64.app.tar.gz` (58 MB, with a `.sig`), not
  `Noughtpad.app.tar.gz`; the `.dmg` is `Noughtpad_0.1.1_aarch64.dmg` (57 MB). `latest.json` has
  `darwin-aarch64` (and `-app`), Linux AppImage/deb/rpm and Windows NSIS/MSI entries, each with a
  signature, and their URLs point at the attached files.
- **Blocker: the repo is private.** GitHub only serves release files of a private repo to signed-in
  users with access, and the updater asks anonymously, so
  `releases/latest/download/latest.json` answers 404 and no install can see an update. The endpoint
  is built into each release, so changing where releases live means a new build. Options: make the
  repo public, publish releases from a separate public repo (e.g. `pdmesh/noughtpad-releases`, with
  a token secret that can write to it), or host the files elsewhere (R2/S3). The user decides after
  a few days of using 0.1.1 on their machines. Until then nothing about Gatekeeper on install or
  update has been seen: the expectations in HANDOFF ("Auto-update: parked on where releases live") are unverified.

What was seen with 0.1.2 → 0.1.3 (2026-11-04, the first real update, on the user's Mac):
- 0.1.2 was installed by hand from the `.dmg` (System Settings → Privacy & Security → Open Anyway
  for the unsigned app). Noughtpad → Check for Updates… offered 0.1.3, Install and restart worked,
  and the updated app opened with no Gatekeeper prompt, as expected for a download the app made
  itself (not quarantined).
- The first launch of each new build (the hand install and the update alike) was slow before the
  notes appeared; the second launch was fast (0.4 s to the editor).

**First launch of a new build is slow when the notes are in ~/Documents** (measured 2026-11-04
with the `open step` log): a build that had never run opened `~/Documents/Noughtpad` in 5.9 s
(Opening your notes 4.4 s, Loading your preferences 1.0 s); its second launch took 0.36 s, and
a fresh copy of the release opening `~/Noughtpad Sample` (outside Documents) took 0.4 s on its
first launch. So the time goes on macOS's privacy check for the protected Documents folder,
which treats every ad-hoc signed build as a new app: the first file access waits while macOS
checks the new build. It happens once per installed build (each update), shows the startup
loader rather than a blank window since 0.1.3, and should go away with a stable Developer ID
signature (macOS remembers a signed app across updates) or with notes outside the protected
folders (Documents, Desktop, Downloads, iCloud Drive).

: complete, verified with the real model** in the desktop app on the sample notebook. 33 notes were connected about 3 s after launch. Indexing runs at about 37 notes/s using 2 CPU threads. Related-note lookup takes about 3 ms and hybrid search about 25 ms, including the query embedding. On a hand-written 13-note test set, all 20 related suggestions were on the same topic (still 20 of 20 at `min_related` 0.63 in M5).
- Background worker: notes are embedded only after 3 s without edits, newest first, in batches of 16, yielding between batches.
- Vectors are cached by `(model, chunk-text hash)`, so index rebuilds, renames and unchanged saves never re-run the model.
- Related notes score each note against all others with `0.5·mean-vector similarity + 0.5·best passage-pair similarity`. A note is kept only above the model's `min_related` and at least 1σ above that note's own similarity distribution, which filters "everything looks alike" notes. Small boosts apply for shared labels and the same source.
- Each related note shows the matching passage, the passage it matched in your note, shared labels, same source and time apart. Wording is cautious: "These ideas appear connected" / "Possibly related".
- Dismissals live in `.noughtpad/user-state/` (per-device logs since M6 slice 1, §12.6). They are not disposable and apply in both directions.
- Hybrid search fuses FTS5 and vector ranks (reciprocal rank fusion, k = 60). Meaning-only hits are marked "similar meaning", and label/type filters apply to them too.
- The CI `model` job downloads the real model and checks that the thresholds separate related from unrelated sentences. Measured: related pairs 0.66–0.88, unrelated pairs up to 0.57, so `min_related` was 0.60; raised to 0.63 in M5 after the Mac UX pass, where short notes on different topics (a run log next to a bread note) scored 0.60–0.63 on the sample notebook and no same-topic suggestion fell below 0.63 (tuned on those same 37 notes, so check it on real notes). This favours precision: two short sourdough notes with no shared vocabulary are missed rather than letting false links in. `examples/semantic.rs` reproduces the quality and speed check. The app logs `editor ready ms=…` on every launch.

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
- Reproduce with `cargo run --release -p noughtpad-core --features embedding-static --example ideas -- <model-dir> samples/vault 2026-11-02`.

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
- **Storage:** both are stored in the user-state (`.noughtpad/user-state/`, §12.6) with the pair's features at that moment.
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
  Built-in sync (M6) merges edits instead; design in §12.

---

## 10. Open questions

None blocking M1. Sync's open points are in §12.12.

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

---

## 12. Sync (M6, design, 2026-11-03)

Built-in two-way sync between your devices. Design only; no code yet. Decisions taken with the
product owner are in the decisions log (§0); what is still open is at the end of this section.

### 12.1 The idea in one paragraph

Each device keeps, for every synced file, a copy of **the version both sides last agreed on** (the
*base*). When it syncs, it compares three versions of each file: the base, the one on this device
and the one in the cloud. If only one side changed, that side wins. If both changed, the two edits
are merged line by line against the base (a *three-way merge*, the same idea git uses). Only when
both devices changed the very same lines is a **conflict copy** written next to the note. Deleted
files leave a small **tombstone** record, so a device that was offline for weeks learns about the
delete instead of uploading the file again. The search index is never synced; every device
rebuilds its own from the notes it has.

### 12.2 What syncs and what doesn't

| Data | Syncs? | How |
|---|---|---|
| Notes (`**/*.md`) | Yes | Three-way merge (§12.5) |
| Images, PDFs and any other file in the notebook | Yes | Whole file. If both devices changed it, a conflict copy |
| Your judgements (dismissed related notes, feedback) | Yes | Per-device logs that never conflict (§12.6) |
| Chat history (`.noughtpad/chats/`) | Yes | One file per chat; merged by message (§12.6) |
| Trash (`.noughtpad/trash/`) | Yes | Files are written once with unique names, so they never conflict |
| Tombstones (`.noughtpad/tombstones/`) | Yes | Per-device logs of deletes (§12.7) |
| Index (`.noughtpad/index.sqlite*`) | **Never** | Derived. Each device builds its own |
| Sync state (`.noughtpad/sync/`) | **Never** | It describes this device's view |
| App settings, provider keys, downloaded models | **Never** | They live outside the notebook folder (app config dir, OS keychain) and are per device |
| Temporary files (`.noughtpad-tmp-*`, `*.tmp`), `.DS_Store`, old `artifacts/` | Never | |

Chat consent travels with the chat, but it names a provider that is set up per device; on a
device where that provider isn't set up, the chat asks again before notes go anywhere.

### 12.3 Pieces, and where the code lives

```
noughtpad-core (open, FSL)                 noughtpad-sync (private repo pdmesh/noughtpad-sync)
  device id                                   engine: plan → merge → apply → upload → commit
  per-device user-state logs                  three-way merge for Markdown, chats, user-state
  Vault::apply_synced(...) (safe writes)      Google Drive backend (first)
  trait SyncBackend + its types               iCloud backend (later)
  a folder backend used by tests              licence check (later)
```

- The app builds the private crate only with a `sync` cargo feature. The open build, CI and
  every command in HANDOFF work without it. Release builds need a read-only token for that repo.
- The open core gets what any sync, including someone else's, needs: stable device ids,
  conflict-free user-state, a safe "write this file from sync" call, and the interface.

### 12.4 Local sync state (per device, never synced)

`.noughtpad/sync/` (added to `.noughtpad/.gitignore`):

```
sync/
  state.sqlite     -- one row per synced path:
                   --   path, note_id, base_hash, remote_rev, remote_hash, synced_at
                   -- plus: device_id, backend, remote notebook id, the backend's change cursor
  base/<blake3>    -- the base text of each file, stored by its hash (needed to merge;
                   -- a hash alone can't be merged against). Images keep no base copy: their
                   -- base is the hash only, since they are never merged.
  journal          -- table in state.sqlite: paths this device changed since the last sync
```

- **Change journal.** Every write through the vault and every watcher event adds the path to the
  journal, so a sync only looks at what changed. The journal is a shortcut, not the truth: on
  start, and every so often, a full comparison of each file's hash with its base catches anything
  the journal missed (the app was closed while another editor or git touched files). Losing the
  journal costs time, never data.
- **Device id.** A ULID kept in the app's data dir (not the notebook), plus a name the user sees
  ("MacBook Air"). `state.sqlite` records the device id it belongs to; if they don't match (the
  folder was copied to another machine), the sync state is thrown away and the device joins
  afresh (§12.8).
- On the remote side, the backend's own change feed plays the journal's role (Drive's
  `changes.list` page token).

### 12.5 Merging notes

A note is frontmatter plus body; they are merged separately.

**Body: line-by-line three-way merge.** Written in our crate on top of `similar`'s line diff
(Apache-2.0) rather than a ready-made diff3, so we own the rules:
1. Lines changed on one side only: take that side.
2. The same change on both sides: take it once.
3. **Both sides only added text at the same spot** (the common case: two devices each appended a
   paragraph): keep both, this device's first, with a blank line between so the paragraphs don't
   run together.
4. Both sides changed the same lines differently: **the merge fails** (§12.5.1).

Line endings and a missing final newline are kept as they are; a file is never reformatted.

**Frontmatter: field by field.** Parsed with the existing note parser into an ordered map:
- `labels`: merged as sets. A label added on either side is kept, one removed on either side is
  removed.
- Any other field (`title`, `source`, `type`, hand-written keys): if one side changed it, that
  side wins; if both changed it differently, **the newer edit wins** (by file modification time,
  device id breaks ties). It is noted in the sync log, not turned into a conflict copy.
- `id` never merges. Different ids at the same path means two different notes landed on the same
  path (rare: same minute, same slug); the incoming one is saved under `…-2.md`. The same id at
  two paths means one device moved the file; the move is followed.

#### 12.5.1 When a merge fails: conflict copies

- The note keeps **this device's** version, untouched.
- The other device's whole version is saved next to it as
  `2026-09-12-2310-ownership-vc (conflict 2026-11-03 1405).md`, with a **new id** (two files
  must never share one) and `conflict_of: <original id>` in its frontmatter. (The device's name
  isn't in it yet: the remote doesn't say which device wrote a version. Drive can carry it in
  `appProperties` in slice 3.) It is an ordinary
  note: search finds it, and you delete it once you've copied over what you need.
- The app shows a quiet notice ("1 note had edits on two devices") linking to both.
- The copy syncs like any other note, so every device sees the same pair.

Why not a CRDT (Automerge, Yjs)? The Markdown files are the truth and other tools edit them
(Obsidian, git, a text editor), so the merge has to work on plain text after the fact. A CRDT
would need its own history next to every file and still fall back to text merging for outside
edits.

### 12.6 Data that must never conflict

- **User-state becomes per-device logs.** `.noughtpad/user-state.sqlite` is one file that every
  device writes, and a SQLite file can't be merged (and copying one mid-write can corrupt it). So
  the truth moves to append-only logs, one per device:
  `.noughtpad/user-state/<device-id>.jsonl`, each line an event
  (`{"at":…, "op":"dismiss", "a":…, "b":…}`, `undismiss`, `feedback`, …). A device only ever
  writes its own log, so sync never sees two writers. An in-memory database is the cache,
  rebuilt from all the logs (newest event per key wins). This works with no sync too, and with
  folder sync tools such as Dropbox.
- **Chats** are one JSON file each. If both devices changed one chat and one message list is a
  prefix of the other, the longer wins. Otherwise (both continued the same chat offline) the
  other device's copy is saved as a separate chat, "(from MacBook Air)".
- **Trash** files are named by time and path when written, so two devices never write the same
  trash file.

### 12.7 Deletes and tombstones

- Deleting a note on this device moves it to `.noughtpad/trash/` as today, and records a
  tombstone in this device's log `.noughtpad/tombstones/<device-id>.jsonl` (synced; one writer per
  log, like user-state): `{path, id, base_hash, at}`.
- Another device that still has the file at that base hash moves it to **its** trash. A sync
  never deletes anything outright; everything stays recoverable in the trash.
- **Edit beats delete.** If the other device edited the note since the base, the note is kept
  with the edit, uploaded again, and the user gets a notice ("A note you deleted on MacBook Air
  was edited on Windows PC, so it's back").
- Tombstones are small, so they are kept for good. That way even a device that hasn't synced for
  a year, or one that lost its sync state, can tell "deleted elsewhere" from "never uploaded".

### 12.8 One sync, step by step

1. **Pull:** ask the backend for remote changes since the saved cursor.
2. **Plan:** for each path changed here (journal) or there (feed), compare local hash, base hash
   and remote hash: upload, download, merge, delete-here, delete-there, or nothing.
3. **Merge and write locally** through `Vault::apply_synced(path, bytes, expected_hash)`, which
   writes atomically and refuses if the file changed since the plan (the user was typing); that
   path is retried next round.
4. **Upload** each changed file with `put(path, bytes, expected_rev)`. If the remote moved on in
   the meantime, the backend says so and the path goes back to step 2.
5. **Commit:** only after the upload is confirmed, store the new base and revision.

A crash at any step is safe: the base isn't updated until step 5, so the next run sees the same
three versions and does the same merge again. Merging a file with an identical result is a no-op
(rule 2), so repeating a step never duplicates text.

**Joining** (first sync on a device, or after its state was reset): there is no base, so every
file is compared directly. Same hash on both sides: becomes the base. Only on one side: copied
over, unless a tombstone says it was deleted. Different on both: merged against an empty base,
which keeps both versions' unique lines, or gives a conflict copy.

**When it runs:** on start, about 5 s after you stop typing, when the window comes to the front,
and every minute while the app is open (Drive can't push changes to a desktop app). Never during
capture: sync runs in the background and writes to an open note only when the editor has nothing
unsaved, and the editor then reloads the note keeping the cursor.

**The editor:** a merged write to the open note arrives like any outside edit today (the watcher
plus the editor's `base_hash` check), so no keystroke is lost.

### 12.9 The `SyncBackend` interface (open, in `noughtpad-core`)

Blocking calls, like the rest of the core; the engine runs them on a background thread.

As built in slice 1 (`crates/noughtpad-core/src/sync/mod.rs`):

```rust
pub trait SyncBackend: Send + Sync {
    fn kind(&self) -> &'static str;                                  // "gdrive", "icloud", "folder"
    fn open_notebook(&self, name: &str) -> Result<RemoteNotebook>;   // find or create
    fn list(&self, nb: &RemoteNotebook) -> Result<Vec<RemoteEntry>>; // every file
    /// Changes since `cursor`; `None` = from now (no entries, just a cursor).
    fn changes(&self, nb: &RemoteNotebook, cursor: Option<&Cursor>) -> Result<ChangePage>;
    fn get(&self, nb: &RemoteNotebook, path: &str) -> Result<(Vec<u8>, Rev)>;
    /// Write only if the remote is still at `expected` (None = must not exist yet).
    fn put(&self, nb: &RemoteNotebook, path: &str, bytes: &[u8], modified: i64, expected: Option<&Rev>)
        -> Result<Put>;
    /// Remove (to the backend's own trash where it has one), same rule.
    fn remove(&self, nb: &RemoteNotebook, path: &str, expected: &Rev) -> Result<Put>;
}

pub struct RemoteEntry { pub path: String, pub rev: Rev, pub hash: Option<String>,
                         pub size: u64, pub modified: i64, pub removed: bool }
pub struct ChangePage  { pub entries: Vec<RemoteEntry>, pub cursor: Cursor, pub more: bool }
pub enum   Put         { Written(Rev), Moved(Option<Rev>) /* someone else got there first */ }
pub struct Rev(pub String);      // opaque to the engine
pub struct Cursor(pub String);   // opaque to the engine
```

The interface is deliberately file-shaped, so a backend is small: the "only if still at
`expected`" rule is the one hard part, and each backend meets it in its own way.

### 12.10 Backends

**Google Drive (first).** Works on every platform.
- **Access:** only files Noughtpad created (`drive.file`). Google rates that scope low-risk: it
  needs basic checks of the Google Cloud project (app name, logo, privacy policy and home page
  links, a verified domain), not the paid security audit that full Drive access needs. Google
  reviews the project's details, not our code. **Starts in Testing mode** for dogfooding: only
  listed test accounts can sign in, and sign-ins expire every 7 days. Before anyone else uses it,
  fill in those details and switch the project to "In production".
- **Sign-in:** OAuth with PKCE through the system browser and a loopback address
  (`http://127.0.0.1:<port>`), the standard desktop flow. The refresh token goes in the OS
  keychain like provider keys. The client id ships in the app (desktop client ids are not
  secret).
- **Layout:** a visible folder, `My Drive/Noughtpad/<notebook>/`, mirroring the notebook's paths,
  so your notes are readable in Drive itself. Each file carries the notebook-relative path and
  note id in `appProperties`.
- **Changes:** `changes.list` with a saved page token; `md5Checksum` and `headRevisionId` per
  file.
- **"Only if still at `expected`":** Drive has no conditional write, so `put` uploads a new
  revision and then reads the file's revision list. If the revision before ours isn't `expected`,
  another device slipped in between. Drive kept both revisions, so the engine fetches theirs,
  merges and uploads again. Nothing is lost, it just takes one more round.
- **Warning:** if the notebook folder is already inside Google Drive for desktop, Dropbox or
  iCloud Drive (`~/Library/CloudStorage/…`, `~/Library/Mobile Documents/…`), the app says so
  before turning sync on, since two syncs on one folder fight each other.

**iCloud (later, Apple devices only).** Waits for the Apple Developer Program decision: the
iCloud permission only works in a signed app with a provisioning profile. Two ways, chosen when
we get there: the app's iCloud Drive container with `NSFileCoordinator` (iCloud moves the files;
the backend reads its conflict versions), or CloudKit records (we control every write, closer to
the Drive design). A notebook uses one backend; to share a notebook with a non-Apple device it
has to be on Google Drive.

**Folder (tests only, for now).** A backend over a plain directory, used by the engine's tests
and for trying sync between two notebooks on one machine. Could become a product feature (sync
over a USB drive or a NAS) later.

### 12.11 Testing

- Merge rules as unit tests, plus property tests: random edit sequences on two simulated devices
  must never lose a line that either device wrote, and repeating a sync must change nothing.
- The engine against the folder backend with two or three simulated devices: offline edits,
  delete vs edit, crash between steps (kill after each step, then rerun), a copied folder.
- Drive against a real test account, by hand, before dogfooding.
- Exit (ROADMAP M6): two Macs and one non-Apple device on one notebook for a week, no lost edits.

### 12.12 Smaller points (settled 2026-11-04, the recommended defaults)

- **Notebook name on Drive:** the remote notebook is matched by its id in `notebook.json`, not its
  name. If a different notebook already has the name, the app asks once and adds a suffix
  ("Notes 2").
- **Big files:** files over 50 MB don't sync; the sync status lists them so nothing is skipped
  silently.
- **Sync status in the UI:** a quiet dot in the top bar and a Settings → Sync section. A sync log
  viewer only if it turns out to be needed.
- **Licensing and billing** (ROADMAP M6) come after the engine works.

### 12.13 First slice (done 2026-11-04)

All in the open core, in this repo, testable without Google:
1. **Device id** in the app data dir, with a name the user can change.
2. **User-state as per-device logs** (`.noughtpad/user-state/<device-id>.jsonl`), with
   `user-state.sqlite` rebuilt from them. (The old rows were first copied over on first start;
   dropped in 0.1.3, since there were no users.) Useful already for anyone syncing the folder with Dropbox or Syncthing.
3. **`SyncBackend` trait and types** in `noughtpad-core::sync`, plus the **folder backend** and
   its tests (including the "only if still at `expected`" rule).
4. **`Vault::apply_synced`** (atomic, refuses if the file changed) and the **change journal**
   hooks, with tests; `sync/` added to `.noughtpad/.gitignore`.

As built: `device.rs` (`device.json` in the app data folder, named from the computer's name);
`userstate.rs` (logs replayed into an in-memory database; `reload()` after sync brings in other
devices' logs; a torn last line is skipped; an old `user-state.sqlite` is ignored);
`sync/` (`is_synced`, the trait, `FolderBackend` with a lock file and a `changes.jsonl` feed,
`Journal` in `.noughtpad/sync/journal.sqlite`); `Vault::{synced_files, read_synced,
apply_synced, remove_synced}`, with a write lock shared with the editor's saves;
`watch_journaled` (a folder event records the files inside it; a path that's gone may have been a
folder, so the engine treats it as a prefix). The app uses the device id for user-state; the
journal isn't switched on until the engine arrives.

### 12.14 Second slice (done 2026-11-04, private repo `pdmesh/noughtpad-sync`)

The engine and the merge rules, tested against the folder backend:
- `merge/text.rs`: the line merge (§12.5) on `similar`'s Myers diff. Rule 3 also keeps a block
  that already holds the other side's addition as is, so re-merging after a crash never
  duplicates text. Joining without a base uses the lines both sides share as the base.
- `merge/note.rs`: frontmatter field by field (labels as sets, other fields newest-wins by file
  time, hand-written keys too, through `Note::extra`/`set_extra` added to the core), body by
  lines; different ids at one path count as a failed merge. `merge/mod.rs`: chats and logs.
- `state.rs`: `.noughtpad/sync/state.sqlite` (files, pending remote changes, meta) and
  `base/<hash>` for notes and chats; unused bases are removed after each sync.
- `engine.rs`: §12.8 as written. Remote changes are first stored as pending, so the cursor can
  move on at once and nothing pulled is lost if the sync stops. Up to 5 rounds per sync pick up
  what a round leaves (a remote that moved on mid-upload, this device's new tombstones).
  Tombstones are only consulted when joining. Files over 50 MB are listed, not synced.
- Tests: 18 engine tests with two or three simulated devices (new, edit, both appended, labels,
  same line → copy, delete → trash, edit beats delete both ways, joining, a copied folder
  rejoining, lost sync state, images, chats, dismissals, a dropped connection mid-sync), and a
  random-edit test: three devices, random edits, deletes and syncs; afterwards all devices hold
  the same files, no written line is lost, and another sync does nothing. 300 seeds passed
  (`NOUGHTPAD_SYNC_SEEDS=300`; 2,075 merges, 575 conflict copies, 472 edit-beats-delete).

Not yet: the app doesn't run the engine (it comes in with slice 3, behind a `sync` cargo
feature), and the private repo has no CI (it would need a token to check out this repo).

### 12.15 Third slice (code done 2026-11-04; not yet tried against real Drive)

- **Drive backend** (`noughtpad-sync/src/drive/`): §12.10 as written. Sign-in is PKCE through
  the system browser and a one-time listener on `127.0.0.1:<random port>` (desktop OAuth clients
  register no redirect address). The refresh token and email are kept in the keychain under
  `sync:google-drive`. An expired sign-in (7 days in Testing mode) shows as "signed out". Paths
  come from parent folders, not from `appProperties`, which can't hold long paths; `np_nb` tags
  every file and folder, and `np_hash` carries the vault hash. Downloads fetch the exact head
  revision.
- **The client:** id `1065938497859-norka….apps.googleusercontent.com` (project
  `noughtpad-app`) is in the code. The secret is compiled in from
  `NOUGHTPAD_GOOGLE_CLIENT_SECRET` (a GitHub secret for releases, an environment variable
  locally). It ships inside every copy; Google treats desktop client secrets as not
  confidential, and PKCE protects the sign-in.
- **`Service`** (`noughtpad-sync/src/service.rs`): what the app calls (sign in/out, start/stop,
  nudge, sync now, status). A background thread syncs on start, ~5 s after changes stop, on
  window focus and every minute.
- **Linking without the private repo:** the app depends on an open stand-in,
  `crates/noughtpad-sync-stub` (same `Service` shape, "not in this build"; kept out of the
  workspace members, since Cargo can't patch a member). `pnpm sync <command>`
  (`scripts/sync-build.mjs`) writes a git-ignored `.cargo/config.toml` that patches the stub to
  `../noughtpad-sync` (or `$NOUGHTPAD_SYNC_DIR`), runs the command and restores `Cargo.lock`. CI
  and open builds never need the private repo. The Release workflow does the same when the
  `RELEASE_TOKEN` secret exists.
- **App:** `src-tauri/src/sync.rs` (commands, keychain adapter, `UserState::reload` and a list
  refresh after a sync that changed things, focus nudges); `start_watching` in `lib.rs` turns on
  the change journal while sync is on. UI: Settings → Sync (sign in, "Sync this notebook" to
  `Noughtpad/<folder name>` in Drive, Sync now, notices for conflict copies, restores and files
  over 50 MB), and a dot in the top bar (accent while syncing, red when it needs you). The mock
  backend shows it with `?sync=on`.
- **Not checked yet:** a real sign-in and sync against Drive (needs the Mac and the secret).
