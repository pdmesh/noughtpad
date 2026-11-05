// The slash menu: type "/" at the start of a line (or after a space) to insert
// a heading, list, table, image, diagram… Keep typing to filter; Enter picks.

import { autocompletion, type Completion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { COMMANDS, type EditorCommand, shortcutLabel } from "./commands";

const SECTION_RANK = { Paragraph: 0, Insert: 1, Format: 2 } as const;

/** Slash commands matching `query`, best first. */
export function matchCommands(query: string): EditorCommand[] {
  const q = query.trim().toLowerCase();
  const all = COMMANDS.filter((c) => c.slash);
  if (!q) return all;
  const scored: Array<[number, EditorCommand]> = [];
  for (const c of all) {
    const label = c.label.toLowerCase();
    const words = `${label} ${c.keywords ?? ""}`.toLowerCase().split(/\s+/);
    let score = -1;
    if (label.startsWith(q)) score = 3;
    else if (words.some((w) => w === q)) score = 2;
    else if (words.some((w) => w.startsWith(q))) score = 1;
    else if (label.includes(q)) score = 0;
    if (score >= 0) scored.push([score, c]);
  }
  return scored.sort((a, b) => b[0] - a[0]).map(([, c]) => c);
}

function inCode(ctx: CompletionContext): boolean {
  for (let n: ReturnType<typeof syntaxTree>["topNode"] | null = syntaxTree(ctx.state).resolveInner(ctx.pos, -1); n; n = n.parent) {
    if (n.name === "FencedCode" || n.name === "CodeBlock" || n.name === "InlineCode") return true;
  }
  return false;
}

function slashSource(ctx: CompletionContext): CompletionResult | null {
  const m = ctx.matchBefore(/\/[\w-]*$/);
  if (!m) return null;
  const before = m.from > 0 ? ctx.state.sliceDoc(m.from - 1, m.from) : "";
  if (before && !/\s/.test(before)) return null; // a/b, URLs, dates
  if (inCode(ctx)) return null;
  const matches = matchCommands(m.text.slice(1));
  if (!matches.length) return null;
  const options: Completion[] = matches.map((c, i) => ({
    label: c.label,
    detail: c.key ? shortcutLabel(c.key) : undefined,
    section: { name: c.section, rank: SECTION_RANK[c.section] },
    boost: -i,
    apply: (view, _completion, from, to) => {
      view.dispatch({ changes: { from, to, insert: "" } });
      c.run(view);
    },
  }));
  // Results are already filtered and ordered; recompute on each keystroke.
  return { from: m.from, options, filter: false };
}

export const slashMenu: Extension = autocompletion({
  override: [slashSource],
  icons: false,
  closeOnBlur: true,
  maxRenderedOptions: 40,
  tooltipClass: () => "np-slash",
});
