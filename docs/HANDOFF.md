# Handoff: next session

Read this first, then `ROADMAP.md` (what's next) and `docs/DESIGN.md` (why things are the way
they are). For the website, read `docs/WEBSITE.md`.

## Where things stand (2026-11-04, end of day)

- **Released: v0.1.5**, published on the public `pdmesh/noughtpad-releases` (installers only; this
  repo stays private). The user runs it on two Macs. Each release is built by `release.yml` on a
  `v*` tag, with sync linked in (`RELEASE_TOKEN`) and the Google client secret compiled in
  (`NOUGHTPAD_GOOGLE_CLIENT_SECRET`); both are GitHub secrets on this repo.
- **Auto-update works:** 0.1.2 → 0.1.3 → 0.1.4 → 0.1.5 through Check for Updates…, with no
  Gatekeeper prompt after an update. Since 0.1.5 the app also checks quietly 10 s after start-up
  and every 4 hours, and shows "Update to x.y.z" at the foot of the sidebar. **Not yet seen:**
  that button on a real Mac (first chance: the release after 0.1.5).
- **Sync works between the user's two Macs** (M6 slice 3, verified 2026-11-04): Google Drive,
  `My Drive/Noughtpad/<notes folder name>/`, Testing mode. Seen: a new note and edits reaching
  the other Mac; an open note updating in place when the other Mac's edit arrives (0.1.5).
  **Timing:** a change uploads about 5 s after typing stops (`AFTER_CHANGE`); every device also
  syncs every 60 s (`EVERY`) and about 5 s after its window gets focus. So an edit shows on the
  other device within about a minute, or a few seconds after you switch to it.
  **Not yet seen:** a delete on one Mac, the same note edited on both at once (merge or conflict
  copy), a non-Apple device, and the Testing-mode sign-in expiring after 7 days.
- **Fixed today on the way:** duplicate `Noughtpad` folders in Drive (two sync runs at once;
  0.1.4), an open note not refreshing after a sync (0.1.5), a blank window while the notes open
  (now a loader with the current step; 0.1.3), a second copy of the app opening the same notebook
  (single-instance guard; 0.1.3).
- **Also new:** note text uses the system font and size; Settings → Writing changes font and size
  (this computer only, `localStorage`). The note is centred beside the sidebar and related panel.
- **Slow first launch after each update** (about 6 s, then 0.4 s): macOS's privacy check on
  `~/Documents` for each new unsigned build (DESIGN § Auto-update). The user chose to leave it;
  Apple signing would fix it. Once, the webview's page load itself started 5.9 s late; not chased.
- **Apple signing is still deferred** (no Developer Program yet).

## Next (the user chooses)

1. **Website, in its own sessions** (the user starts them): marketing and SEO pages, an Obsidian
   comparison, and a plain-English user guide, hosted on Cloudflare. Brief: `docs/WEBSITE.md`.
2. **More sync checks** with the log running (`open --stdout F --stderr F
   /Applications/Noughtpad.app`): delete on one Mac, edit the same paragraph on both, a long
   offline gap, and a Windows or Linux machine (ROADMAP M6 exit).
3. **Dogfood** (ROADMAP M5 exit), then the parked UX items below.

## How to release

Bump `version` in the root `Cargo.toml` and `package.json`, run `cargo check -p noughtpad-app` (so
`Cargo.lock` follows), commit, tag `v0.1.x`, push both. Wait for all three Release jobs (20–40
min). Check the draft on `pdmesh/noughtpad-releases`: 14 files, and `latest.json` with the new
version, a `darwin-aarch64` entry and signatures. Check each job's log shows the `noughtpad-sync`
commit you expect. **Ask the user before publishing.** Then
`gh release edit v0.1.x --repo pdmesh/noughtpad-releases --draft=false --latest` and check
`https://github.com/pdmesh/noughtpad-releases/releases/latest/download/latest.json`. If a job fails
while uploading, see "Release upload race" below. Sync changes must be pushed to
`pdmesh/noughtpad-sync` main before tagging: the release checks out its `main`.

## Testing on the Mac

- Local build: `pnpm tauri build --features local-llm-metal --bundles app`; with sync:
  `pnpm sync pnpm tauri build …`. Local builds have no Google secret, so they can't sign in; use
  a release build to test real sync.
- Quit the installed app first: the single-instance guard sends a second launch to the running
  copy. Launch with a log: `open --env RUST_LOG=info --stdout F --stderr F <app>`, or run
  `Contents/MacOS/noughtpad-app` with `NOUGHTPAD_VAULT="$HOME/Noughtpad Sample"` for the sample.
- Every command is in the menu bar, so background computer use can drive it with `app_menu`.
- Browser-only UI with the mock backend: `pnpm dev`, with `?slowstart` (startup loader),
  `?update=available` (sidebar update button), `?remoteedit` (Sync now edits the newest note as
  another device would).
- The user's real notes folder is `~/Documents/Noughtpad`, synced. `~/Noughtpad Sample` is for
  tests (`node scripts/sample-vault.mjs` recreates it).

## Parked (do when convenient)

- UX leftovers: dark mode, minimum window size, keyboard-only use, related-notes dismiss/undo,
  chat history delete/hide, leaving a chat while it streams, images and Mermaid, the user's real
  notes (and whether `min_related` 0.63 holds there).
- Turn on "Sync this notebook" automatically after signing in (the user missed the switch once;
  asked, not answered).
- Providers: Anthropic, OpenAI, LM Studio, the no-tools fallback, reading a key back after a rebuild.
- Release workflow: serialise the jobs' `latest.json` edits if the upload race happens again.
- A sturdier Drive lookup: keep the notebook folder's id in sync state and check it by id, so a
  lagging Drive search can't make a second folder.

## Repo map

| Path | What |
|---|---|
| `crates/noughtpad-core` | Vault, SQLite index, FTS, embeddings, related notes, themes (`semantic/ideas.rs`), chat context scopes (`semantic/context.rs`) |
| `crates/noughtpad-llm` | Model catalogue, checked and resumable downloads, local runtime (mistral.rs), remote providers and the tool loop (genai), keychain, prompts |
| `src-tauri/src` | `lib.rs` (vault commands), `lifecycle.rs` (window hide/reopen, open at login), `menu.rs` (macOS menu bar), `updater.rs` (check, install, restart), `semantic.rs`, `think.rs` (Models page commands), `chat.rs` (chat, tools, consent, `.noughtpad/chats/`), `sync.rs` (Settings → Sync, keychain sign-in, refresh after a sync) |
| `crates/noughtpad-sync-stub` | Open stand-in for Noughtpad Sync; `pnpm sync <cmd>` swaps in the private `pdmesh/noughtpad-sync` (checked out next to this repo) |
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
- **Releases up to 0.1.1** are on the private `pdmesh/noughtpad` and can't update themselves;
  everything from 0.1.2 is on `pdmesh/noughtpad-releases`.
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
- **Release upload race:** the three jobs each edit `latest.json` on the draft. On v0.1.5 the macOS
  job built fine but failed uploading ("Not Found - update a release asset"), leaving the `.dmg`
  but no update bundle and no `darwin-aarch64` entry. Fix used: delete the half-uploaded Mac
  assets from the draft, `gh run rerun <id> --failed`, then check `latest.json` lists
  `darwin-aarch64`. If it recurs, serialise the jobs' uploads (e.g. `max-parallel: 1` on the
  upload, or a final job that writes `latest.json`).
- **Release CI time:** the release build compiles mistral.rs, which takes 20–40 minutes per
  platform on GitHub runners.
- **Old answers:** answers saved with "Keep" before M4.5 remain in `.noughtpad/artifacts/` and
  aren't shown. That's intentional (dropped).
