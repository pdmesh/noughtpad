# Handoff: next session (M5 on a real Mac)

Read this first next session, then `ROADMAP.md` (what's next) and
`docs/DESIGN.md` (why things are the way they are).

## Where things stand (2026-11-03, evening)

- **M1–M4.5 are done**, and CI is green.
- **Verified on this Mac (Apple M4, 16 GB):** a `--features local-llm-metal` build; startup and the
  10k-note benchmark; the window lifecycle (close hides, ⌘⇧Space and the Dock bring it back, ⌘Q
  quits); quick capture with no lost keystrokes; "Open at login" adding and removing
  `~/Library/LaunchAgents/Noughtpad.plist`; the 2.5 GB Qwen3 4B download with quit-and-resume;
  Metal speed (first in-app answer 23 s, follow-up 6 s); Ollama with tool use; the provider key in
  Keychain (`app.noughtpad.llm`) and not in `config.json`; paste/copy/undo right after launch.
  Numbers are in DESIGN.md § M5 measurements.
- **Never verified:** a real Anthropic, OpenAI or LM Studio provider (the user chose Ollama only for
  now), the fallback when a model can't use tools, logging out and in with "Open at login" on, and
  reading a saved key back after a rebuild.
- **M5 so far:** everything above, plus a macOS menu bar, the first-visit blank-page fix, the
  related-notes threshold at 0.63 and a plain out-of-memory message for the local model.

## Repo map

| Path | What |
|---|---|
| `crates/noughtpad-core` | Vault, SQLite index, FTS, embeddings, related notes, themes (`semantic/ideas.rs`), chat context scopes (`semantic/context.rs`) |
| `crates/noughtpad-llm` | Model catalogue, checked and resumable downloads, local runtime (mistral.rs), remote providers and the tool loop (genai), keychain, prompts |
| `src-tauri/src` | `lib.rs` (vault commands), `lifecycle.rs` (window hide/reopen, open at login), `menu.rs` (macOS menu bar), `semantic.rs`, `think.rs` (Models page commands), `chat.rs` (chat, tools, consent, `.noughtpad/chats/`) |
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

The local build is the quickest: `pnpm tauri build --features local-llm-metal --bundles app`
(about 1–2 minutes after the first build) lands in `target/release/bundle/macos/Noughtpad.app`.
Launch it on the sample notebook with
`open --env NOUGHTPAD_VAULT="$HOME/Noughtpad Sample" target/release/bundle/macos/Noughtpad.app`, or
run `Contents/MacOS/noughtpad-app` directly with that variable to get the log (`setup`,
`editor ready`, model load times). Every app command is in the menu bar now, so background
computer use can drive it with `app_menu` (Go → Language Models, View → Chat…); paste, global
shortcuts and resizing need full-screen control.

State on this Mac: `~/Noughtpad Sample` has 2 test notes and 2 chats from the UX pass
(`node scripts/sample-vault.mjs` recreates it). The app config has the local Qwen3 4B active and an
Ollama provider (`qwen3.5:4b`) saved. "Open at login" is off.

### 1. Finish the UX pass
- Dark mode, the minimum window size (480×360), keyboard-only use.
- Related notes: dismiss and undo; check the reasons. Chat: history delete and hide, leaving a chat
  while it streams. Editor: images (drag and paste) and Mermaid.
- Then the user's real notes (`~/Documents/Noughtpad`): does 0.63 still keep related notes on topic?
- Recheck startup after a reboot: warm launches measured ~550 ms to `editor ready` late on
  2026-11-03, against ~350 ms earlier that day, and none of the day's changes explain it (see
  DESIGN.md).

### 2. Providers
- When the user has keys: Anthropic and OpenAI. The user pastes the key into Models → Add a
  provider (the computer-use tool can't type into password fields anyway). Expect search/read steps
  and an answer that doesn't list sources.
- LM Studio, if installed, and the tool fallback (a model that rejects tools should get app-chosen
  notes).
- Read a key back after a rebuild: the build is ad-hoc signed, so macOS may ask for Keychain access.

### 3. Then
- **Auto-update:** run `pnpm tauri signer generate`, add the public key and updater endpoint, and
  store the private key as `TAURI_SIGNING_PRIVATE_KEY`.
- **Signing**, once the user decides to pay: Apple Developer Program, then the secrets listed at
  the top of `release.yml`.
- **Start dogfooding** (ROADMAP M5 exit): install the app in /Applications, turn "Open at login" on
  there, then M6 sync design.

### Done on 2026-11-03
- Morning: renamed Nought → Noughtpad; Mac build with Metal and speed numbers; startup bundle split
  (1.6 MB → 0.6 MB); window lifecycle per platform and the "Open at login" switch.
- Evening: Mac checks and a UX pass on the sample notebook. Added a macOS menu bar
  (`src-tauri/src/menu.rs`) with Settings ⌘, and the app's commands; fixed a blank page on the
  first visit to each page (`page()` in `src/App.tsx`); raised `min_related` 0.60 → 0.63 after
  unrelated short notes showed up as related; a plain message when the local model doesn't fit in
  free memory.

## Known limits and gotchas

- **GPU memory is shared with Ollama/LM Studio:** with Ollama holding a model (7.5 GB for
  `qwen3.5:4b`, since Ollama gives it a 131k context), the local 4B model can't load on a 16 GB Mac.
  The app says so plainly. Sending a smaller `num_ctx` to Ollama would help (Backlog candidate).
- **Editor lists:** Enter after a task adds `- [ ] ` automatically, so typing `- [x] ` by hand gives
  `- [ ] - [x] …`; Enter on an empty item clears it without a blank line, so the next paragraph
  joins the list in Markdown.
- **Deleting a provider** (trash icon) removes it and its key at once, with no confirm or undo.
- **`examples/ask`** answers without `[n]` citations, unlike the app; don't use it to check citing.
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
