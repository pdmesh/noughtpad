// Editing commands shared by the slash menu, the right-click menu and the
// keyboard shortcuts. Each one writes plain Markdown: nothing here creates
// syntax the live preview can't show.

import { EditorSelection, type EditorState, Facet, type Line } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

export type Section = "Format" | "Paragraph" | "Insert";

export interface EditorCommand {
  id: string;
  label: string;
  section: Section;
  /** Extra words the slash menu matches, e.g. "h1 title" for Heading 1. */
  keywords?: string;
  /** CodeMirror key binding, e.g. "Mod-b". */
  key?: string;
  /** The same shortcut as a native menu accelerator, e.g. "CmdOrCtrl+B". */
  accelerator?: string;
  /** Offered in the slash menu (block-level commands). */
  slash?: boolean;
  /** For a tick in the right-click menu. */
  active?: (state: EditorState) => boolean;
  run: (view: EditorView) => boolean;
}

/** Opens a file picker and inserts the chosen images (provided by the editor). */
export const imagePicker = Facet.define<(view: EditorView) => void, ((view: EditorView) => void) | null>({
  combine: (values) => values[0] ?? null,
});

// ---------- inline formatting ------------------------------------------------

/** Wrap each selection in `mark`, or unwrap it if it is already wrapped. */
export function toggleMark(view: EditorView, mark: string): boolean {
  const { state } = view;
  const n = mark.length;
  view.dispatch(
    state.changeByRange((r) => {
      const before = state.sliceDoc(r.from - n, r.from);
      const after = state.sliceDoc(r.to, r.to + n);
      // A single `*` next to `**` is part of bold, not italic.
      const isPartOfLonger = (pos: number) => mark.length === 1 && state.sliceDoc(pos, pos + 1) === mark;
      if (before === mark && after === mark && !isPartOfLonger(r.from - n - 1) && !isPartOfLonger(r.to + n)) {
        return {
          changes: [
            { from: r.from - n, to: r.from },
            { from: r.to, to: r.to + n },
          ],
          range: EditorSelection.range(r.from - n, r.to - n),
        };
      }
      const text = state.sliceDoc(r.from, r.to);
      if (text.length >= 2 * n && text.startsWith(mark) && text.endsWith(mark)) {
        return {
          changes: { from: r.from, to: r.to, insert: text.slice(n, text.length - n) },
          range: EditorSelection.range(r.from, r.to - 2 * n),
        };
      }
      return {
        changes: [
          { from: r.from, insert: mark },
          { from: r.to, insert: mark },
        ],
        range: EditorSelection.range(r.from + n, r.to + n),
      };
    }),
    { userEvent: "input.format", scrollIntoView: true },
  );
  view.focus();
  return true;
}

/** Strip inline Markdown formatting (bold, italic, strikethrough, code, maths) from the selection. */
export function stripInline(text: string): string {
  return text
    .replace(/(\*\*|__|~~)(.+?)\1/g, "$2")
    .replace(/(?<![\w*])([*_])(?!\s)(.+?)(?<!\s)\1(?![\w*])/g, "$2")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\$([^$\n]+)\$/g, "$1");
}

function clearFormatting(view: EditorView): boolean {
  const { state } = view;
  view.dispatch(
    state.changeByRange((r) => {
      if (r.empty) return { range: r };
      const out = stripInline(state.sliceDoc(r.from, r.to));
      return { changes: { from: r.from, to: r.to, insert: out }, range: EditorSelection.range(r.from, r.from + out.length) };
    }),
    { userEvent: "input.format" },
  );
  view.focus();
  return true;
}

const URLISH = /^(https?:\/\/|mailto:)\S+$/i;

function addLink(view: EditorView): boolean {
  const { state } = view;
  view.dispatch(
    state.changeByRange((r) => {
      const text = state.sliceDoc(r.from, r.to);
      if (URLISH.test(text.trim())) {
        // Selected a URL: make it the target and type the text.
        const insert = `[](${text.trim()})`;
        return { changes: { from: r.from, to: r.to, insert }, range: EditorSelection.cursor(r.from + 1) };
      }
      const insert = `[${text}]()`;
      const cursor = text ? r.from + insert.length - 1 : r.from + 1;
      return { changes: { from: r.from, to: r.to, insert }, range: EditorSelection.cursor(cursor) };
    }),
    { userEvent: "input.format" },
  );
  view.focus();
  return true;
}

// ---------- paragraph types --------------------------------------------------

export type Block = "body" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "bullet" | "numbered" | "task" | "quote";

const LINE = /^(\s*)(#{1,6}\s+|>\s?|[-*+]\s+\[[ xX]\]\s+|[-*+]\s+|\d+[.)]\s+)?(.*)$/;

/** What kind of paragraph a line is, and its parts. */
export function parseLine(text: string): { indent: string; block: Block; rest: string } {
  const m = LINE.exec(text)!;
  const [, indent, prefix = "", rest] = m;
  let block: Block = "body";
  if (/^#{1,6}\s/.test(prefix)) block = `h${prefix.trim().length}` as Block;
  else if (prefix.startsWith(">")) block = "quote";
  else if (/\[[ xX]\]/.test(prefix)) block = "task";
  else if (/^[-*+]/.test(prefix)) block = "bullet";
  else if (/^\d/.test(prefix)) block = "numbered";
  return { indent, block, rest };
}

function prefixFor(block: Block, n: number): string {
  if (block === "body") return "";
  if (block === "quote") return "> ";
  if (block === "bullet") return "- ";
  if (block === "task") return "- [ ] ";
  if (block === "numbered") return `${n}. `;
  return `${"#".repeat(Number(block.slice(1)))} `;
}

function selectedLines(state: EditorState): Line[] {
  const seen = new Set<number>();
  const lines: Line[] = [];
  for (const r of state.selection.ranges) {
    for (let pos = r.from; pos <= r.to; ) {
      const line = state.doc.lineAt(pos);
      if (!seen.has(line.number)) {
        seen.add(line.number);
        lines.push(line);
      }
      pos = line.to + 1;
    }
  }
  return lines;
}

export function currentBlock(state: EditorState): Block {
  return parseLine(state.doc.lineAt(state.selection.main.head).text).block;
}

/** Make the selected lines `block`; if they all already are, make them body text. */
export function setBlock(view: EditorView, block: Block): boolean {
  const { state } = view;
  const lines = selectedLines(state);
  const parsed = lines.map((l) => parseLine(l.text));
  const target = block !== "body" && parsed.every((p) => p.block === block) ? "body" : block;
  // Headings and quotes drop the indent of a list item; lists keep it.
  const keepIndent = target === "bullet" || target === "numbered" || target === "task";
  const changes = lines.map((line, i) => {
    const p = parsed[i];
    const insert = (keepIndent ? p.indent : "") + prefixFor(target, i + 1) + p.rest;
    return { from: line.from, to: line.to, insert };
  });
  const set = state.changes(changes);
  const sel = state.selection.main;
  let selection: EditorSelection;
  if (sel.empty) {
    // One line: keep the cursor at the same place in the text after the prefix.
    const line = lines[0];
    const oldPrefix = line.text.length - parsed[0].rest.length;
    const offset = Math.max(0, sel.head - line.from - oldPrefix);
    selection = EditorSelection.single(line.from + changes[0].insert.length - parsed[0].rest.length + offset);
  } else {
    selection = state.selection.map(set);
  }
  view.dispatch({ changes: set, selection, userEvent: "input.format", scrollIntoView: true });
  view.focus();
  return true;
}

// ---------- inserted blocks ----------------------------------------------------

/**
 * Insert a block of Markdown on its own lines. `select` is [from, to] within
 * `text` to select afterwards (a cursor when equal).
 */
export function insertBlock(view: EditorView, text: string, select: [number, number] = [text.length, text.length]): boolean {
  const { state } = view;
  const sel = state.selection.main;
  const line = state.doc.lineAt(sel.head);
  let from: number;
  let to: number;
  let before = "";
  if (line.text.trim() === "") {
    from = line.from;
    to = line.to;
    // Keep a blank line from the paragraph above (a rule right under text would make it a heading).
    if (line.number > 1 && state.doc.line(line.number - 1).text.trim() !== "") before = "\n";
  } else {
    from = to = line.to;
    before = "\n\n";
  }
  const next = line.number < state.doc.lines ? state.doc.line(line.number + 1) : null;
  const after = next && next.text.trim() !== "" ? "\n" : "";
  const insert = before + text + after;
  const base = from + before.length;
  view.dispatch({
    changes: { from, to, insert },
    selection: EditorSelection.range(base + select[0], base + select[1]),
    userEvent: "input.insert",
    scrollIntoView: true,
  });
  view.focus();
  return true;
}

const TABLE = "| Column | Column |\n| --- | --- |\n|  |  |";
const MERMAID = "```mermaid\nflowchart TD\n  A[Start] --> B[Finish]\n```";

// ---------- the list -------------------------------------------------------------

const heading = (level: number): EditorCommand => ({
  id: `h${level}`,
  label: `Heading ${level}`,
  section: "Paragraph",
  keywords: `h${level} title heading ${"#".repeat(level)}`,
  slash: true,
  active: (s) => currentBlock(s) === `h${level}`,
  run: (v) => setBlock(v, `h${level}` as Block),
});

export const COMMANDS: EditorCommand[] = [
  // Format
  { id: "bold", label: "Bold", section: "Format", key: "Mod-b", accelerator: "CmdOrCtrl+B", run: (v) => toggleMark(v, "**") },
  { id: "italic", label: "Italic", section: "Format", key: "Mod-i", accelerator: "CmdOrCtrl+I", run: (v) => toggleMark(v, "*") },
  {
    id: "strike",
    label: "Strikethrough",
    section: "Format",
    key: "Mod-Shift-x",
    accelerator: "CmdOrCtrl+Shift+X",
    run: (v) => toggleMark(v, "~~"),
  },
  { id: "code", label: "Code", section: "Format", key: "Mod-e", accelerator: "CmdOrCtrl+E", run: (v) => toggleMark(v, "`") },
  { id: "math", label: "Maths", section: "Format", run: (v) => toggleMark(v, "$") },
  { id: "link", label: "Link", section: "Format", keywords: "url href", slash: true, run: addLink },
  { id: "clear", label: "Clear formatting", section: "Format", run: clearFormatting },
  // Paragraph
  {
    id: "body",
    label: "Body text",
    section: "Paragraph",
    keywords: "text paragraph plain normal",
    active: (s) => currentBlock(s) === "body",
    run: (v) => setBlock(v, "body"),
  },
  heading(1),
  heading(2),
  heading(3),
  heading(4),
  heading(5),
  heading(6),
  {
    id: "bullet",
    label: "Bullet list",
    section: "Paragraph",
    keywords: "unordered ul - list",
    key: "Mod-Shift-8",
    accelerator: "CmdOrCtrl+Shift+8",
    slash: true,
    active: (s) => currentBlock(s) === "bullet",
    run: (v) => setBlock(v, "bullet"),
  },
  {
    id: "numbered",
    label: "Numbered list",
    section: "Paragraph",
    keywords: "ordered ol 1. list",
    key: "Mod-Shift-7",
    accelerator: "CmdOrCtrl+Shift+7",
    slash: true,
    active: (s) => currentBlock(s) === "numbered",
    run: (v) => setBlock(v, "numbered"),
  },
  {
    id: "task",
    label: "Task list",
    section: "Paragraph",
    keywords: "todo checkbox check [] list",
    key: "Mod-Shift-9",
    accelerator: "CmdOrCtrl+Shift+9",
    slash: true,
    active: (s) => currentBlock(s) === "task",
    run: (v) => setBlock(v, "task"),
  },
  {
    id: "quote",
    label: "Quote",
    section: "Paragraph",
    keywords: "blockquote >",
    slash: true,
    active: (s) => currentBlock(s) === "quote",
    run: (v) => setBlock(v, "quote"),
  },
  // Insert
  { id: "table", label: "Table", section: "Insert", keywords: "grid columns rows", slash: true, run: (v) => insertBlock(v, TABLE, [2, 8]) },
  {
    id: "image",
    label: "Image…",
    section: "Insert",
    keywords: "picture photo screenshot attachment",
    slash: true,
    run: (v) => {
      const pick = v.state.facet(imagePicker);
      if (!pick) return false;
      pick(v);
      return true;
    },
  },
  { id: "hr", label: "Divider", section: "Insert", keywords: "horizontal rule line separator ---", slash: true, run: (v) => insertBlock(v, "---\n") },
  { id: "codeblock", label: "Code block", section: "Insert", keywords: "fence snippet ```", slash: true, run: (v) => insertBlock(v, "```\n\n```", [4, 4]) },
  { id: "mathblock", label: "Maths block", section: "Insert", keywords: "equation latex katex $$", slash: true, run: (v) => insertBlock(v, "$$\n\n$$", [3, 3]) },
  {
    id: "mermaid",
    label: "Diagram",
    section: "Insert",
    keywords: "mermaid flowchart chart graph",
    slash: true,
    run: (v) => insertBlock(v, MERMAID, [MERMAID.indexOf("A["), MERMAID.length - 4]),
  },
];

export const commandById = (id: string) => COMMANDS.find((c) => c.id === id);

/** Mac-style shortcut text for a CodeMirror key, e.g. "Mod-Shift-x" → "⇧⌘X". */
export function shortcutLabel(key: string, mac = /Mac|iPhone|iPad/.test(navigator.platform)): string {
  const parts = key.split("-");
  const k = parts.pop()!.toUpperCase();
  const mods = new Set(parts);
  if (!mac) return [...parts.map((m) => (m === "Mod" ? "Ctrl" : m)), k].join("+");
  return `${mods.has("Ctrl") ? "⌃" : ""}${mods.has("Alt") ? "⌥" : ""}${mods.has("Shift") ? "⇧" : ""}${mods.has("Mod") ? "⌘" : ""}${k}`;
}
