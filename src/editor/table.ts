// Tables you edit in place, like a rich-text editor, while the note stays
// plain Markdown. The widget shows the table; typing in a cell rewrites the
// table's Markdown underneath (one undo step per edit, the usual history).
// Hover shows + buttons for a row and a column, right-click in the app gives
// Row and Column menus, and </> shows the Markdown itself.

import { redo, undo } from "@codemirror/commands";
import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import { type EditorState, StateEffect, StateField } from "@codemirror/state";
import { type EditorView, WidgetType } from "@codemirror/view";
import { loadMarkdownPipeline } from "./clipboard";
import { linkOpener } from "./livePreview";
import { isTauri } from "../lib/api";
import { caretRect, ICONS, toolBar, toolButton } from "./widgetTools";

// ---------- model ------------------------------------------------------------

export type Align = "left" | "center" | "right" | null;

export interface Table {
  /** Row 0 is the header. Cells hold their Markdown, pipes unescaped. */
  rows: string[][];
  aligns: Align[];
}

/** Split one table line into cells, honouring `\|`. Returned cells are unescaped. */
export function splitRow(line: string): string[] {
  let s = line.trim();
  if (s.startsWith("|")) s = s.slice(1);
  if (s.endsWith("|") && !s.endsWith("\\|")) s = s.slice(0, -1);
  const cells: string[] = [];
  let cur = "";
  for (let i = 0; i < s.length; i++) {
    if (s[i] === "\\" && s[i + 1] === "|") {
      cur += "|";
      i++;
    } else if (s[i] === "|") {
      cells.push(cur.trim());
      cur = "";
    } else cur += s[i];
  }
  cells.push(cur.trim());
  return cells;
}

export function parseTable(md: string): Table {
  const lines = md.split("\n").filter((l) => l.trim() !== "");
  const header = splitRow(lines[0] ?? "");
  const n = header.length;
  const aligns: Align[] = splitRow(lines[1] ?? "").map((d) => {
    const l = d.startsWith(":");
    const r = d.endsWith(":");
    return l && r ? "center" : r ? "right" : l ? "left" : null;
  });
  const fit = (cells: string[]) => Array.from({ length: n }, (_, i) => cells[i] ?? "");
  return { rows: [header, ...lines.slice(2).map((l) => fit(splitRow(l)))], aligns: fit(aligns as string[]) as Align[] };
}

const escapeCell = (s: string) => s.replace(/\r?\n/g, " ").replace(/\|/g, "\\|");

export function serializeTable(t: Table): string {
  const row = (cells: string[]) => `| ${cells.map(escapeCell).join(" | ")} |`;
  const delim = t.aligns.map((a) => (a === "center" ? ":---:" : a === "right" ? "---:" : a === "left" ? ":---" : "---"));
  return [row(t.rows[0]), `| ${delim.join(" | ")} |`, ...t.rows.slice(1).map(row)].join("\n");
}

const clone = (t: Table): Table => ({ rows: t.rows.map((r) => [...r]), aligns: [...t.aligns] });
const blankRow = (t: Table) => t.aligns.map(() => "");

/** Table edits. Row 0 is the header: rows insert, move and delete only below it. */
export const ops = {
  insertRow(t: Table, at: number): Table {
    const c = clone(t);
    c.rows.splice(Math.max(1, at), 0, blankRow(t));
    return c;
  },
  deleteRow(t: Table, r: number): Table {
    if (r < 1) return t;
    const c = clone(t);
    c.rows.splice(r, 1);
    return c;
  },
  duplicateRow(t: Table, r: number): Table {
    const c = clone(t);
    c.rows.splice(Math.max(1, r + 1), 0, [...t.rows[r]]);
    return c;
  },
  moveRow(t: Table, r: number, by: -1 | 1): Table {
    const to = r + by;
    if (r < 1 || to < 1 || to >= t.rows.length) return t;
    const c = clone(t);
    [c.rows[r], c.rows[to]] = [c.rows[to], c.rows[r]];
    return c;
  },
  insertCol(t: Table, at: number): Table {
    const c = clone(t);
    c.rows.forEach((row) => row.splice(at, 0, ""));
    c.aligns.splice(at, 0, null);
    return c;
  },
  deleteCol(t: Table, col: number): Table {
    if (t.aligns.length <= 1) return t;
    const c = clone(t);
    c.rows.forEach((row) => row.splice(col, 1));
    c.aligns.splice(col, 1);
    return c;
  },
  duplicateCol(t: Table, col: number): Table {
    const c = clone(t);
    c.rows.forEach((row) => row.splice(col + 1, 0, row[col]));
    c.aligns.splice(col + 1, 0, t.aligns[col]);
    return c;
  },
  moveCol(t: Table, col: number, by: -1 | 1): Table {
    const to = col + by;
    if (to < 0 || to >= t.aligns.length) return t;
    const c = clone(t);
    const swap = <T>(a: T[]) => ([a[col], a[to]] = [a[to], a[col]]);
    c.rows.forEach(swap);
    swap(c.aligns);
    return c;
  },
  align(t: Table, col: number, a: Align): Table {
    const c = clone(t);
    c.aligns[col] = a;
    return c;
  },
  sort(t: Table, col: number, dir: 1 | -1): Table {
    const c = clone(t);
    const coll = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });
    c.rows = [c.rows[0], ...c.rows.slice(1).sort((a, b) => dir * coll.compare(a[col], b[col]))];
    return c;
  },
};

// ---------- finding a table in the document ------------------------------------

/** The full lines of the table at `pos`, or null. */
export function tableRangeAt(state: EditorState, pos: number): { from: number; to: number } | null {
  const tree = ensureSyntaxTree(state, Math.min(state.doc.length, pos + 1), 50) ?? syntaxTree(state);
  for (let n: ReturnType<typeof tree.resolveInner> | null = tree.resolveInner(pos, 1); n; n = n.parent) {
    if (n.name === "Table") return { from: state.doc.lineAt(n.from).from, to: state.doc.lineAt(n.to).to };
  }
  return null;
}

/** Show this table's Markdown (its start position) until the cursor leaves it. */
export const showTableSource = StateEffect.define<number | null>();

export const tableSource = StateField.define<number | null>({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(showTableSource)) return e.value;
    if (value === null) return null;
    const pos = tr.changes.mapPos(value);
    const range = tableRangeAt(tr.state, pos);
    const head = tr.state.selection.main.head;
    if (!range || head < range.from || head > range.to) return null;
    return range.from;
  },
});

// ---------- the widget ------------------------------------------------------------

interface Live {
  code: string;
  table: Table;
  render(): void;
  /** Put the cursor in cell [r, c] with its text selected, so typing replaces it. */
  select?(r: number, c: number): void;
}

type Item =
  | { text: string; action?: () => void; enabled?: boolean; items?: Item[] }
  | { item: "Separator" | "Cut" | "Copy" | "Paste" | "SelectAll" };

const SEP: Item = { item: "Separator" };
const RICH = /[*_`$~[<\\]/;

export class TableWidget extends WidgetType {
  constructor(readonly code: string) {
    super();
  }

  eq(other: TableWidget) {
    return other.code === this.code;
  }

  toDOM(view: EditorView): HTMLElement {
    const dom = document.createElement("div");
    dom.className = "cm-table-widget";
    const scroll = document.createElement("div");
    scroll.className = "cm-table-scroll";
    const tableEl = document.createElement("table");
    scroll.append(tableEl);
    dom.append(scroll);

    const live: Live = { code: this.code, table: parseTable(this.code), render: () => {} };
    (dom as HTMLElement & { npTable?: Live }).npTable = live;

    // Where the table is now (the widget covers exactly its lines).
    const range = () => {
      const from = view.posAtDOM(dom);
      return { from, to: from + live.code.length };
    };

    /** Write a new table into the note. Focus goes to cell [r, c] if given. */
    const commit = (next: Table, focus?: [number, number], typing = false) => {
      const { from } = range();
      const old = live.code;
      const code = serializeTable(next);
      live.table = next;
      live.code = code;
      if (!typing) live.render();
      // Only the characters that changed, so typing in a cell undoes like any typing,
      // while row and column changes are steps of their own.
      const d = diff(old, code);
      view.dispatch({
        changes: { from: from + d.from, to: from + d.to, insert: d.insert },
        userEvent: typing ? "input.type" : "input.table",
      });
      if (focus) focusCell(focus[0], focus[1]);
    };

    const cellAt = (r: number, c: number) => tableEl.querySelector<HTMLElement>(`[data-r="${r}"][data-c="${c}"]`);
    const focusCell = (r: number, c: number, atEnd = true) => {
      const cell = cellAt(r, c);
      if (!cell) return;
      cell.focus();
      const sel = window.getSelection();
      if (sel) {
        const rng = document.createRange();
        rng.selectNodeContents(cell);
        rng.collapse(!atEnd);
        sel.removeAllRanges();
        sel.addRange(rng);
      }
    };
    live.select = (r, c) => {
      const cell = cellAt(r, c);
      if (!cell) return;
      cell.focus();
      const sel = window.getSelection();
      const rng = document.createRange();
      rng.selectNodeContents(cell);
      sel?.removeAllRanges();
      sel?.addRange(rng);
    };
    const leave = (after: boolean) => {
      const { from, to } = range();
      const doc = view.state.doc;
      if (!after) {
        view.dispatch({ selection: { anchor: Math.max(0, from - 1) } });
      } else {
        // A line typed right under a table joins it as a row, so land below a blank line.
        const tableEnd = doc.lineAt(to);
        const next = tableEnd.number < doc.lines ? doc.line(tableEnd.number + 1) : null;
        const afterNext = next && next.number < doc.lines ? doc.line(next.number + 1) : null;
        if (next && next.text.trim() === "" && afterNext && afterNext.text.trim() === "") {
          view.dispatch({ selection: { anchor: afterNext.from } });
        } else if (next && next.text.trim() === "") {
          view.dispatch({ changes: { from: next.to, insert: "\n" }, selection: { anchor: next.to + 1 } });
        } else {
          view.dispatch({ changes: { from: to, insert: "\n\n" }, selection: { anchor: to + 2 } });
        }
      }
      view.focus();
    };

    const showCell = (cell: HTMLElement, src: string, editing: boolean) => {
      cell.dataset.src = src;
      if (editing || !RICH.test(src)) {
        cell.textContent = src;
        return;
      }
      cell.textContent = src;
      void loadMarkdownPipeline().then((p) => {
        if (document.activeElement === cell || cell.dataset.src !== src) return;
        cell.innerHTML = p.untrustedMarkdownToHtml(src).replace(/^<p>|<\/p>\s*$/g, "");
      });
    };

    live.render = () => {
      const t = live.table;
      const focused = tableEl.contains(document.activeElement) ? (document.activeElement as HTMLElement) : null;
      const keep = focused ? [Number(focused.dataset.r), Number(focused.dataset.c)] : null;
      tableEl.textContent = "";
      const thead = tableEl.createTHead();
      const tbody = tableEl.createTBody();
      t.rows.forEach((row, r) => {
        const tr = (r === 0 ? thead : tbody).insertRow();
        row.forEach((src, c) => {
          const cell = document.createElement(r === 0 ? "th" : "td");
          cell.contentEditable = "true";
          cell.spellcheck = true;
          cell.dataset.r = String(r);
          cell.dataset.c = String(c);
          if (t.aligns[c]) cell.style.textAlign = t.aligns[c]!;
          showCell(cell, src, false);
          tr.append(cell);
        });
      });
      if (keep) focusCell(keep[0], keep[1]);
    };
    live.render();

    // Typing in a cell.
    tableEl.addEventListener("focusin", (e) => {
      const cell = e.target as HTMLElement;
      if (cell.dataset.src !== undefined && cell.textContent !== cell.dataset.src) {
        showCell(cell, cell.dataset.src, true);
        focusCell(Number(cell.dataset.r), Number(cell.dataset.c));
      }
    });
    tableEl.addEventListener("focusout", (e) => {
      const cell = e.target as HTMLElement;
      if (cell.dataset.src !== undefined) showCell(cell, cell.dataset.src, false);
    });
    tableEl.addEventListener("input", (e) => {
      const cell = e.target as HTMLElement;
      const r = Number(cell.dataset.r);
      const c = Number(cell.dataset.c);
      const text = (cell.textContent ?? "").replace(/ /g, " ");
      cell.dataset.src = text;
      const next = { rows: live.table.rows.map((row) => [...row]), aligns: live.table.aligns };
      next.rows[r][c] = text;
      commit(next, undefined, true);
    });
    tableEl.addEventListener("paste", (e) => {
      e.preventDefault();
      const text = e.clipboardData?.getData("text/plain") ?? "";
      document.execCommand("insertText", false, text.replace(/\s*\n\s*/g, " "));
    });
    tableEl.addEventListener("keydown", (e) => {
      const cell = e.target as HTMLElement;
      if (cell.dataset.r === undefined) return;
      const r = Number(cell.dataset.r);
      const c = Number(cell.dataset.c);
      const t = live.table;
      const lastRow = t.rows.length - 1;
      const lastCol = t.aligns.length - 1;
      const mod = e.metaKey || e.ctrlKey;
      const stop = () => {
        e.preventDefault();
        e.stopPropagation();
      };
      if (mod && e.key.toLowerCase() === "z") {
        stop();
        (e.shiftKey ? redo : undo)(view);
        return;
      }
      if (mod && (e.key.toLowerCase() === "b" || e.key.toLowerCase() === "i")) {
        stop();
        const mark = e.key.toLowerCase() === "b" ? "**" : "*";
        const sel = window.getSelection()?.toString() ?? "";
        document.execCommand("insertText", false, `${mark}${sel}${mark}`);
        return;
      }
      if (e.key === "Enter" && !mod) {
        stop();
        if (r < lastRow) focusCell(r + 1, c);
        else if (r > 1 && t.rows[r].every((x) => x.trim() === "")) {
          // Enter on an empty last row ends the table, like an empty list item.
          commit(ops.deleteRow(t, r));
          leave(true);
        } else commit(ops.insertRow(t, r + 1), [r + 1, c]);
      } else if (e.key === "Tab") {
        stop();
        // Tab selects the next cell's text, as in a spreadsheet or Word, so typing replaces it.
        const select = (rr: number, cc: number) => live.select?.(rr, cc);
        if (e.shiftKey) {
          if (c > 0) select(r, c - 1);
          else if (r > 0) select(r - 1, lastCol);
        } else if (c < lastCol) select(r, c + 1);
        else if (r < lastRow) select(r + 1, 0);
        else commit(ops.insertRow(t, r + 1), [r + 1, 0]);
      } else if (e.key === "ArrowUp") {
        stop();
        if (r > 0) focusCell(r - 1, c);
        else leave(false);
      } else if (e.key === "ArrowDown") {
        stop();
        if (r < lastRow) focusCell(r + 1, c);
        else leave(true);
      } else if (e.key === "Escape") {
        stop();
        leave(true);
      }
    });
    // ⌘-click a rendered link opens it.
    tableEl.addEventListener("mousedown", (e) => {
      const a = (e.target as HTMLElement).closest("a");
      if (a && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        const href = a.getAttribute("href");
        if (href) view.state.facet(linkOpener)(href);
      }
    });

    // Hover controls.
    const addRow = button("cm-table-add-row", "+", "Add row", () => commit(ops.insertRow(live.table, live.table.rows.length), [live.table.rows.length, 0]));
    const addCol = button("cm-table-add-col", "+", "Add column", () => {
      const n = live.table.aligns.length;
      commit(ops.insertCol(live.table, n), [0, n]);
    });
    const tools = toolBar([
      toolButton("Edit as Markdown", ICONS.code, () => {
        const { from } = range();
        view.dispatch({ effects: showTableSource.of(from), selection: { anchor: from } });
        view.focus();
      }),
    ]);
    dom.append(addRow, addCol, tools);

    // Right-click: Row and Column menus (in the app; the browser keeps its own).
    if (isTauri) {
      tableEl.addEventListener("contextmenu", (e) => {
        const cell = (e.target as HTMLElement).closest<HTMLElement>("[data-r]");
        if (!cell) return;
        e.preventDefault();
        const r = Number(cell.dataset.r);
        const c = Number(cell.dataset.c);
        void popup(tableMenu(live.table, r, c, commit, () => leaveAndDelete()));
      });
    }
    const leaveAndDelete = () => {
      const { from, to } = range();
      view.dispatch({ changes: { from, to: Math.min(to + 1, view.state.doc.length) }, selection: { anchor: from } });
      view.focus();
    };

    return dom;
  }

  updateDOM(dom: HTMLElement): boolean {
    const live = (dom as HTMLElement & { npTable?: Live }).npTable;
    if (!live) return false;
    if (live.code !== this.code) {
      // Changed from outside the widget (undo, sync, an edit in the source): redraw.
      live.code = this.code;
      live.table = parseTable(this.code);
      live.render();
    }
    return true;
  }

  ignoreEvent() {
    return true;
  }

  coordsAt(dom: HTMLElement, pos: number) {
    return caretRect(dom, pos);
  }
}

/** The smallest single replacement turning `a` into `b`. */
export function diff(a: string, b: string): { from: number; to: number; insert: string } {
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  return { from: start, to: endA, insert: b.slice(start, endB) };
}

function button(cls: string, text: string, title: string, onClick: () => void) {
  const b = document.createElement("button");
  b.className = cls;
  b.textContent = text;
  b.title = title;
  b.setAttribute("aria-label", title);
  b.tabIndex = -1;
  b.addEventListener("mousedown", (e) => e.preventDefault());
  b.addEventListener("click", (e) => {
    e.preventDefault();
    onClick();
  });
  return b;
}

export function tableMenu(
  t: Table,
  r: number,
  c: number,
  commit: (next: Table, focus?: [number, number]) => void,
  deleteTable: () => void,
): Item[] {
  const body = r > 0;
  const lastRow = t.rows.length - 1;
  const lastCol = t.aligns.length - 1;
  const item = (text: string, run: () => void, enabled = true): Item => ({ text, action: run, enabled });
  return [
    {
      text: "Row",
      items: [
        item("Add row before", () => commit(ops.insertRow(t, r), [r, c]), body),
        item("Add row after", () => commit(ops.insertRow(t, r + 1), [r + 1, c])),
        SEP,
        item("Move row up", () => commit(ops.moveRow(t, r, -1), [r - 1, c]), r > 1),
        item("Move row down", () => commit(ops.moveRow(t, r, 1), [r + 1, c]), body && r < lastRow),
        SEP,
        item("Duplicate row", () => commit(ops.duplicateRow(t, r), [Math.max(1, r + 1), c])),
        item("Delete row", () => commit(ops.deleteRow(t, r), [Math.min(r, lastRow - 1), c]), body),
      ],
    },
    {
      text: "Column",
      items: [
        item("Add column before", () => commit(ops.insertCol(t, c), [r, c])),
        item("Add column after", () => commit(ops.insertCol(t, c + 1), [r, c + 1])),
        SEP,
        item("Move column left", () => commit(ops.moveCol(t, c, -1), [r, c - 1]), c > 0),
        item("Move column right", () => commit(ops.moveCol(t, c, 1), [r, c + 1]), c < lastCol),
        SEP,
        item("Align left", () => commit(ops.align(t, c, "left"))),
        item("Align centre", () => commit(ops.align(t, c, "center"))),
        item("Align right", () => commit(ops.align(t, c, "right"))),
        SEP,
        item("Duplicate column", () => commit(ops.duplicateCol(t, c), [r, c + 1])),
        item("Delete column", () => commit(ops.deleteCol(t, c), [r, Math.min(c, lastCol - 1)]), lastCol > 0),
      ],
    },
    SEP,
    item("Sort by column (A to Z)", () => commit(ops.sort(t, c, 1)), lastRow > 1),
    item("Sort by column (Z to A)", () => commit(ops.sort(t, c, -1)), lastRow > 1),
    SEP,
    item("Delete table", deleteTable),
    SEP,
    { item: "Cut" },
    { item: "Copy" },
    { item: "Paste" },
  ];
}

async function popup(items: Item[]) {
  const { Menu, MenuItem, PredefinedMenuItem, Submenu } = await import("@tauri-apps/api/menu");
  const make = async (i: Item): Promise<unknown> => {
    if ("item" in i) return PredefinedMenuItem.new({ item: i.item });
    if (i.items) return Submenu.new({ text: i.text, items: (await Promise.all(i.items.map(make))) as never[] });
    return MenuItem.new({ text: i.text, enabled: i.enabled ?? true, action: i.action });
  };
  const menu = await Menu.new({ items: (await Promise.all(items.map(make))) as never[] });
  await menu.popup();
}

/** Put the cursor in a cell of the table starting at `pos`, with its text selected (a new table). */
export function selectTableCell(view: EditorView, pos: number, r: number, c: number) {
  for (const dom of Array.from(view.contentDOM.querySelectorAll<HTMLElement>(".cm-table-widget"))) {
    if (view.posAtDOM(dom) !== pos) continue;
    (dom as HTMLElement & { npTable?: Live }).npTable?.select?.(r, c);
    return;
  }
}
