# Noughtpad roadmap

The working plan. Design detail and the reasons behind decisions are in
[`docs/DESIGN.md`](docs/DESIGN.md). Each item has a one-line exit criterion.

## Done

| Milestone | What shipped |
|---|---|
| M1 Notebook | Markdown vault, CodeMirror live preview, autosave, labels, sources, FTS search, journal, collections, quick capture |
| M2 Quiet retrieval | Local embeddings (BGE-small), hybrid search, related notes with reasons, dismissals, learning from feedback |
| M3 Ideas over time | Themes with stable identity, returning/forming/dormant, influences, open questions, activity map |
| M4 Models | Explicit opt-in Models page: local GGUF models (mistral.rs, default Qwen3-4B) or your own key (Anthropic, OpenAI, Gemini, Ollama, OpenAI-compatible); keys in the OS keychain |
| M4.5 Chat | Full-page chat with your notes. History lives in a right-hand panel. Online models use read-only tools bounded by scope; local models get notes picked per question. Consent is per chat |
| M5 (started) | Release workflow (macOS with Metal, Windows, Linux), Settings page, capture-speed budget in CI |

## Now: M5, ship it to yourself

The goal is a build on your own Mac, used daily for two weeks.

- [x] Release workflow builds installers on demand (Actions → Release) and on `v*` tags. All three platforms are green
- [x] Settings page (folder, connections, models, layout, shortcuts)
- [x] Capture-speed budget in CI (open → typing; ~0.4 s, budget 1.5 s)
- [x] Renamed from Nought (taken) to Noughtpad before daily use: bundle ID, keychain service, `.noughtpad` data folder, crates, packages, docs
- [x] Mac build with Metal; startup and 10k-note speed measured (`docs/DESIGN.md` § M5 measurements); startup bundle split
- [x] Platform window lifecycle: on macOS closing hides and ⌘⇧Space/Dock bring it back; "Open at login" setting
- [x] Mac checks: local model speed with Metal (`docs/DESIGN.md` § M5 measurements), keychain (key stored under `app.noughtpad.llm`, not in `config.json`; deleted with the provider), "Open at login" (adds and removes `~/Library/LaunchAgents/Noughtpad.plist`). Left: log out and in to see the hidden launch; read a key back after a rebuild
- [ ] UX pass on macOS using computer use (rest parked, see HANDOFF). Done on the sample notebook: capture and ⌘⇧Space, paste/copy/undo right after launch, maths and tasks, related notes, ideas and theme pages, journal, chat (local and Ollama, follow-ups, citations, consent), models, preferences surviving a restart. Fixed: a macOS menu bar (Settings ⌘, and the app's commands were missing), a blank page on the first visit to each page, unrelated related-notes (threshold 0.60 → 0.63), a raw out-of-memory error. Left: dark mode, the minimum window size, keyboard-only use, dismiss/undo of related notes, chat history delete/hide, leaving a chat while it streams, images and Mermaid, your real notes
- [ ] Real providers (rest parked): Ollama works, including tool use. Left: an Anthropic or OpenAI key, LM Studio, and the fallback when a model can't use tools
- [x] Full 2.5 GB model download through the app: ~5 min, quit at 0.8 GB and it resumed from there
- [x] Auto-update: updater key generated (public half in `tauri.conf.json`), `tauri-plugin-updater` reading `latest.json` from the newest published GitHub release, Release workflow publishing signed update bundles and `latest.json`, a quiet "Check for updates" in Settings and the macOS app menu (`docs/DESIGN.md` § Auto-update). Checked: release v0.1.1 built signed update bundles and `latest.json` on all three platforms. Releases now go to the public `pdmesh/noughtpad-releases` (installers only) and the app asks there (2026-11-04). v0.1.2 published there with sync; 0.1.2 → 0.1.3 updated through Check for Updates… on the Mac with no Gatekeeper prompt (2026-11-04). The repo went public on 2026-11-04 and releases moved back here from 0.1.6
- [ ] Signing: Apple Developer Program ($99/yr) for notarisation; Windows via Azure Artifact Signing (~$10/mo) when Windows matters. Deferred (2026-11-03): a few weeks on your own Macs with right-click → Open first, then decide
- [ ] Dogfood for two weeks; track the dismiss rate of related notes and anything that slows capture

Exit: Noughtpad replaces your current notes app for two weeks without workarounds.

## Next: M6, Noughtpad Sync (paid, proprietary)

Built-in, two-way sync across devices. It isn't open source, so it can be charged for. The core
stays FSL; sync lives in a private repo (`pdmesh/noughtpad-sync`) that plugs into an open
`SyncBackend` interface in `noughtpad-core`. Design: `docs/DESIGN.md` §12 (written 2026-11-03).

- [x] Design (§12): what syncs, merge rules, tombstones, the interface, Drive vs iCloud
- [x] Slice 1, open core (2026-11-04): device id, user-state as per-device logs, `SyncBackend` and a folder backend for tests, `Vault::apply_synced` and the change journal

- [x] Slice 2, sync engine (private repo, 2026-11-04): per-file change journal, three-way merge for Markdown against a base copy in `.noughtpad/sync/`, conflict copies only when a merge fails, tombstones, edit beats delete; tested against the folder backend
- [x] Slice 3, Google Drive backend and the app wiring (Settings → Sync, status dot). Done 2026-11-04 (DESIGN §12.15): two Macs syncing through real Drive in 0.1.5. Left for the M6 exit: deletes and same-paragraph edits on both, a non-Apple device, a week of use. Details: `drive.file` access, OAuth (PKCE, desktop loopback), Drive changes feed, works on every platform. Testing mode while dogfooding (sign in again every 7 days); production needs Google's basic project checks
- [ ] iCloud backend: the app's iCloud container, using CloudKit or `NSFileCoordinator`. Needs the Apple Developer Program (signed app). **Only works on Apple devices**, because iCloud has no official API for Windows, Linux or Android. Non-Apple devices use Google Drive (or a future Noughtpad server)
- [ ] Licensing and billing: account, licence key check, subscription (e.g. Paddle or Lemon Squeezy, which handle VAT)
- [ ] The index stays local and disposable on every device; notes, other files, user-state, chats and trash sync

Exit: edit the same notebook on two Macs and one non-Apple device for a week with no lost edits.

## Started: M7, mobile apps (paid with Sync)

Plan and status: `docs/MOBILE.md`. iOS and Android with Tauri 2 mobile, reusing the React UI
(`src/`) and the Rust core (`crates/noughtpad-core`) as-is. **The mobile app itself is not
source-available** (like Sync): the actual app project (`gen/android`, `gen/apple`, app-store
identity) lives in the private `pdmesh/noughtpad-mobile` repo, not here — see
`docs/MOBILE.md` § "Where the mobile app project actually lives".

- [x] Design and slice plan (`docs/MOBILE.md`, 2026-11-05)
- [x] Slice 1, scaffolding: mobile entry point, desktop-only code switched off in this repo
      (`#[cfg(mobile)]` branches, `default_vault_path` via Tauri's path resolver), desktop
      `cargo fmt`/clippy/tests and `pnpm typecheck`/`test`/`perf` checked green throughout. A
      debug Android build was proven end to end against this repo's own `src-tauri` to validate
      the approach (SDK/NDK versions and the exact recipe are in `docs/MOBILE.md`), then that
      scaffolding moved to `pdmesh/noughtpad-mobile`, which now carries the actual app project
- [x] Slice 2, phone layout: sidebar as a drawer, 375px note page, tap alternatives for hover
      tools and the right-click menu. Done for the write page, sidebar and editor, checked with
      screenshots; not yet checked on Journal/Ideas/Chat/Settings
- [ ] Slice 3, runs on a real device: Android done — `pdmesh/noughtpad-mobile` builds a verified
      debug APK from its own `src-tauri` (see `docs/MOBILE.md`); not yet run on an actual device
      or emulator. iOS picks up in a local Mac session (see `docs/HANDOFF.md`)
- [ ] Slice 4, capture from outside the app (share sheet/intent), same shape as desktop's
      ⌘⇧Space
- [ ] Slice 5, Sync checked on a real phone alongside the two Macs
- [ ] Same distraction-free capture: open to a blank page, labels and source on demand (covered by
      reusing `src/`; a quick-capture widget/share target is slice 4, above)
- [ ] Search and related notes on device (BGE-small is small enough, unverified build-wise — see
      "Open risks" in `docs/MOBILE.md`); chat through your own provider. Local LLMs on phones are a
      later experiment (see `docs/MOBILE.md` § Local AI on mobile for what's actually available)
- [ ] Sync through M6, exercised on a real device (slice 5)

Exit: the same M6 exit, plus a phone — edit the same notebook on a phone and a Mac with no lost
edits, over the app's normal day-to-day use, not just a one-off test.

## Website (in progress, own sessions)

- [ ] Marketing and SEO pages, an Obsidian comparison, a download page and a plain-English user guide, hosted on Cloudflare. Brief: `docs/WEBSITE.md`

## Backlog

- Long scopes for local models: summarise in batches when a month or theme has many notes
- Languages beyond English: a multilingual embedding model (e.g. multilingual-e5-small)
- First-run experience and an accessibility and keyboard pass
- Import from Obsidian and plain-Markdown folders; a web clipper for sources
- Intel Mac and Linux ARM builds

## Dropped

- Showing old "Kept" answers (`.noughtpad/artifacts/`), replaced by chat history
- Workflow files (prebuilt and custom)
- Token and cost display
- Small judge models (reranker, NLI, Laya) and GLiNER concept extraction: evaluated, didn't beat embeddings plus rules
