# Nought — Design Proposal (v0.1, draft for discussion)

> A local-first notebook where you capture anything without friction, and which
> gradually reveals the shape of your own ideas.

Source: *Frictionless Personal Knowledge Notebook — working product thesis (Sep/Nov 2026)*.
This document turns that thesis into a concrete architecture and build plan. Items marked
**[Decision needed]** are open questions for the product owner.

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
| Local LLM (optional) | **Ollama** or **llama.cpp server** via its OpenAI-compatible HTTP API | Not bundled; detected if present. Keeps the app small (G8) | MIT |
| Remote LLM (optional) | Anthropic Claude / OpenAI-compatible, via one `LlmProvider` trait | Explicit opt-in, per-request transparency | — |
| Background jobs | Tokio task queue inside core, persisted in SQLite (`jobs` table) | No extra service | MIT |
| Migrations | **rusqlite_migration** | Simple | Apache-2.0 |
| Global quick-capture hotkey | `tauri-plugin-global-shortcut` | Capture from anywhere (G1) | MIT/Apache |
| Tests | `cargo test`, **Vitest**, **Playwright** | — | MIT/Apache |

All of these are permissive licences, so they are compatible with either a closed or an
open-source distribution. Model weights get a separate licence check (thesis §19.5).

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

### 6.5 Thinking workspace (on demand)
Retrieval-augmented: hybrid search over the notebook, then the top-k notes with their dates, then
the LLM, with a prompt that requires `[[note-id]]` citations. Answers are rendered with clickable
citations, and nothing is saved unless the user clicks **Keep**, which writes to
`.nought/artifacts/`, never to the vault. A banner shows which provider (local or remote) will see
which notes before a remote call.

---

## 7. Screens (MVP shape)

- **Notebook (home):** a large blank editor at the top (cursor already there). Below it is the
  chronological stream grouped by day. A small, collapsed "＋ source · labels" affordance sits under
  the editor. `⌘K` opens search/palette and `⌘N` starts a new note. A global hotkey opens a
  floating capture window.
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

See the accompanying discussion. These are the ones that change the build:
1. Desktop shell & language comfort (Tauri/Rust vs Electron/all-TypeScript).
2. Mobile capture in v1?
3. Obsidian: open existing vaults at launch, or import later?
4. Default AI posture and preferred remote provider.
5. Languages of notes (affects embedding model).
6. Licence / open-source intent for this repo.
7. Expected scale (hundreds vs tens of thousands of notes) and first user (dogfooding by you?).
8. Browser clipping in v1, or manual source metadata only?
