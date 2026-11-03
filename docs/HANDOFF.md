# Handoff: next session (M5 on a real Mac)

Read this first next session, then `ROADMAP.md` (what's next) and
`docs/DESIGN.md` (why things are the way they are).

## Where things stand (2026-11-03, late)

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
  related-notes threshold at 0.63, a plain out-of-memory message for the local model, and
  **auto-update** (code, key and workflow done; the two-release check on the Mac is step 1 below).
- **Apple signing is deferred:** no Developer Program for now. The user runs unsigned builds on
  their own Macs with right-click → Open for a few weeks, then decides.

## Repo map

| Path | What |
|---|---|
| `crates/noughtpad-core` | Vault, SQLite index, FTS, embeddings, related notes, themes (`semantic/ideas.rs`), chat context scopes (`semantic/context.rs`) |
| `crates/noughtpad-llm` | Model catalogue, checked and resumable downloads, local runtime (mistral.rs), remote providers and the tool loop (genai), keychain, prompts |
| `src-tauri/src` | `lib.rs` (vault commands), `lifecycle.rs` (window hide/reopen, open at login), `menu.rs` (macOS menu bar), `updater.rs` (check, install, restart), `semantic.rs`, `think.rs` (Models page commands), `chat.rs` (chat, tools, consent, `.noughtpad/chats/`) |
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

### 1. Finish auto-update: the two-release check (next)

Done already: `tauri-plugin-updater` (`src-tauri/src/updater.rs`), the public key and the endpoint
in `src-tauri/tauri.conf.json`, Settings → Updates (`src/lib/updates.ts`), Noughtpad → Check for
Updates… in the menu, and `release.yml` making signed update bundles and `latest.json` when the
updater secret exists. How it works: `docs/DESIGN.md` § Auto-update.

Before the check, the user must have stored the two GitHub secrets
(`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). The private key was generated
on 2026-11-03 and handed to the user; it is not in the repo (`*.key` is git-ignored). Check with
`gh secret list --repo pdmesh/noughtpad`.

1. **Release A (0.1.1):** set `version = "0.1.1"` in the root `Cargo.toml` (`[workspace.package]`)
   and `package.json`, commit, `git tag v0.1.1 && git push origin main v0.1.1`. Wait for Actions →
   Release (20–40 min). On the draft release check that `latest.json`,
   `Noughtpad.app.tar.gz` and `Noughtpad.app.tar.gz.sig` are attached, then press **Publish**.
2. **Install A like a user:** download the `.dmg` from the release in Safari, drag Noughtpad to
   /Applications, right-click → Open (note exactly what Gatekeeper shows). Settings → Updates should
   say 0.1.1 and "You have the newest version."
3. **Release B (0.1.2):** bump to 0.1.2 the same way, tag `v0.1.2`, wait, publish.
4. **Update:** in the running 0.1.1, Noughtpad → Check for Updates… → Settings shows "Version
   0.1.2 is available" → **Install and restart**. Expect: download progress, the app restarts as
   0.1.2.
5. **Write down** in DESIGN.md § Auto-update what Gatekeeper did at step 2 and after step 4, how long
   the download took, and whether macOS asked for the keychain (only if a provider key is saved).
   Then drop "Left: …" from the ROADMAP item.

What Gatekeeper should do (expected, not yet seen on a Mac): the first install is quarantined
because Safari downloaded it, so it needs right-click → Open once (or
`xattr -cr /Applications/Noughtpad.app`). Updates are downloaded by the app itself, which doesn't
mark files as quarantined, so the updated app should open with no prompt. Each build has a new
ad-hoc signature, so macOS may treat it as a different app for anything tied to the signature:
the keychain may ask "Noughtpad wants to use … app.noughtpad.llm" (choose Always Allow), and
privacy permissions, if any are ever granted, may be asked again. If an update ever does trigger
"Noughtpad is damaged / can't be opened", `xattr -cr /Applications/Noughtpad.app` fixes it, and
that is worth recording.

If something fails: run `/Applications/Noughtpad.app/Contents/MacOS/noughtpad-app` from Terminal
to see the log (`update check`, `update: …` warnings). The updater refuses anything whose
signature doesn't match the public key, so a wrong secret shows as "The download didn't match
Noughtpad's signature".

### 2. Then
- **Start dogfooding** (ROADMAP M5 exit): install in /Applications from a release (not a local
  build, so updates work), turn "Open at login" on there. Ship fixes as tagged releases and update
  through the app.
- **Signing**, deferred: the user runs unsigned builds with right-click → Open for a few weeks.
  If they decide to pay: Apple Developer Program, then the secrets listed at the top of
  `release.yml`. Updates keep working either way; the updater key is separate from Apple's.
- M6 sync design.

### Parked (do when convenient, not blocking)
- UX pass leftovers: dark mode, minimum window size, keyboard-only use, related-notes dismiss/undo,
  chat history delete/hide, leaving a chat while it streams, images and Mermaid, the user's real
  notes (and whether `min_related` 0.63 holds there).
- Providers: Anthropic, OpenAI, LM Studio, the no-tools fallback, reading a key back after a rebuild.
- Startup: under 1 s is fine. Optimise when there's an easy win; the ~550 ms seen late on
  2026-11-03 (vs ~350 ms earlier) needs no chase.

### Done on 2026-11-03
- Morning: renamed Nought → Noughtpad; Mac build with Metal and speed numbers; startup bundle split
  (1.6 MB → 0.6 MB); window lifecycle per platform and the "Open at login" switch.
- Evening: Mac checks and a UX pass on the sample notebook. Added a macOS menu bar
  (`src-tauri/src/menu.rs`) with Settings ⌘, and the app's commands; fixed a blank page on the
  first visit to each page (`page()` in `src/App.tsx`); raised `min_related` 0.60 → 0.63 after
  unrelated short notes showed up as related; a plain message when the local model doesn't fit in
  free memory.
- Late: auto-update (`tauri-plugin-updater`, GitHub Releases, signed bundles, Settings → Updates,
  Noughtpad → Check for Updates…). Updater key generated; the user stores it as GitHub secrets.

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
- **Updates only reach published releases:** tag pushes make drafts; nothing updates until you
  press Publish. A release built without the updater secret has no `latest.json`, and publishing
  it makes "Check for updates" fail until a release with one is published.
- **Updater key:** lose the private key or its password and installed copies can't update any more;
  they need one manual reinstall of a build with a new public key.
- **Intel Macs** get no update (the release builds Apple Silicon only).
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
