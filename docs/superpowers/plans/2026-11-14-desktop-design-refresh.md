# Desktop Design Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raise the note's first line, add app-wide back/forward, show a green "synced" dot, and add four themes chosen as a light + dark pair.

**Architecture:** Two new pure modules with unit tests, `src/lib/theme.ts` (preferences → a resolved theme on `<html data-theme data-scheme>`) and `src/lib/navHistory.ts` (a browser-style stack). Wire them into `App.tsx`, `Settings.tsx` and `main.tsx`. Colours are token sets in `src/styles.css` keyed on `data-theme`, replacing the `prefers-color-scheme` blocks. The note-top change is a reorder in `NotePane.tsx`, a single-row `MetaStrip.tsx`, and CSS.

**Tech Stack:** React 19 + TypeScript, Vite, vitest, Tauri 2 (Rust menu in `src-tauri/src/menu.rs`), lucide icons.

**Spec:** `docs/superpowers/specs/2026-11-14-desktop-design-refresh-design.md`

## Global Constraints

- Desktop only. Don't change the phone rules inside `@media (max-width: 680px)` / coarse-pointer blocks, apart from the token rename in Task 2.
- No fixed bar is added. The top bar keeps its auto-hide behaviour.
- The default look is unchanged: Paper (today's light palette) for light and Graphite for dark, with Mode set to System.
- Preferences live in localStorage on this computer, like `textStyle.ts`, under the key `noughtpad.theme`.
- Shortcuts: ⌘[ = Back, ⌘] = Forward (Ctrl on Windows/Linux). Tab / Shift-Tab still indent in the editor.
- Comments explain *why*, in the house voice (see existing files). Each commit ends with the `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` line.

## Review Focus

1. ⌘[ while typing in the editor must go Back and must not outdent the line, since CodeMirror's `defaultKeymap` binds `Mod-[` to `indentLess`. → Task 4 (capture-phase handler), check by hand in Step 6.
2. A new note that saves, or a new chat that gets its id, must not add a second history entry. Back should skip the blank state it came from. → Task 3, tests `replaces a new note once it is saved` and `replaces a chat when it gets its id`.
3. Going Back to a note that has since been trashed or deleted must not crash. It should open the blank Write page instead. → Task 4, Step 4.
4. With Mode = System, switching macOS between light and dark while the app is open must re-theme it live. → Task 1 (`watchSystemTheme`), check by hand in Task 2, Step 5.
5. A source that has a URL but no title must still show something to click and edit, and must not collapse into nothing. → Task 5 (hostname placeholder), check by hand in Step 5.

---

### Task 1: Theme preferences module

**Files:**
- Create: `src/lib/theme.ts`
- Create: `src/lib/theme.test.ts`
- Modify: `src/main.tsx` (apply the theme before the first render, next to `applyStyle(loadStyle())`)

**Interfaces:**
- Produces:
  - `type Mode = "system" | "light" | "dark"`, `type LightTheme = "paper" | "white"`, `type DarkTheme = "graphite" | "midnight"`, `type ThemeName = LightTheme | DarkTheme`
  - `interface ThemePrefs { mode: Mode; light: LightTheme; dark: DarkTheme }`
  - `DEFAULT_THEME: ThemePrefs = { mode: "system", light: "paper", dark: "graphite" }`
  - `LIGHT_THEMES`, `DARK_THEMES`: `Array<{ value; label }>` with labels "Paper", "White", "Graphite", "Midnight"; `MODES` with labels "System", "Light", "Dark"
  - `parseTheme(raw: string | null): ThemePrefs` (any bad field falls back to its default)
  - `resolveTheme(p: ThemePrefs, systemDark: boolean): { name: ThemeName; scheme: "light" | "dark" }`
  - `loadTheme(): ThemePrefs`, `saveTheme(p: ThemePrefs): void` (applies, then stores; storage errors are ignored), `applyTheme(p: ThemePrefs): void` (sets `document.documentElement.dataset.theme` and `.dataset.scheme`, plus `style.colorScheme`)
  - `watchSystemTheme(): () => void`: listens to `(prefers-color-scheme: dark)` changes and re-applies `loadTheme()`; returns an unsubscribe function

- [ ] **Step 1: Write the failing tests** in `src/lib/theme.test.ts`:
  - `parseTheme(null)` and `parseTheme("not json")` equal `DEFAULT_THEME`
  - `parseTheme('{"mode":"dark","light":"white","dark":"neon"}')` equals `{ mode: "dark", light: "white", dark: "graphite" }`
  - `resolveTheme(DEFAULT_THEME, false)` → `{ name: "paper", scheme: "light" }`; `resolveTheme(DEFAULT_THEME, true)` → `{ name: "graphite", scheme: "dark" }`
  - `resolveTheme({ mode: "light", light: "white", dark: "midnight" }, true)` → `{ name: "white", scheme: "light" }`
  - `resolveTheme({ mode: "dark", light: "white", dark: "midnight" }, false)` → `{ name: "midnight", scheme: "dark" }`
- [ ] **Step 2: Run** `pnpm vitest run src/lib/theme.test.ts`. Expected: FAIL (the module is missing).
- [ ] **Step 3: Implement `src/lib/theme.ts`**, modelled on `src/lib/textStyle.ts`.
- [ ] **Step 4: Call `applyTheme(loadTheme())` and `watchSystemTheme()` in `src/main.tsx`** before `createRoot`.
- [ ] **Step 5: Run** `pnpm vitest run src/lib/theme.test.ts && pnpm typecheck`. Expected: PASS.
- [ ] **Step 6: Commit** `Themes: light/dark preference module`.

### Task 2: Theme palettes, Appearance settings, green sync dot

**Files:**
- Modify: `src/styles.css` (the `:root` tokens at the top, the dark block at about line 39, the `.tok-*` dark block at about line 1085, `.sync-dot` at about line 2670)
- Modify: `src/views/Settings.tsx` (a new `AppearanceSettings` section placed just before `<WritingSettings />`)

**Interfaces:**
- Consumes: everything Task 1 produces.
- Produces: the CSS token `--sync-ok` and the attribute selectors `:root[data-theme="…"]` and `:root[data-scheme="dark"]`.

- [ ] **Step 1: Restructure the tokens.** The `:root` block keeps the shared non-colour tokens (fonts, widths, `--safe-top`). Paper's colours go under `:root, :root[data-theme="paper"]`, with today's light values unchanged. Delete the `@media (prefers-color-scheme: dark)` block and define `graphite`, `midnight` and `white` the same way. Each theme defines **every** colour token (`--bg --bg-elev --bg-sunk --fg --fg-muted --fg-faint --border --accent --accent-soft --link --selection --mark --code-bg --danger --heat-0…4 --sync-ok`) plus `color-scheme`. Starting values below. Keep fg / muted / faint at ≥ 12:1 / ≥ 4.5:1 / ≥ 2.8:1 against `--bg`, and adjust if a value misses.

  | token | paper (add only) | white | graphite | midnight |
  |---|---|---|---|---|
  | bg / elev / sunk | — | #ffffff / #ffffff / #f6f7f8 | #18181b / #202024 / #121214 | #161b26 / #1c2230 / #11151d |
  | fg / muted / faint | — | #1f2328 / #59636e / #8c959f | #ececed / #a1a1aa / #71717a | #e2e6ee / #9aa4b5 / #687286 |
  | border | — | #e4e6e9 | #2e2e33 | #2a3242 |
  | accent / accent-soft | — | #2f6fb7 / #e3eefa | #e0a458 / #3a2f22 | #6cb6ff / #1f3350 |
  | link / selection / mark | — | #0969da / #cfe2fb / #fff1b3 | #8cbce0 / #3f3f46 / #5a4523 | #79c0ff / #264f78 / #5a4a1f |
  | code-bg / danger | — | #f3f4f6 / #cf222e | #232327 / #f07167 | #1e2533 / #f47067 |
  | heat-0…4 | — | #e8edf3 #a8c7ea #6fa2dc #3a78c2 #1d4f8f | today's dark ramp | #222a38 #1f4a7a #2f6fb0 #4f95d8 #8cc4f5 |
  | sync-ok | #7fb685 | #5fae72 | #7ccf8f | #7ccf8f |

  Graphite is today's dark palette with the surfaces made neutral. Its accent stays amber.
- [ ] **Step 2: Change the `.tok-*` dark block** from `@media (prefers-color-scheme: dark)` to `:root[data-scheme="dark"] .tok-…`. Then `grep -n "prefers-color-scheme" src/styles.css` should print nothing.
- [ ] **Step 3: Colour the synced dot:** add `.sync-dot.is-idle span { background: var(--sync-ok); }` and update the comment above it ("green when synced").
- [ ] **Step 4: Add `AppearanceSettings` to Settings.** It's a `settings-section` titled "Appearance", built like `WritingSettings`, with three `<Row>`s each holding a `settings-select`:
  - "Mode": hint "System follows your computer's light or dark setting."
  - "Light theme"
  - "Dark theme"

  On change, call `saveTheme(next)`.
- [ ] **Step 5: Verify in the preview.** Run `preview_start {name: "ui"}` and open Settings. Set each Light/Dark theme in turn with Mode = Light, then Dark, and check that the page re-colours with no console errors. Then set Mode = System and switch with `resize_window {colorScheme: "dark"}` / `"light"`: the page should follow (Review Focus 4). Run `pnpm typecheck`. Take a screenshot of each theme.
- [ ] **Step 6: Commit** `Themes: Paper, White, Graphite, Midnight; synced dot is green`.

### Task 3: Navigation history module

**Files:**
- Create: `src/lib/navHistory.ts`
- Create: `src/lib/navHistory.test.ts`

**Interfaces:**
- Produces (generic over the place type, so the module stays independent of `App.tsx`):
  - `interface NavHistory<T> { entries: T[]; index: number }`
  - `emptyHistory<T>(first: T): NavHistory<T>`
  - `visit<T>(h: NavHistory<T>, place: T, relate: (prev: T, next: T) => "same" | "replace" | "new", cap = 100): NavHistory<T>`
    - `"same"`: returns `h` unchanged
    - `"replace"`: overwrites `entries[index]`
    - `"new"`: drops entries after `index`, appends, and trims the oldest entries once there are more than `cap`
  - `back<T>(h)` / `forward<T>(h)`: return `{ history, place: T | null }`. `place` is null at either end.
  - `canBack(h)`, `canForward(h)`: boolean

- [ ] **Step 1: Write the failing tests.** Use places as plain strings, with a `relate` that returns `"same"` when the strings are equal, `"replace"` when `next === prev + "!"`, and `"new"` otherwise.
  - `visit` a→b→c, then `back` twice gives b then a; `canBack` is false at a; `forward` gives b
  - `starts a new branch after going back`: a→b→c, back, visit d → entries `[a,b,d]`, `canForward` false
  - `ignores a repeat visit`: a, a → one entry
  - `replaces a new note once it is saved`: a, then `a!` → entries `[a!]` (Review Focus 2)
  - `replaces a chat when it gets its id`: the same rule, with b → b! after a → entries `[a, b!]`
  - `keeps at most cap entries`: 105 visits with cap 100 → length 100, last entry is the latest visit, `index` 99
- [ ] **Step 2: Run** `pnpm vitest run src/lib/navHistory.test.ts`. Expected: FAIL.
- [ ] **Step 3: Implement `src/lib/navHistory.ts`.** Every function is pure and returns a new object.
- [ ] **Step 4: Run the same command.** Expected: PASS.
- [ ] **Step 5: Commit** `Navigation: browser-style history stack`.

### Task 4: Back / forward in the app

**Files:**
- Modify: `src/App.tsx` (`Workspace`: state, `runCommand` at about line 281, keyboard effect at about line 331, topbar at about line 440)
- Modify: `src-tauri/src/menu.rs` (`GO_ITEMS`)
- Modify: `src/views/Settings.tsx` (`SHORTCUTS`)

**Interfaces:**
- Consumes: `navHistory.ts` (Task 3), plus the existing `View`, `openNote`, `newNote` and `setView` in `App.tsx`.
- Produces: the command ids `"go-back"` and `"go-forward"`, handled by `runCommand`.

- [ ] **Step 1: Define the place type in `App.tsx`:** `type Place = { view: View; noteId: string | null }` and `relatePlaces(prev: Place, next: Place): "same" | "replace" | "new"`:
  - both `write`: same if the note ids are equal; replace if `prev.noteId === null` and `next.noteId` is set (a new note was saved); otherwise new
  - both `chat` with the same `key`: replace (a new chat got its id), or same if the ids are equal too
  - otherwise: same if `JSON.stringify(prev.view) === JSON.stringify(next.view)`, else new
- [ ] **Step 2: Record visits.** Keep `const [nav, setNav] = useState(() => emptyHistory<Place>({ view: { kind: "write" }, noteId: null }))` and a `restoring = useRef(false)`. An effect on `[view, currentId]` calls `setNav(h => visit(h, { view, noteId: view.kind === "write" ? currentId : null }, relatePlaces))`, unless `restoring.current` is set, in which case it clears the flag and does nothing else.
- [ ] **Step 3: Add `goTo(place: Place)`.** It sets `restoring.current = true`, then:
  - `write` with a note id: run `openNote`'s body
  - `write` with no note id: `newNote()`
  - anything else: `setView(place.view)`

  Add `runCommand` cases `"go-back"` / `"go-forward"`. Each calls `back`/`forward`, stores the new history, and calls `goTo` when the place is non-null.
- [ ] **Step 4: Handle a missing note** (Review Focus 3). If the note no longer exists when it is restored, `NotePane` fails to load it. Check what `NotePane` does with a missing id: if it shows an error or throws, have `goTo` first run `api.getNote(id).catch(() => null)` and fall back to `newNote()` when that returns null.
- [ ] **Step 5: Add the inputs.**
  - Top bar: right after the sidebar toggle, add two `icon-btn`s with `ChevronLeft` / `ChevronRight` (size 16), titled "Back (⌘[)" / "Forward (⌘])", `disabled` when `!canBack(nav)` / `!canForward(nav)`. Add CSS: `.icon-btn:disabled { opacity: 0.35; pointer-events: none; }`.
  - Keyboard: a **capture-phase** `keydown` listener (like the ⌘L one at about line 356). Mod + `[` / `]` without Shift/Alt calls `preventDefault()`, `stopPropagation()` and `runCommand("go-back" | "go-forward")`. Capture phase is what keeps CodeMirror's outdent from running.
  - Mouse: a `mouseup` listener where `e.button === 3` goes back and `4` goes forward.
  - Native menu: `("go-back", "Back", "CmdOrCtrl+[")` and `("go-forward", "Forward", "CmdOrCtrl+]")` at the top of `GO_ITEMS`.
  - Settings `SHORTCUTS`: add `` [`${mod} [`, "Back"] `` and `` [`${mod} ]`, "Forward"] ``.
- [ ] **Step 6: Verify.**
  - Run `pnpm typecheck && pnpm test && cargo check -p noughtpad-app`.
  - In the preview, go Write (open a note) → All notes → open another note → Settings → Chat. Press Back four times and check that each step lands where it should, including the right note, then Forward once.
  - Click into the editor on an indented list line and press ⌘[: the app should go Back and the line should keep its indent (Review Focus 1).
- [ ] **Step 7: Commit** `Navigation: back/forward across the app (⌘[ ⌘], top bar, menu)`.

### Task 5: Note top: title first, one faint details row

**Files:**
- Modify: `src/components/NotePane.tsx` (the header/title block at about lines 200–225)
- Modify: `src/components/MetaStrip.tsx`
- Modify: `src/styles.css` (`.main` padding about line 171, `.chat-page` min-height about line 2335, `.note-header*`, `.meta-*` about lines 560–650, `.note-title*` about lines 712–743)

**Interfaces:**
- Consumes: the existing `MetaStrip` props (unchanged) and `openableUrl`.

- [ ] **Step 1: Reorder by CSS, not DOM, so untitled notes keep today's order and the existing `.note-header:hover + .note-title.no-title` rule still works.** `.note-pane { display: flex; flex-direction: column; }` and `.note-title.has-title { order: -1; margin-bottom: 4px; }`.
- [ ] **Step 2: Make `MetaStrip` one row.** Render a single `meta-strip` containing:
  - the tag icon and chips, then `LabelInput`
  - when a source exists, a `.meta-source` group: `BookOpen` icon, the title input, a URL input, and the ↗ button. The title input sizes to its content (`field-sizing: content`, with a `size` fallback). Its placeholder is the URL's hostname when there's a URL but no title (Review Focus 5). The URL input is hidden unless `.meta-source:focus-within`. The ↗ button's `title` shows the full URL.
  - when no source exists, the "Add source" button, as today

  Remove the second `meta-strip` element and update the component's doc comment.
- [ ] **Step 3: Restyle the header.**
  - The header (meta row + date + save-dot + trash) is a single line with `flex-wrap: nowrap` on desktop; the meta row takes `flex: 1; min-width: 0`.
  - At rest: `.meta-filled` and `.note-date` sit at 0.55 opacity. The save-dot and trash stay hidden in `chrome-auto` until the header is hovered.
  - While typing: `.is-typing .meta-filled:not(:focus-within)` and the date drop to 0.2.
  - When the top of the note is hovered or focused, everything is at full strength, as today.
  - Delete the now-unused `.meta-source` full-width rules (`order: 1; flex: 1 0 100%`).
- [ ] **Step 4: Change the top padding.** `.main` goes to `padding: calc(44px + var(--safe-top)) 24px 120px`, and `.chat-page` min-height to `calc(100vh - 44px - var(--safe-top))`. The phone media query sets its own padding and is left alone.
- [ ] **Step 5: Verify in the preview** with the mock backend (light and Graphite):
  - A note with a title, tags and a source shows the title at the top, one faint row under it, then the body. Measure the first body line's `getBoundingClientRect().top` before and after the change: it should be at least 120px higher.
  - A note with only a URL source shows the hostname as the placeholder. Clicking it reveals the URL field, and ↗ opens the link.
  - A note with no title looks the same as before: row first, title hidden until hover.
  - Typing fades the row. Hovering brings back the save-dot and trash.

  Run `pnpm typecheck && pnpm test`. Take screenshots before and after.
- [ ] **Step 6: Commit** `Note top: title first, one quiet details row, less top padding`.
