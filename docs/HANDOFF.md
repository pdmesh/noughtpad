# Handoff: next session (M5 on a real Mac)

Read this first next session, then `ROADMAP.md` (what's next) and
`docs/DESIGN.md` (why things are the way they are).

## Where things stand (2026-11-04, later)

- **M6 slice 3 code is done, untried against real Drive** (DESIGN §12.15): the Google Drive
  backend and sign-in, the `Service` the app calls, Settings → Sync and the top-bar dot. The
  app links an open stub by default; `pnpm sync <command>` builds with the real crate.
- **Releases move to the public `pdmesh/noughtpad-releases`** (installers only), and the
  updater asks there. Installed 0.1.1 copies still ask the old address, so the next release is
  installed by hand once.
- **v0.1.2 is published on `pdmesh/noughtpad-releases`** (2026-11-04; the updater address answers 200 with version 0.1.2): the first release
  through the new path, with sync linked in (each job's log shows `noughtpad-sync v0.1.0 (…)`) and
  the Google client secret compiled in. All three jobs green; `latest.json` says 0.1.2, every URL
  points at `noughtpad-releases/releases/download/v0.1.2/…`, all nine entries signed, Mac
  `.dmg` and `Noughtpad_aarch64.app.tar.gz` + `.sig` present. Nothing new on pdmesh/noughtpad.
  Both secrets (`RELEASE_TOKEN`,
  `NOUGHTPAD_GOOGLE_CLIENT_SECRET`) are set.

- **v0.1.3 is published** (2026-11-04; `latest.json` serves 0.1.3). It holds: on the user's first launch 0.1.2 stuck on a blank
  window (the UI waiting for `vault_info`); cause not pinned down. A restart opened in 9.7 s, the
  next in 0.4 s. In response: the one-time `user-state.sqlite` migration is gone (no users to carry
  over); a single-instance guard (`tauri-plugin-single-instance`; a second launch shows the
  running window and exits); a startup loader with the current step ("Opening your notes"…,
  `startup-step` event, shown after 250 ms; `pnpm dev` with `?slowstart` to see it) and each step's
  time in the log (`open step: … ms=`), so the next slow start says where it went; and the app
  icon without the amber dot. The updater check passed (0.1.2 → 0.1.3, no Gatekeeper prompt).
  The slow first launch is macOS's privacy check on `~/Documents` for each new ad-hoc build
  (DESIGN § Auto-update): 5.9 s once per build, then 0.4 s. Goes away with signing.

Earlier the same day:

- **M6 sync: slices 1 and 2 are done** (`docs/DESIGN.md` §12.13–12.14). The design was settled
  with the user: eager merge rules, edit beats delete, notes + images/other files + user-state +
  chats + trash sync, Google Drive first in Testing mode, iCloud after the signing decision, the
  smaller points as recommended (§12.12).
  - Open core (this repo): device id, user-state as per-device logs (an old
    `user-state.sqlite` is ignored; the one-time copy was dropped after 0.1.2), `noughtpad_core::sync`
    (`SyncBackend`, `FolderBackend`, `Journal`, `is_synced`), `Vault::apply_synced` and friends.
  - Private repo `pdmesh/noughtpad-sync` (check it out next to this one; `cargo test` there):
    the engine and merge rules, 39 tests including a random three-device test.
  - **Not in the app yet:** the app doesn't run the engine. Dismissals and feedback now live in
    `.noughtpad/user-state/<device-id>.jsonl`.

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
  **auto-update** (code, key and workflow done; **v0.1.1 is published** with signed update files,
  but updates can't reach anyone while the repo is private: see "Auto-update: parked on where
  releases live" below).
- **The user is using 0.1.1 on their own machines for a few days** (installed by hand from the
  release), then decides the release path.
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
cargo fmt --all --check            # CI fails on formatting
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

### 1. Pick the next piece of work (the user chooses)

The user is living with 0.1.1 on several machines for a few days. Good next pieces, none blocked:
- **Try sync on the Mac** (the Google Cloud project `noughtpad-app` is set up: Desktop client,
  `drive.file` scope, the user as test user; check the Drive API is enabled):
  1. `git clone https://github.com/pdmesh/noughtpad-sync` next to this repo.
  2. `export NOUGHTPAD_GOOGLE_CLIENT_SECRET=…` (from the Google Cloud console).
  3. `pnpm sync pnpm tauri build --features local-llm-metal --bundles app`, then open it on a
     copy of the sample notebook: `open --env NOUGHTPAD_VAULT="$HOME/Noughtpad Sample" …`.
  4. Settings → Sync → Sign in with Google… (Google warns the app is unverified while in
     Testing: Continue), then "Sync this notebook". Check `My Drive/Noughtpad/Noughtpad Sample`.
  5. Second Mac (or a second notebook folder with the same name on this Mac): same build, same
     folder name, sign in; edit on both, delete on one, watch the dot and the notices.
  Run the binary from Terminal for the log (`sync failed: …`, Drive error messages). Things
  most likely to need fixing: Drive API responses I couldn't test here (revisions on a new
  file, change-feed edge cases), the browser hand-off, the keychain prompt for the new entry.
- **Release 0.1.2** (published): install it by hand once (0.1.1 asks
  the old address), then the two-release updater check (below) with 0.1.3 as release B. The
  release build has the Google secret compiled in, so it's also the easiest way to try sync
  (steps above, skipping 2–3).
- **Minor items** from "Parked" below (UX leftovers such as dark mode and the minimum window size,
  chat history delete, related-notes dismiss/undo), or anything the user hit while dogfooding.

Ship fixes as tagged releases (bump `version` in the root `Cargo.toml` and `package.json`, tag
`v0.1.x`, push, wait, check the draft, ask before publishing). Until updates work, the user
installs each one by hand from https://github.com/pdmesh/noughtpad-releases/releases (public; a
draft needs `gh release download v0.1.x --repo pdmesh/noughtpad-releases -p '*.dmg'`). Releases up
to 0.1.1 are on the private pdmesh/noughtpad.

### Auto-update: releases live on pdmesh/noughtpad-releases (decided 2026-11-04)

Done: `tauri-plugin-updater` (`src-tauri/src/updater.rs`), the public key and endpoint in
`src-tauri/tauri.conf.json`, Settings → Updates, Noughtpad → Check for Updates…, and `release.yml`
signing update bundles and writing `latest.json`. Both updater secrets are stored on
pdmesh/noughtpad. **v0.1.1 is published** and its `latest.json` is correct (`docs/DESIGN.md`
§ Auto-update has what was seen). The Mac bundle is `Noughtpad_aarch64.app.tar.gz` (+ `.sig`).

The blocker: pdmesh/noughtpad is **private**, so
`https://github.com/pdmesh/noughtpad/releases/latest/download/latest.json` answers 404 to the app
(which asks without signing in). Every build has that address built in. Options, the user's call:
1. Make the repo public: nothing changes, 0.1.1 can update as is.
2. A separate public repo just for releases (e.g. `pdmesh/noughtpad-releases`): point
   `plugins.updater.endpoints` there, have `tauri-action` upload to it (`owner`/`repo` inputs plus
   a token secret with Contents write on that repo only). Installs from 0.1.1 can't follow; start
   the check from the first build with the new address.
3. Host on R2/S3: more setup, no gain yet.

Once decided, the two-release check (release A, install like a user, release B, Check for
Updates… → Install and restart) is the rest of this item. Record in DESIGN.md § Auto-update what
Gatekeeper does on install, after the update restart and on a later reopen, how long the download
took, and any keychain prompt; then drop "Left: …" from the ROADMAP item and tick it.

Checks for that run: download the `.dmg` in Safari (the user clicks; the built-in browser may not
set quarantine), confirm `xattr -p com.apple.quarantine` on it, copy with a Finder drag, and while
0.1.1 runs check `ps -o comm= -p $(pgrep noughtpad-app)` is under /Applications, not
`AppTranslocation` (a translocated app can't replace itself). Wait for all three Release jobs
before publishing: each edits `latest.json` on the draft.

What Gatekeeper should do (expected, not yet seen): on macOS 15 and later, right-click → Open may
no longer offer "Open" for an unsigned app; instead it's System Settings → Privacy & Security →
**Open Anyway** plus a password (the user presses it, it's a security setting), or
`xattr -cr /Applications/Noughtpad.app` if the user agrees. Updates are downloaded by the app,
which doesn't mark them quarantined, so the updated app should open with no prompt. Each build has
a new ad-hoc signature, so the keychain may ask "Noughtpad wants to use … app.noughtpad.llm"
(Always Allow). If an update gives "Noughtpad is damaged", `xattr -cr` fixes it; record it.

If something fails: run `/Applications/Noughtpad.app/Contents/MacOS/noughtpad-app` from Terminal
for the log (`update check`, `update: …`). A wrong key shows as "The download didn't match
Noughtpad's signature".

### 2. Then
- **Dogfood** (ROADMAP M5 exit) from an installed release with "Open at login" on.
- **Signing**, deferred: unsigned builds for a few weeks, then decide. If the user pays: Apple
  Developer Program, then the secrets listed at the top of `release.yml`. The updater key is
  separate from Apple's.

### Parked (do when convenient, not blocking)
- UX pass leftovers: dark mode, minimum window size, keyboard-only use, related-notes dismiss/undo,
  chat history delete/hide, leaving a chat while it streams, images and Mermaid, the user's real
  notes (and whether `min_related` 0.63 holds there).
- Providers: Anthropic, OpenAI, LM Studio, the no-tools fallback, reading a key back after a rebuild.
- Startup: under 1 s is fine. Optimise when there's an easy win; the ~550 ms seen late on
  2026-11-03 (vs ~350 ms earlier) needs no chase.

### Done on 2026-11-04
- M6 sync slice 1 (open core) and slice 2 (engine in `pdmesh/noughtpad-sync`); the smaller
  design points settled as recommended. `Note::extra`/`set_extra` added for the merge, and a
  frontmatter change to hand-written keys alone is now written out (it was kept verbatim).

### Done on 2026-11-03
- Late night (cloud session): M6 sync design written in `docs/DESIGN.md` §12 after the user's
  answers (decisions log entry 2026-11-03); ROADMAP M6 split into slices.
- Morning: renamed Nought → Noughtpad; Mac build with Metal and speed numbers; startup bundle split
  (1.6 MB → 0.6 MB); window lifecycle per platform and the "Open at login" switch.
- Evening: Mac checks and a UX pass on the sample notebook. Added a macOS menu bar
  (`src-tauri/src/menu.rs`) with Settings ⌘, and the app's commands; fixed a blank page on the
  first visit to each page (`page()` in `src/App.tsx`); raised `min_related` 0.60 → 0.63 after
  unrelated short notes showed up as related; a plain message when the local model doesn't fit in
  free memory.
- Late: auto-update (`tauri-plugin-updater`, GitHub Releases, signed bundles, Settings → Updates,
  Noughtpad → Check for Updates…). Updater key generated; the user stored it as GitHub secrets.
- Night: released and published v0.1.1 (signed update files on all three platforms). Found the
  private repo blocks the updater (404). Fixed CI formatting (`cargo fmt`).

## Known limits and gotchas

- **Building with sync:** always through `pnpm sync …` (it patches the stub and puts
  `Cargo.lock` back). A plain `pnpm tauri build` has no sync and Settings shows no Sync section.
  If a run is killed, delete `.cargo/config.toml` and `Cargo.lock.sync-build` (restore the lock).
- **Drive in Testing mode:** sign-ins expire after 7 days (Settings shows "Signed out…"; sign in
  again), and only the listed test users can sign in.
- **Renaming files in Drive by hand** isn't supported: they read as new files, and the old
  path as deleted.
- **Sync repo:** `noughtpad-sync` uses `../noughtpad/crates/noughtpad-core` as a path dependency,
  so it only builds with both repos side by side. It has no CI yet.
- **Folder backend** is for tests: its lock file isn't safe across machines (e.g. over Dropbox).

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
- **Private repo:** release files aren't downloadable without signing in to GitHub, so the app
  can't update (see "Auto-update: parked") and installs need `gh release download`.
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
