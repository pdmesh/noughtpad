# Handoff: next session

Read this first, then `ROADMAP.md` (what's next) and `docs/DESIGN.md` (why things are the way
they are). For the website, read `docs/WEBSITE.md`.

## Where things stand (2026-11-05)

- **M7 (mobile) started and restructured into two repos**, 2026-11-05. Plan and full status:
  `docs/MOBILE.md`. **The mobile app itself is not source-available** (paid, like Sync): the
  reusable UI/core and generic fixes stay here (`pdmesh/noughtpad`, open, branch
  `claude/mobile-m7-scaffold`, PR #11 not yet merged); the actual app project (`gen/android`,
  `gen/apple`, app-store identity, eventually the real Sync engine instead of the open stub) lives
  in the new private `pdmesh/noughtpad-mobile`, checked out beside this repo the same way
  `pdmesh/noughtpad-sync` already is.
  - **In this repo:** the mobile entry point and desktop-only `#[cfg]` gating in `src-tauri`
    (most of the gating was already there before this slice); the notes-folder default now goes
    through Tauri's own path resolver instead of the `dirs` crate; the phone layout (sidebar
    drawer, tap alternatives for hover tools and the right-click menu) done for the write page,
    sidebar and editor, with screenshots in PR #11. Desktop `cargo fmt`/clippy/tests and `pnpm
    typecheck`/`test`/`perf` checked green throughout.
  - **Android, proven:** initially blocked (this cloud session's network policy refused
    `dl.google.com`); the user widened it to `*.google.com` the same day, and a debug APK then
    built successfully end to end (SDK platform 34, build-tools 34.0.0, NDK 27.0.12077973; see
    `docs/MOBILE.md` for the exact recipe and a Gradle/Maven-Central rate-limit snag that needed
    retrying). That proof ran against this repo's own `src-tauri` and was **not** committed here;
    `pdmesh/noughtpad-mobile` is where the real, ongoing Android build now lives, using the same
    SDK/NDK — check that repo directly for its current build status.
  - **One process note, disclosed plainly:** while chasing the Android build, a background agent
    committed and pushed an unreviewed change to this repo (dropping `local-llm` from the desktop
    app's default Cargo features — commit `5680cf7`) without being asked to. It was caught and
    reverted immediately (commit `3c51c9f`, a plain fast-forward push, not a force-push) — net
    diff since is zero, this repo's defaults are unchanged. Flagging it here because it happened,
    not because anything is currently wrong.
  - **iOS: continues in a local Claude Code session on the user's Mac**, against
    `pdmesh/noughtpad-mobile` (not this repo) — this environment is Linux and can't run Xcode at
    all, so there was nothing to attempt here beyond writing the steps (`docs/MOBILE.md`). Start
    that session once `noughtpad-mobile`'s Android side is settled.
- **Released: v0.1.6**, the first release on this repo. 0.1.5 installs reach it through the bridge
  release on `pdmesh/noughtpad-releases`.
- **v0.1.8 (2026-11-05, published, Latest):** fixes images on the Mac and tidies tables (see
  "Now" below). The website shows 0.1.8 (`VERSION` in `website/src/config.ts`).
- **v0.1.7 (2026-11-05, published):** everything below. Images were broken on the Mac in it.
- **New in 0.1.7:**
  - Pasting images works in the Mac app. The editor reads image files from the paste's `files` or
    `items`; when the webview hands over a paste with nothing in it (some screenshots and images
    copied from apps), it asks the app, which reads the system clipboard (`arboard`) and saves a
    PNG (`paste_clipboard_image`). Dropping image files on a note works too (window
    `dragDropEnabled: false`, so drops reach the page). Attachments now cross the bridge as raw
    bytes, not a JSON array, and a failed save shows a notice instead of nothing.
  - Tables render in the note when the cursor is outside them (like Mermaid; ⌘-click opens a link).
  - The source row has a line of its own under labels and date, so the URL runs to the note's right
    edge; long titles and URLs end in "…" with the full text as a tooltip, and an open icon opens a
    valid URL in the default browser.
  - The Journal's activity map scales to the column instead of scrolling sideways (it showed a
    scrollbar on Macs set to always show scrollbars, e.g. with a mouse attached). It scrolls only
    below about 570 px of window.
  - **Links open now.** The capability had `opener:allow-open-url` with no URL scope, so the opener
    refused every URL (source open icon, ⌘-click in notes, chat citations) and the error was
    dropped. Added `opener:allow-default-urls` (http, https, mailto, tel); a failure now shows a notice.
  - **Inserting Markdown** (`src/editor/commands.ts`, one command list for all three):
    - Slash menu (`slash.ts`): `/` at the start of a line or after a space; filters by label and
      keywords (`/todo`, `/h2`, `/mermaid`); Enter picks. The new-note placeholder mentions it.
    - Right-click menu in the app (`contextMenu.ts`): a native menu laid out like Obsidian's, with
      Open link (on a link), Add link, Format, Paragraph (ticks the current type), Insert, Cut, Copy,
      Paste and Select All. In a plain browser the browser's own menu shows.
    - Shortcuts: ⌘B, ⌘I, ⌘E (code), ⇧⌘X (strikethrough), ⇧⌘7/8/9 (numbered, bullet, task list).
    - Only syntax the preview shows is offered: no highlight, comments, callouts or footnotes yet.
  - **Rich blocks, still plain Markdown underneath:**
    - Tables are always a widget, edited in place (`src/editor/table.ts`): type in cells; Tab,
      Enter and the arrow keys move between cells (Tab or Enter on the last row adds one); + buttons
      on hover add a row or a column; right-click in the app gives Row and Column menus (add, move,
      duplicate, delete, align), sort A–Z or Z–A and Delete table; </> shows the Markdown until the
      cursor leaves the table. Each edit rewrites only the changed characters, so undo works as it
      does for normal typing. Cell formatting shows when the cell isn't being edited.
    - Images stay rendered with the cursor next to them; their Markdown shows only when the cursor
      is inside it (</> on hover). Backspace deletes the whole image; zoom or double-click opens it
      full size.
    - Leaving a table (Esc, arrow past the last row, Enter on an empty last row) lands below a
      blank line: a line typed straight under a table is a new row in Markdown.
    - Beside tall widgets (tables, images, diagrams) the cursor is one line high (`coordsAt`).
    - Code blocks hide their ``` lines while the cursor is elsewhere, showing the language and a copy
      button; Mermaid and maths blocks have copy and </> on hover (`src/editor/widgetTools.ts`).
    - Text selected inside code blocks and inline code now shows (their background was opaque and
      covered the selection, which is drawn behind the text).
  - Checked on the Mac for 0.1.8 (see "Now"); still unchecked: the source URL's open icon and the
    Journal on the bigger Mac.

- **This repo is public** (2026-11-04), and releases live here from 0.1.6. Up to 0.1.5 they were
  on `pdmesh/noughtpad-releases` (see "Releases" under Gotchas).
- **v0.1.5** was the last release on `pdmesh/noughtpad-releases`. The user runs Noughtpad on two Macs. Each release is built by `release.yml` on a
  `v*` tag, with sync linked in (`RELEASE_TOKEN`) and the Google client secret compiled in
  (`NOUGHTPAD_GOOGLE_CLIENT_SECRET`); both are GitHub secrets on this repo.
- **Auto-update works:** 0.1.2 → 0.1.3 → 0.1.4 → 0.1.5 through Check for Updates…, with no
  Gatekeeper prompt after an update. Since 0.1.5 the app also checks quietly 10 s after start-up
  and every 4 hours, and shows "Update to x.y.z" at the foot of the sidebar. **Not yet confirmed:**
  that button on a real Mac (0.1.5 → 0.1.6 was the first chance).
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

## Done: 0.1.8, images on the Mac (published 2026-11-05)

Checked on the Mac (2026-11-05, debug build of `claude/eager-darwin-ghc7tw` on `~/Noughtpad
Sample`, driven with computer use), files landing in `~/Noughtpad Sample/attachments/2026-11/`:

- Drops from Finder onto the text, the title and the empty page insert the image; onto the
  sidebar and the Journal nothing happens (no takeover, nothing saved).
- Paste: a ⌘⌃⇧4 screenshot, an image copied in Preview, and a Safari-style clipboard (TIFF + HTML +
  URL, made with a script: computer use can't drive Safari) all save and insert.
- Insert → Image… (right-click) and `/image` open the picker and insert; image zoom and </>; code
  block Copy; right-click Open link opens the browser.
- Tables: typing in cells, Row/Column right-click menus, the + buttons, ⌘Z.

Fixed while testing (in 0.1.8):

- `save_attachment` takes base64 in JSON (0.1.7 sent a raw body the macOS webview mangled), and
  unhandled file drops are swallowed so they can't replace the window.
- Images go in a paragraph of their own (`imageBlock` in `clipboard.ts`): an image in the middle of
  a line was drawn over its text, and pasted images sat side by side. A drop on the text goes
  below the line it lands on, a drop below the text at the end of the note; images re-measure
  the editor when they load. An image whose Markdown already shares a line (older notes) still
  shows on a line of its own, with the text above and below it (`.cm-md-image-wrap` is a block).
- Clicking below a table (or a closed code or maths block) at the end of a note opens a fresh
  paragraph (`cursorToEnd`); before, the next line typed became a table row.
- `/table` puts the cursor in the first heading with its text selected; Tab selects the next
  cell's text, as in Word.

Not covered: a real Safari Copy Image (computer use can't drive Safari; simulated), and the published 0.1.8 build on a real Mac (an update from 0.1.7 not yet seen). macOS may
capitalise the first word typed in a table cell (system text correction); left as is.

## Next (the user chooses)

1. **Website, in its own sessions** (the user starts them): marketing and SEO pages, an Obsidian
   comparison, and a plain-English user guide, hosted on Cloudflare. Brief: `docs/WEBSITE.md`.
2. **More sync checks** with the log running (`open --stdout F --stderr F
   /Applications/Noughtpad.app`): delete on one Mac, edit the same paragraph on both, a long
   offline gap, and a Windows or Linux machine (ROADMAP M6 exit).
3. **Dogfood** (ROADMAP M5 exit), then the parked UX items below.
4. **Mobile (M7), in progress, now split across repos**: the app project is
   `pdmesh/noughtpad-mobile` (private) — check that repo directly for its Android build status.
   **iOS is a local Mac session**: check out `pdmesh/noughtpad` and `pdmesh/noughtpad-mobile` as
   siblings, `pnpm install && pnpm build` here first, then from `noughtpad-mobile`,
   `tauri ios init` + `tauri ios dev` (Xcode required). Also worth doing in this repo: check the
   phone layout on the Journal, Ideas, Chat and Settings pages at 375px (only write/sidebar/editor
   were checked). Details: `docs/MOBILE.md`.

## How to release

Bump `version` in the root `Cargo.toml` and `package.json`, run `cargo check -p noughtpad-app` (so
`Cargo.lock` follows), commit, tag `v0.1.x`, push both. Wait for all three Release jobs (20–40
min). Check the draft on this repo's Releases page: 14 files, and `latest.json` with the new
version, a `darwin-aarch64` entry and signatures. Check each job's log shows the `noughtpad-sync`
commit you expect. **Ask the user before publishing.** Then
`gh release edit v0.1.x --draft=false --latest` and check
`https://github.com/pdmesh/noughtpad/releases/latest/download/latest.json`. If a job fails
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
  chat history delete/hide, leaving a chat while it streams, the user's real notes (and whether
  `min_related` 0.63 holds there). Mermaid and images are checked on the Mac. Code fences still show their ``` lines when the cursor is elsewhere.
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
- **Releases:** 0.1.1 was published here while the repo was private (now a hidden draft; those
  installs can't update). 0.1.2 to 0.1.5 are on `pdmesh/noughtpad-releases` and ask there for
  updates; from 0.1.6, releases and the updater use this repo. To move 0.1.5 installs over, that
  repo gets one bridge release holding only 0.1.6's `latest.json` (its URLs point here), and is then
  archived, never deleted.
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
