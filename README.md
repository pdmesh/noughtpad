# Nought

A local-first notebook: capture anything on a blank page, and let it gradually reveal the shape of your own ideas.

Notes are plain Markdown files you own. AI stays out of the way during capture and works quietly in the background. It shows up through derived views (Journal, Connections, Ideas over time) or when you ask.

- Design: [docs/DESIGN.md](docs/DESIGN.md)
- Status: **M1 complete; M2 (quiet retrieval) implemented**. Next: M3, temporal discovery

## What works today

- Blank-page capture with an Obsidian-style live-preview Markdown editor (CodeMirror 6)
  - headings, emphasis, links, task checkboxes, quotes, code, images rendered inline. Syntax shows only on the line you're editing
  - ```` ```mermaid ```` diagrams, `$$ … $$` / ```` ```math ```` blocks and inline `$…$` maths (KaTeX) render in place
  - paste rich text (web, Docs, Word) → Markdown. Copy → Markdown + rich HTML. URL over selection → link. Pasted images saved to `attachments/`. `⌘/Ctrl+Shift+V` pastes plain text
- Autosave to plain `.md` files (`YYYY/MM/YYYY-MM-DD-HHMM-slug.md`) with minimal frontmatter. Files are written atomically and never rewritten behind your back
- Optional title, labels (Gmail-style, many per note) and source (title, URL), all hidden until you hover. Whether a note is personal or source-based is inferred from whether it has a source
- A Write page that is only the editor. Separate pages for All notes (chronological, every note), Collections (by label) and the Journal (personal notes by day, week or month). The note list can be shown under the editor from the `⌘K` palette. Lists scroll infinitely and render only what's on screen
- Full-text search (SQLite FTS5) with `#label`, `type:personal|source`, `"phrases"`, prefix matching: `⌘/Ctrl+K`
- Opening a vault never blocks capture: catching up with changes runs in the background (10k notes: under 1 s first time, ~40 ms after)
- Edits from other apps (Obsidian, git, sync) are picked up live. The index in `.nought/` is disposable and rebuildable
- Distraction-free by default: toolbar and sidebar appear on hover. `⌘/Ctrl+\` pins them
- **Related notes** (`⌘/Ctrl+J`, or the hover button on a note): notes that appear connected, each with the passage that matched, shared labels, same source and how far apart in time. "Not related" hides a suggestion for good, with undo
- **Search by meaning**: once turned on, `⌘K` finds notes by meaning as well as words. Opt-in: a ~35 MB local model is downloaded once, and notes never leave your computer
- Global quick capture: `⌘⇧Space` (macOS) / `Ctrl+Shift+Space`

## Development

Prerequisites: Rust (stable), Node 22, pnpm, and the [Tauri system dependencies](https://v2.tauri.app/start/prerequisites/).

```sh
pnpm install
pnpm tauri dev        # run the desktop app
pnpm dev              # UI only, in a browser, with an in-memory mock backend
cargo test --workspace
pnpm test && pnpm typecheck
```

Local embeddings use ONNX Runtime, statically linked by default; the build downloads it from `cdn.pyke.io`. Offline, build with `--no-default-features --features embedding-dynamic` and point `ORT_DYLIB_PATH` at a `libonnxruntime` (e.g. from the `onnxruntime` PyPI wheel). Real-model test: `cargo test -p nought-core --features embedding-static -- --ignored model_`.

Layout:

| Path | What |
|---|---|
| `crates/nought-core` | UI-agnostic engine: note format, vault, SQLite/FTS5 index, file watcher |
| `src-tauri` | Desktop shell: exposes the core as Tauri commands and events |
| `src` | React + TypeScript UI. `src/editor` holds the CodeMirror live preview, clipboard and block renderers |

## License

[Functional Source License 1.1, Apache-2.0 future license](LICENSE.md) (`FSL-1.1-ALv2`).

You may use, modify and redistribute Nought for any purpose **except** a Competing Use, meaning offering it, or something substantially similar built from it, as a commercial product or service. Each release becomes available under Apache-2.0 two years after it is published.

Contributions are accepted under the terms in [CONTRIBUTING.md](CONTRIBUTING.md).
