# Nought

A local-first notebook: capture anything on a blank page, and let it gradually reveal the shape of your own ideas.

Notes are plain Markdown files you own. AI stays out of the way during capture and works quietly in the background. It shows up through derived views (Journal, Connections, Ideas over time) or when you ask.

- Design: [docs/DESIGN.md](docs/DESIGN.md)
- Status: **M1 (a genuinely good notebook) complete**. Next: M2, quiet retrieval with local embeddings

## What works today

- Blank-page capture with an Obsidian-style live-preview Markdown editor (CodeMirror 6)
  - headings, emphasis, links, task checkboxes, quotes, code, images rendered inline. Syntax shows only on the line you're editing
  - ```` ```mermaid ```` diagrams, `$$ … $$` / ```` ```math ```` blocks and inline `$…$` maths (KaTeX) render in place
  - paste rich text (web, Docs, Word) → Markdown. Copy → Markdown + rich HTML. URL over selection → link. Pasted images saved to `attachments/`. `⌘/Ctrl+Shift+V` pastes plain text
- Autosave to plain `.md` files (`YYYY/MM/YYYY-MM-DD-HHMM-slug.md`) with minimal frontmatter. Files are written atomically and never rewritten behind your back
- Optional labels (Gmail-style, many per note) and source (title, URL), hidden until you hover
- A Write page that is only the editor. Separate pages for All notes (chronological, every note), Collections (by label) and the Journal (personal notes by day, week or month). The note list can be shown under the editor from the `⌘K` palette
- Full-text search (SQLite FTS5) with `#label`, `type:personal|source`, `"phrases"`, prefix matching: `⌘/Ctrl+K`
- Opening a vault never blocks capture: catching up with changes runs in the background (10k notes: under 1 s first time, ~40 ms after)
- Edits from other apps (Obsidian, git, sync) are picked up live. The index in `.nought/` is disposable and rebuildable
- Distraction-free by default: toolbar and sidebar appear on hover. `⌘/Ctrl+\` pins them
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
