# Handoff: next session (M5 on a real Mac)

Read this first next session, then `ROADMAP.md` (what's next) and
`docs/DESIGN.md` (why things are the way they are).

## Where things stand (2026-11-03)

- **M1–M4.5 are done**, and CI is green.
- **Everything has been verified only on Linux:** in a cloud container, with the real Tauri app
  under a virtual display, the real BGE embedding model, the real Qwen3-4B (CPU, about 45–60 s per
  answer) and a fake OpenAI-compatible provider.
- **Never verified:**
  - macOS, including the Keychain, Metal speed and the ⌘⇧Space quick-capture shortcut.
  - A real Anthropic, OpenAI or Ollama provider.
  - A full 2.5 GB model download through the app.
- **M5 so far:**
  - The release workflow (`.github/workflows/release.yml`).
  - The Settings page.
  - The capture-speed check in CI (`pnpm perf`: open → typing about 0.4 s; budget 1.5 s).

## Repo map

| Path | What |
|---|---|
| `crates/nought-core` | Vault, SQLite index, FTS, embeddings, related notes, themes (`semantic/ideas.rs`), chat context scopes (`semantic/context.rs`) |
| `crates/nought-llm` | Model catalogue, checked and resumable downloads, local runtime (mistral.rs), remote providers and the tool loop (genai), keychain, prompts |
| `src-tauri/src` | `lib.rs` (vault commands), `semantic.rs`, `think.rs` (Models page commands), `chat.rs` (chat, tools, consent, `.nought/chats/`) |
| `src/` | React UI. Views: `Chat`, `Models`, `Settings`, `Ideas`, `Journal`. `lib/mockBackend.ts` and `lib/mockLlm.ts` let the UI run in a plain browser |
| `tests/perf/capture.mjs` | Capture-speed budget |
| `tools/eval/` | Model evaluations (embeddings, judges, SLMs) |
| `samples/vault` | 37-note sample notebook |

## Commands

```sh
pnpm install
pnpm tauri dev                     # the app on your real notes folder
pnpm dev:sample                    # the app on a copy of the sample notebook (~/Nought Sample)
pnpm dev                           # UI only, in a browser, with the mock backend (?llm=ready, ?mock=many)
pnpm test && pnpm typecheck        # UI tests
pnpm build && pnpm perf            # capture-speed budget
cargo test --workspace --no-default-features --features nought-app/embedding-static   # fast Rust tests
cargo clippy -p nought-llm --all-targets --features local,remote,keychain -- -D warnings # local runtime
```

## Next session, step by step (on the Mac)

The session needs the Mac. Use Remote Control or computer use so the agent can run commands and see
the screen.

### 1. Get a build
Choose one:
- **From CI:** GitHub → Actions → Release → latest run → artifact `nought-macos-arm64`. The
  first green run with all three platforms is
  [run #2](https://github.com/pdmesh/nought/actions/runs/41607043430): macOS arm64 with Metal
  115 MB, Windows 66 MB, Linux 167 MB. It's
  ad-hoc signed and not notarised. Unzip it, then `xattr -cr Nought.app` (or right-click → Open
  the first time).
- **Local build**, which also gives Metal:
  `pnpm install && pnpm tauri build --features local-llm-metal`. The app lands in
  `target/release/bundle/macos/`.

### 2. Speed checks (record the numbers in DESIGN.md § M5)
- **Startup to typing:** run `RUST_LOG=info /path/Nought.app/Contents/MacOS/nought-app` and read
  the `editor ready ms=…` log line. Target: under 300 ms on Apple Silicon (M1's design goal).
- **Large vault:** `cargo run --release -p nought-core --example bench -- 10000`.
- **Local model with Metal:**
  ```sh
  cargo run --release -p nought-llm --features local-metal --example ask -- \
    ~/Library/Application\ Support/app.nought.desktop/models/llm qwen3-4b-instruct-2507-q4km
  ```
  First download the model in the app: Settings → Language models → Download. Compare with
  about 8 tok/s on a 4-core CPU.
- **In the app:** time the first chat answer after launch (the model is warmed when Chat opens),
  then a follow-up.

### 3. UX pass with computer use
On the sample notebook (`pnpm dev:sample`) and then on real notes:
- **Capture:** ⌘⇧Space from another app, type at once, then check that labels, source and title
  appear on hover. Check that the sidebar and toolbar stay hidden while typing.
- **Editor:** paste Markdown and rich text, copy out, Mermaid, maths, tasks, images (drag and paste).
- **Related notes:** ⌘J, open, dismiss and undo. Check that the reasons make sense.
- **Ideas over time** and theme pages: check that themes and influences match the notebook.
- **Chat:**
  - Ask with each scope.
  - Follow-ups.
  - History panel: open, delete, hide.
  - Navigate away while an answer streams.
  - Click citations.
- **Models:** download (pause and resume), Use, Turn off, add a provider, the consent card.
- **Settings:** every switch and button. Check that preferences survive a restart.
- Dark mode, small windows (minimum 480×360) and keyboard-only use.

### 4. Real providers
Run the same question with each provider:
- An **Anthropic** key, and an **OpenAI** key. Expect search/read steps to appear, and the answer
  not to list sources.
- **Ollama** (`ollama serve`, then pull `qwen3:4b`) and **LM Studio**. Check whether tool calls
  work, and that the fallback (app-chosen notes) kicks in when they don't.

Also check that the key ends up in Keychain Access (service `app.nought.llm`), not in
`~/Library/Application Support/app.nought.desktop/config.json`.

### 5. Name
`Nought.app` is taken. Candidates: Mull, Quire, Strata. Check the App Store, domains and
trademarks (class 9), then rename everywhere **before** daily use. The name is in the bundle ID
`app.nought.desktop`, the keychain service `app.nought.llm`, the `.nought` data folder, crate and
package names, and the docs. Changing it later means migrating data.

### 6. Then
- **Auto-update:** run `pnpm tauri signer generate`, add the public key and updater endpoint, and
  store the private key as `TAURI_SIGNING_PRIVATE_KEY`.
- **Signing**, once you decide to pay: Apple Developer Program, then the secrets listed at the top
  of `release.yml`.
- **Start dogfooding** (ROADMAP M5 exit), then M6 sync design.

## Known limits and gotchas

- **Unsigned builds:** macOS Gatekeeper blocks them until `xattr -cr` or right-click → Open, and
  Windows SmartScreen warns.
- **First model load:** the first load of the 2.5 GB local model reads it from disk, which took
  3.5 minutes on the cloud container's slow disk and seconds when cached. A warm-up starts when
  you open Chat.
- **Local chat** reads at most 6 notes per question. Big scopes need summarising in batches
  (Backlog).
- **⌘L** opens chat; the editor's select-line binding is overridden.
- **Release CI time:** the release build compiles mistral.rs, which takes 20–40 minutes per
  platform on GitHub runners.
- **Old answers:** answers saved with "Keep" before M4.5 remain in `.nought/artifacts/` and
  aren't shown. That's intentional (dropped).
