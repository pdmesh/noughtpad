# Handoff: next session (M5 on a real Mac)

Read this first next session, then `ROADMAP.md` (what's next) and
`docs/DESIGN.md` (why things are the way they are).

## Where things stand (2026-11-03)

- **M1–M4.5 are done**, and CI is green.
- **Everything has been verified only on Linux:** in a cloud container, with the real Tauri app
  under a virtual display, the real BGE embedding model, the real Qwen3-4B (CPU, about 45–60 s per
  answer) and a fake OpenAI-compatible provider.
- **Verified on a Mac (2026-11-03):** a local `--features local-llm-metal` build, startup time
  (345–366 ms warm, mostly WKWebView creation), the 10k-note benchmark, and the window lifecycle:
  closing hides the window, ⌘⇧Space and the Dock icon bring it back, ⌘Q quits, and `--hidden`
  starts with no window.
- **Never verified:**
  - On macOS: the Keychain, Metal model speed, and the "Open at login" switch actually adding the
    login item (`~/Library/LaunchAgents`).
  - A real Anthropic, OpenAI or Ollama provider.
  - A full 2.5 GB model download through the app.
- **M5 so far:** the release workflow, the Settings page, the capture-speed check in CI
  (`pnpm perf`), the rename, the Mac build and speed numbers, the startup bundle split, and the
  platform window lifecycle with "Open at login".

## Repo map

| Path | What |
|---|---|
| `crates/noughtpad-core` | Vault, SQLite index, FTS, embeddings, related notes, themes (`semantic/ideas.rs`), chat context scopes (`semantic/context.rs`) |
| `crates/noughtpad-llm` | Model catalogue, checked and resumable downloads, local runtime (mistral.rs), remote providers and the tool loop (genai), keychain, prompts |
| `src-tauri/src` | `lib.rs` (vault commands), `lifecycle.rs` (window hide/reopen, open at login), `semantic.rs`, `think.rs` (Models page commands), `chat.rs` (chat, tools, consent, `.noughtpad/chats/`) |
| `src/` | React UI. Views: `Chat`, `Models`, `Settings`, `Ideas`, `Journal`. `lib/mockBackend.ts` and `lib/mockLlm.ts` let the UI run in a plain browser |
| `tests/perf/capture.mjs` | Capture-speed budget |
| `tools/eval/` | Model evaluations (embeddings, judges, SLMs) |
| `samples/vault` | 37-note sample notebook |

## Commands

```sh
pnpm install
pnpm tauri dev                     # the app on your real notes folder
pnpm dev:sample                    # the app on a copy of the sample notebook (~/Noughtpad Sample)
pnpm dev                           # UI only, in a browser, with the mock backend (?llm=ready, ?mock=many)
pnpm test && pnpm typecheck        # UI tests
pnpm build && pnpm perf            # capture-speed budget
cargo test --workspace --no-default-features --features noughtpad-app/embedding-static   # fast Rust tests
cargo clippy -p noughtpad-llm --all-targets --features local,remote,keychain -- -D warnings # local runtime
```

## Next session, step by step (on the Mac)

Run on this Mac with computer use (background `app_*` tools work for the app window; global
shortcuts need full-screen control). The local build is the quickest:
`pnpm install && pnpm tauri build --features local-llm-metal --bundles app`, which lands in
`target/release/bundle/macos/Noughtpad.app` (about 7 minutes from clean, about 1 minute after).
Launch it on the sample notebook with
`open --env NOUGHTPAD_VAULT="$HOME/Noughtpad Sample" target/release/bundle/macos/Noughtpad.app`
(`node scripts/sample-vault.mjs` creates the folder). Rust must be 1.94 or newer for mistral.rs.

### 1. Open Mac checks
- **Open at login:** the user turns it on in Settings → Start-up (the agent's auto mode blocks
  adding login items). Expect a plist in `~/Library/LaunchAgents`; log out and in, and the app
  should be running with no window. Turning it off removes the plist.
- **Local model with Metal:** download in Settings → Language models (2.5 GB; also covers the
  ROADMAP item "full download, resume after quitting midway"), then
  ```sh
  cargo run --release -p noughtpad-llm --features local-metal --example ask -- \
    ~/Library/Application\ Support/app.noughtpad.desktop/models/llm qwen3-4b-instruct-2507-q4km
  ```
  Compare with about 8 tok/s on a 4-core CPU. In the app, time the first chat answer after launch
  and a follow-up. Record the numbers in DESIGN.md § M5 measurements.

### 2. UX pass with computer use
On the sample notebook and then on real notes:
- **Capture:** ⌘⇧Space from another app, type at once, then check that labels, source and title
  appear on hover. Check that the sidebar and toolbar stay hidden while typing.
- **Editor:** paste Markdown and rich text, copy out, Mermaid, maths, tasks, images (drag and
  paste). Paste/copy and inline maths now load lazily after startup; check them right after launch.
- **Related notes:** ⌘J, open, dismiss and undo. Check that the reasons make sense.
- **Ideas over time** and theme pages: check that themes and influences match the notebook.
- **Chat:** each scope, follow-ups, history panel (open, delete, hide), navigate away while an
  answer streams, citations.
- **Models:** download (pause and resume), Use, Turn off, add a provider, the consent card.
- **Settings:** every switch and button. Check that preferences survive a restart.
- Dark mode, small windows (minimum 480×360) and keyboard-only use.

### 3. Real providers (needs the user's keys)
Run the same question with each provider:
- An **Anthropic** key, and an **OpenAI** key. Expect search/read steps to appear, and the answer
  not to list sources.
- **Ollama** (`ollama serve`, then pull `qwen3:4b`) and **LM Studio**. Check whether tool calls
  work, and that the fallback (app-chosen notes) kicks in when they don't.

Also check that the key ends up in Keychain Access (service `app.noughtpad.llm`), not in
`~/Library/Application Support/app.noughtpad.desktop/config.json`.

### 4. Then
- **Auto-update:** run `pnpm tauri signer generate`, add the public key and updater endpoint, and
  store the private key as `TAURI_SIGNING_PRIVATE_KEY`.
- **Signing**, once the user decides to pay: Apple Developer Program, then the secrets listed at
  the top of `release.yml`.
- **Start dogfooding** (ROADMAP M5 exit), then M6 sync design.

### Done on 2026-11-03
- Renamed Nought → Noughtpad everywhere (no data migration needed).
- Mac build with Metal; speed numbers in DESIGN.md § M5 measurements. Startup is ~350 ms warm,
  mostly WKWebView creation, so the 300 ms goal is met by keeping the app running instead.
- Startup bundle split (1.6 MB → 0.6 MB main chunk).
- Window lifecycle per platform (`src-tauri/src/lifecycle.rs`) and the "Open at login" switch.

## Known limits and gotchas

- **Unsigned builds:** macOS Gatekeeper blocks them until `xattr -cr` or right-click → Open, and
  Windows SmartScreen warns.
- **First model load:** the first load of the 2.5 GB local model reads it from disk, which took
  3.5 minutes on the cloud container's slow disk and seconds when cached. A warm-up starts when
  you open Chat.
- **Local chat** reads at most 6 notes per question. Big scopes need summarising in batches
  (Backlog).
- **Release run #2** predates the rename, so its macOS artifact is named `nought-macos-arm64`.
- **⌘L** opens chat; the editor's select-line binding is overridden.
- **Release CI time:** the release build compiles mistral.rs, which takes 20–40 minutes per
  platform on GitHub runners.
- **Old answers:** answers saved with "Keep" before M4.5 remain in `.noughtpad/artifacts/` and
  aren't shown. That's intentional (dropped).
