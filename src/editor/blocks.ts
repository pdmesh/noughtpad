// Fenced-block renderers: ```mermaid, ```math and $$…$$ blocks render as
// widgets when the cursor is outside them. This is Noughtpad's extension point
// for "block plugins" — a renderer is just { languages, render(code, el) }.

import { ensureSyntaxTree, syntaxTree } from "@codemirror/language";
import { type EditorState, type Range, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, EditorView, WidgetType } from "@codemirror/view";

export interface BlockRenderer {
  /** Fence info strings this renderer handles, e.g. ["mermaid"]. */
  languages: string[];
  render(code: string, el: HTMLElement): void | Promise<void>;
}

const registry = new Map<string, BlockRenderer>();

export function registerBlockRenderer(r: BlockRenderer) {
  for (const lang of r.languages) registry.set(lang.toLowerCase(), r);
}

let mermaidSeq = 0;

registerBlockRenderer({
  languages: ["mermaid"],
  async render(code, el) {
    const { default: mermaid } = await import("mermaid");
    const dark = window.matchMedia?.("(prefers-color-scheme: dark)").matches;
    mermaid.initialize({ startOnLoad: false, theme: dark ? "dark" : "neutral", securityLevel: "strict" });
    try {
      const { svg } = await mermaid.render(`noughtpad-mermaid-${++mermaidSeq}`, code);
      el.innerHTML = svg;
    } catch (e) {
      el.classList.add("cm-block-error");
      el.textContent = `Mermaid: ${(e as Error).message ?? e}`;
    }
  },
});

registerBlockRenderer({
  languages: ["math", "latex", "katex"],
  async render(code, el) {
    const { default: katex } = await import("katex");
    katex.render(code, el, { displayMode: true, throwOnError: false });
  },
});

class BlockWidget extends WidgetType {
  constructor(
    readonly lang: string,
    readonly code: string,
    readonly from: number,
  ) {
    super();
  }
  eq(other: BlockWidget) {
    return other.lang === this.lang && other.code === this.code;
  }
  toDOM(view: EditorView) {
    const el = document.createElement("div");
    el.className = `cm-block-widget cm-block-${this.lang}`;
    el.title = "Click to edit";
    const renderer = registry.get(this.lang);
    void renderer?.render(this.code, el);
    el.addEventListener("mousedown", (e) => {
      e.preventDefault();
      // Put the cursor on the first content line so the source shows.
      const line = view.state.doc.lineAt(this.from);
      const target = Math.min(line.to + 1, view.state.doc.length);
      view.dispatch({ selection: { anchor: target } });
      view.focus();
    });
    return el;
  }
  ignoreEvent() {
    return true;
  }
}

function overlapsSelection(state: EditorState, from: number, to: number) {
  return state.selection.ranges.some((r) => r.from <= to && r.to >= from);
}

function buildBlocks(state: EditorState): DecorationSet {
  const deco: Range<Decoration>[] = [];
  const tree = ensureSyntaxTree(state, state.doc.length, 50) ?? syntaxTree(state);
  const codeRanges: Array<[number, number]> = [];

  tree.iterate({
    enter(node) {
      if (node.name !== "FencedCode") return;
      codeRanges.push([node.from, node.to]);
      const info = node.node.getChild("CodeInfo");
      const text = node.node.getChild("CodeText");
      if (!info) return false;
      const lang = state.doc.sliceString(info.from, info.to).trim().split(/\s+/)[0].toLowerCase();
      if (!registry.has(lang) || overlapsSelection(state, node.from, node.to)) return false;
      const code = text ? state.doc.sliceString(text.from, text.to) : "";
      const from = state.doc.lineAt(node.from).from;
      const to = state.doc.lineAt(node.to).to;
      deco.push(Decoration.replace({ widget: new BlockWidget(lang, code, from), block: true }).range(from, to));
      return false;
    },
  });

  // $$ … $$ display math (each fence on its own line).
  const inCode = (pos: number) => codeRanges.some(([a, b]) => pos >= a && pos <= b);
  for (let i = 1; i <= state.doc.lines; i++) {
    const open = state.doc.line(i);
    if (open.text.trim() !== "$$" || inCode(open.from)) continue;
    for (let j = i + 1; j <= state.doc.lines; j++) {
      const close = state.doc.line(j);
      if (close.text.trim() !== "$$") continue;
      if (!overlapsSelection(state, open.from, close.to)) {
        const code = state.doc.sliceString(open.to + 1, Math.max(open.to + 1, close.from - 1));
        deco.push(
          Decoration.replace({ widget: new BlockWidget("math", code, open.from), block: true }).range(open.from, close.to),
        );
      }
      i = j;
      break;
    }
  }
  return Decoration.set(deco, true);
}

export const blockWidgets = StateField.define<DecorationSet>({
  create: buildBlocks,
  update(value, tr) {
    if (tr.docChanged || tr.selection || syntaxTree(tr.startState) !== syntaxTree(tr.state)) return buildBlocks(tr.state);
    return value;
  },
  provide: (f) => EditorView.decorations.from(f),
});
