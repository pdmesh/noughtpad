# Desktop design refresh

Short design. Desktop first; mobile gets its own pass later (the note-top change is shared code, so check it there).

## Why

- The first readable line of a note sits about a third of the way down the window, far below where the eye lands (Obsidian puts it right under a thin breadcrumb).
- Moving between Write, notes, chat, settings, journal and ideas has no way back.
- The synced sync-dot is grey, which reads as "something is off".
- One fixed palette pair (warm light, one dark). People want a plain white and a choice of darks.

## Goals / non-goals

- Goals: title and first line high on the page; back/forward across the app; a positive "synced" colour; selectable themes.
- Non-goals: a fixed toolbar or breadcrumb bar; mobile layout changes; custom/user-defined themes.
- Anti-goals: metadata that is hard to reach or edit; chrome that stays visible while writing.

## 1. Note top (combo of three options)

Considered: (A) title first, details below; (B) details on one row; (C) details hidden until hover. Chosen: all three together.

- Order: **title → one details row → body**. The title is already its own `<input>` in `NotePane`, so this is a reorder.
- Details row: tags · source name ↗ … date · save-dot · trash. The long URL no longer gets its own row; it lives in the source's tooltip and the ↗ button.
- At rest the row is faint (~0.55 opacity), empty fields are not shown; while typing it nearly vanishes; on hover/focus of the note top it is full strength and the "+ tag" / "+ source" adders appear.
- Top padding of `.main` drops from ~72px to ~44px (just clears the hover top bar).
- Note with no title: the details row is at the top; the empty title stays hidden until hover, as today.
- Target: the first body line ~130–150px higher than today.

## 2. Back / forward

- ‹ › buttons next to the sidebar toggle in the top bar (shown with the top bar, so on hover in auto-hide mode).
- Shortcuts ⌘[ / ⌘], and mouse buttons 3/4.
- History holds every view change, and each opened note is its own entry (so note → note → settings → back works). Going back restores the view and, for notes, reopens that note.
- Opening something new after going back drops the forward entries (browser rules). Cap the history at a sensible size (builder's call, e.g. 100).

## 3. Sync dot

- Synced: soft pastel green (a token per theme). Syncing (accent, pulsing) and error/signed-out (red) unchanged.

## 4. Themes

Considered: a single theme list vs. a light + dark pair. Chosen: **light + dark pair** (VS Code style), so following the system light/dark mode keeps working.

- Settings → Appearance: **Mode** (System / Light / Dark), **Light theme**, **Dark theme**.
- Themes: **Paper** (today's warm light, default light), **White** (neutral, close to stock macOS), **Graphite** (neutral dark from shadcn neutral/zinc, default dark), **Midnight** (blue-tinted dark in the spirit of VS Code Dark Modern).
- Implemented as token sets on `:root[data-theme=…]` replacing the current `prefers-color-scheme` block; every existing token (incl. heat ramp, code, mark, selection, and the new sync-ok green) is defined per theme. `color-scheme` set per theme.
- Persisted with the other app settings; applied before first paint to avoid a flash.

## Left to the builder

Exact colour values (check contrast for fg/muted/faint against bg), the history cap, how the theme setting is stored, and whether the native window background should follow the theme.

## Revision (same day): one-line details, popovers, heading-first notes

Found in testing: most notes have no title field (2 of 48 sample notes); their first line stands in as the title, so they kept the old "details on top" layout.

Considered for untitled notes: (1) details under the first line inside the editor, (2) details above with the hidden-title space removed, (3) details at the end/margin. Chosen: (1) when the first line is a heading, (2) otherwise.

- Where the details line goes:
  - Title field set: title → details → text.
  - No title, first line is a heading (`#`…): that heading → details → rest of text. The details line lives inside the editor after line 1 and moves if line 1 changes.
  - No title, first line is plain text: details → text, with no hidden title space (reclaimed).
- Details line is **one line, never wraps**: tags (as many as fit, then a "+N" chip) · source name ↗ · date on the right; save-dot and trash on hover; fades while typing as before.
- **Tags popover**: hovering "+N" previews all tags; clicking it (or a tag, or "+ tag" on hover) opens it for editing: remove chips, add with the existing label combobox.
- **Source popover**: clicking the source name (or "+ Add source") opens a popover with the name and URL fields; hovering the name previews the URL. ↗ opens the link.
- **Add a title**: a faint "+ Title" in the details line on hover; clicking shows the title field above, focused.
- Popovers: hover only previews, click/focus opens for editing; Escape or an outside click closes; keyboard reachable.
