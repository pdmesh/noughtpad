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
- [ ] Auto-update: updater key generated (public half in `tauri.conf.json`), `tauri-plugin-updater` reading `latest.json` from the newest published GitHub release, Release workflow publishing signed update bundles and `latest.json`, a quiet "Check for updates" in Settings and the macOS app menu (`docs/DESIGN.md` § Auto-update). Checked: release v0.1.1 built signed update bundles and `latest.json` on all three platforms. Releases now go to the public `pdmesh/noughtpad-releases` (installers only) and the app asks there (2026-11-04). Left: the first release there, then the two-release check on the Mac (steps in HANDOFF)
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
- [ ] Slice 3, Google Drive backend and the app wiring (Settings → Sync, status dot). Code done 2026-11-04 (DESIGN §12.15); left: a real sign-in and two-Mac sync. Details: `drive.file` access, OAuth (PKCE, desktop loopback), Drive changes feed, works on every platform. Testing mode while dogfooding (sign in again every 7 days); production needs Google's basic project checks
- [ ] iCloud backend: the app's iCloud container, using CloudKit or `NSFileCoordinator`. Needs the Apple Developer Program (signed app). **Only works on Apple devices**, because iCloud has no official API for Windows, Linux or Android. Non-Apple devices use Google Drive (or a future Noughtpad server)
- [ ] Licensing and billing: account, licence key check, subscription (e.g. Paddle or Lemon Squeezy, which handle VAT)
- [ ] The index stays local and disposable on every device; notes, other files, user-state, chats and trash sync

Exit: edit the same notebook on two Macs and one non-Apple device for a week with no lost edits.

## Later: M7, mobile apps (paid with Sync)

- [ ] iOS and Android with Tauri 2 mobile, reusing the React UI and the Rust core
- [ ] Same distraction-free capture: open to a blank page, labels and source on demand, quick capture widget or shortcut
- [ ] Search and related notes on device (BGE-small is small enough); chat through your own provider. Local LLMs on phones are a later experiment
- [ ] Sync through M6

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
