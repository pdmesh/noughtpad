# Details Line Revision Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** One-line details line with tag/source popovers, placed under the title or a first-line heading, and no hidden title space.

**Architecture:** Pure layout helpers in `src/lib/metaLayout.ts` (tested). A small `Popover` component. `MetaStrip` becomes one non-wrapping row. `NotePane` decides where the header goes: above the editor, or portalled into a CodeMirror block widget after line 1 (`src/editor/headerSlot.ts`).

**Spec:** `docs/superpowers/specs/2026-11-14-desktop-design-refresh-design.md` § Revision

## Global Constraints
- Branch `desktop-design-refresh`. Desktop only; phone keeps working (touch: click opens popovers).
- No change to how notes are saved; metadata stays in front matter.

## Review Focus
1. Typing, selecting and undo in the editor around the in-editor details line (Enter at end of the heading, deleting line 1, ⌘A).
2. Clicks and typing inside the details line when it lives in the editor must not move the editor cursor or type into the note.
3. Many tags or a very long source name must never wrap the line.
4. Popovers close on Escape / outside click and don't trap focus.
5. The heading → plain text change (and back) moves the details line without losing popover state badly.

---

### Task 6: Layout helpers
**Files:** Create `src/lib/metaLayout.ts`, `src/lib/metaLayout.test.ts`
**Produces:** `headingLine(doc: string): { level: number; to: number } | null` (first line is an ATX heading `#{1,6} text`; `to` = end offset of line 1); `fitCount(widths: number[], available: number, gap: number, reserve: number): number` (how many leading items fit in one line, leaving `reserve` px for a "+N" chip when not all fit).
- [ ] Tests: `headingLine("# Hi\nbody")` → `{level:1,to:4}`; `headingLine("Hi\n# x")` → null; `headingLine("#tag")` → null; `headingLine("")` → null. `fitCount([50,50,50],200,6,30)` → 3; `fitCount([50,50,50],120,6,30)` → 1 (50+6+50=106 > 120-30); `fitCount([300],100,6,30)` → 0.
- [ ] RED, implement, GREEN, commit.

### Task 7: One-line details with popovers
**Files:** Create `src/components/Popover.tsx`; modify `src/components/MetaStrip.tsx`, `src/components/NotePane.tsx`, `src/styles.css`
**Produces:** `Popover({ anchor: ReactNode; children; peek?: ReactNode; open; onOpenChange; label })`: hover shows `peek` (or children read-only), click/Enter opens; Escape/outside click closes. `MetaStrip` gains `onAddTitle?: () => void` (shows "+ Title" when set).
- [ ] MetaStrip: tags row (`flex-wrap: nowrap; overflow: hidden`), measure chip widths in `useLayoutEffect` + `ResizeObserver`, show `fitCount` chips then "+N"; tags popover holds all chips (removable) + `LabelInput`. Source shows name (or hostname) + ↗; source popover holds title + URL inputs. "+ tag", "+ Add source", "+ Title" show on hover.
- [ ] NotePane: render the title input only when a title is set or "+ Title" was clicked (then focus it); no hidden title space.
- [ ] CSS: `.meta-strip` single line; popover styles from theme tokens (`--bg-elev`, `--border`), z-index above editor.
- [ ] Verify in preview: 8 tags never wrap; "+N" peek on hover, edit on click; source popover; Escape closes. Typecheck + tests. Commit.

### Task 8: Details under a first-line heading
**Files:** Create `src/editor/headerSlot.ts`; modify `src/editor/Editor.tsx` (prop `headerSlot?: HTMLElement`), `src/components/NotePane.tsx`, `src/styles.css`
**Consumes:** `headingLine` (Task 6).
**Produces:** `headerSlotExtension(host: HTMLElement): Extension` — a StateField of a block `Decoration.widget({ side: 1, block: true })` at `headingLine(doc).to` whose widget returns `host` (same element every time, `eq` true, `ignoreEvent` true); `onHeading(cb: (has: boolean) => void)` via an update listener.
- [ ] NotePane: if no title and the doc's first line is a heading, portal the header into the slot host; else render it above the editor.
- [ ] Verify (Review Focus 1, 2, 5) in the preview with a `# heading` note: details sit under the heading; typing in the tag/source popovers doesn't touch the note; Enter at end of heading moves text below the details line; deleting the `#` moves the details line above the text. Typecheck + tests. Commit.
