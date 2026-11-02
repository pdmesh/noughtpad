// Obsidian-style live preview for CodeMirror 6.
//
// The document is always the raw Markdown. We only *decorate* it: syntax
// marks (#, **, `, [](url)…) are hidden on lines the cursor isn't on, task
// markers become checkboxes, bullets become dots, images render inline.
// Nothing here ever changes the text except an explicit checkbox click.

import { syntaxTree } from "@codemirror/language";
import type { EditorState, Range } from "@codemirror/state";
import { Facet } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";
import type { SyntaxNodeRef } from "@lezer/common";
import katex from "katex";

/** Resolves a Markdown image/link target to a loadable URL. */
export const assetResolver = Facet.define<(src: string) => string, (src: string) => string>({
  combine: (values) => values[0] ?? ((s) => s),
});

/** Called on Cmd/Ctrl-click of a link. */
export const linkOpener = Facet.define<(url: string) => void, (url: string) => void>({
  combine: (values) => values[0] ?? ((u) => window.open(u, "_blank", "noopener")),
});

const hide = Decoration.replace({});

class CheckboxWidget extends WidgetType {
  constructor(
    readonly checked: boolean,
    readonly pos: number,
  ) {
    super();
  }
  eq(other: CheckboxWidget) {
    return other.checked === this.checked && other.pos === this.pos;
  }
  toDOM() {
    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = this.checked;
    box.className = "cm-md-task";
    box.dataset.pos = String(this.pos);
    box.setAttribute("aria-label", this.checked ? "Completed task" : "Open task");
    return box;
  }
  ignoreEvent() {
    return false;
  }
}

class BulletWidget extends WidgetType {
  eq() {
    return true;
  }
  toDOM() {
    const s = document.createElement("span");
    s.className = "cm-md-bullet";
    s.textContent = "•";
    return s;
  }
}

class InlineMathWidget extends WidgetType {
  constructor(readonly tex: string) {
    super();
  }
  eq(other: InlineMathWidget) {
    return other.tex === this.tex;
  }
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-md-math";
    katex.render(this.tex, span, { throwOnError: false, displayMode: false });
    return span;
  }
}

/**
 * Inline `$…$` maths, using Pandoc's rule so prices don't trigger it: no space
 * after the opening `$` or before the closing one, and no digit right after it.
 */
export const INLINE_MATH = /(?<![\\$\w])\$(?=[^\s$])([^$\n]*?[^\s\\$])\$(?![$\d])/g;

function inCode(state: EditorState, pos: number): boolean {
  for (let n: SyntaxNodeRef["node"] | null = syntaxTree(state).resolveInner(pos, 1); n; n = n.parent) {
    if (n.name === "InlineCode" || n.name === "FencedCode" || n.name === "CodeBlock" || n.name === "URL") return true;
  }
  return false;
}

class ImageWidget extends WidgetType {
  constructor(
    readonly src: string,
    readonly alt: string,
  ) {
    super();
  }
  eq(other: ImageWidget) {
    return other.src === this.src && other.alt === this.alt;
  }
  toDOM() {
    const img = document.createElement("img");
    img.src = this.src;
    img.alt = this.alt;
    img.className = "cm-md-image";
    img.loading = "lazy";
    return img;
  }
}

function activeLines(state: EditorState): Set<number> {
  const lines = new Set<number>();
  for (const r of state.selection.ranges) {
    const a = state.doc.lineAt(r.from).number;
    const b = state.doc.lineAt(r.to).number;
    for (let l = a; l <= b; l++) lines.add(l);
  }
  return lines;
}

function build(view: EditorView): DecorationSet {
  const { state } = view;
  const active = view.hasFocus ? activeLines(state) : new Set<number>();
  const isActive = (pos: number) => active.has(state.doc.lineAt(pos).number);
  const deco: Range<Decoration>[] = [];
  const resolve = state.facet(assetResolver);

  const lineClass = (pos: number, cls: string) =>
    deco.push(Decoration.line({ class: cls }).range(state.doc.lineAt(pos).from));

  for (const { from, to } of view.visibleRanges) {
    syntaxTree(state).iterate({
      from,
      to,
      enter: (node: SyntaxNodeRef) => {
        const name = node.name;

        const heading = /^ATXHeading(\d)$/.exec(name) ?? /^SetextHeading(\d)$/.exec(name);
        if (heading) {
          lineClass(node.from, `cm-md-h cm-md-h${heading[1]}`);
          return;
        }

        switch (name) {
          case "HeaderMark": {
            if (isActive(node.from) || node.node.parent?.name.startsWith("Setext")) return;
            // Hide "## " including the following space.
            const end = state.doc.sliceString(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
            deco.push(hide.range(node.from, end));
            return;
          }
          case "EmphasisMark":
          case "StrikethroughMark":
            if (!isActive(node.from)) deco.push(hide.range(node.from, node.to));
            return;
          case "CodeMark":
            if (node.node.parent?.name === "InlineCode" && !isActive(node.from))
              deco.push(hide.range(node.from, node.to));
            return;
          case "InlineCode":
            deco.push(Decoration.mark({ class: "cm-md-inline-code" }).range(node.from, node.to));
            return;
          case "Link": {
            const urlNode = node.node.getChild("URL");
            const url = urlNode ? state.doc.sliceString(urlNode.from, urlNode.to) : "";
            const marks = node.node.getChildren("LinkMark");
            if (marks.length >= 2) {
              const textFrom = marks[0].to;
              const textTo = marks[1].from;
              if (textTo > textFrom)
                deco.push(
                  Decoration.mark({ class: "cm-md-link", attributes: { "data-url": url } }).range(textFrom, textTo),
                );
            }
            if (!isActive(node.from) && marks.length >= 2) {
              deco.push(hide.range(node.from, marks[0].to));
              deco.push(hide.range(marks[1].from, node.to));
            }
            return false;
          }
          case "URL":
            // Bare autolinks.
            if (node.node.parent?.name !== "Link" && node.node.parent?.name !== "Image") {
              const url = state.doc.sliceString(node.from, node.to);
              deco.push(
                Decoration.mark({ class: "cm-md-link", attributes: { "data-url": url } }).range(node.from, node.to),
              );
            }
            return;
          case "Image": {
            if (isActive(node.from)) return false;
            const urlNode = node.node.getChild("URL");
            if (!urlNode) return false;
            const raw = state.doc.sliceString(node.from, node.to);
            const alt = /^!\[([^\]]*)\]/.exec(raw)?.[1] ?? "";
            const src = resolve(state.doc.sliceString(urlNode.from, urlNode.to).replace(/^<|>$/g, ""));
            deco.push(Decoration.replace({ widget: new ImageWidget(src, alt) }).range(node.from, node.to));
            return false;
          }
          case "Blockquote":
            for (let pos = node.from; pos <= node.to; ) {
              const line = state.doc.lineAt(pos);
              lineClass(line.from, "cm-md-quote");
              pos = line.to + 1;
            }
            return;
          case "QuoteMark":
            if (!isActive(node.from)) {
              const end = state.doc.sliceString(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
              deco.push(hide.range(node.from, end));
            }
            return;
          case "ListMark": {
            const parentList = node.node.parent?.parent?.name;
            const item = node.node.parent;
            const isTask = item?.getChild("Task") != null;
            if (isTask && !isActive(node.from)) {
              const end = state.doc.sliceString(node.to, node.to + 1) === " " ? node.to + 1 : node.to;
              deco.push(hide.range(node.from, end));
            } else if (parentList === "BulletList" && !isActive(node.from)) {
              deco.push(Decoration.replace({ widget: new BulletWidget() }).range(node.from, node.to));
            }
            return;
          }
          case "TaskMarker": {
            const text = state.doc.sliceString(node.from, node.to);
            const checked = /\[[xX]\]/.test(text);
            if (checked) {
              const line = state.doc.lineAt(node.from);
              deco.push(Decoration.mark({ class: "cm-md-task-done" }).range(node.to, line.to));
            }
            if (!isActive(node.from))
              deco.push(Decoration.replace({ widget: new CheckboxWidget(checked, node.from) }).range(node.from, node.to));
            return;
          }
          case "HorizontalRule":
            if (!isActive(node.from)) {
              lineClass(node.from, "cm-md-hr");
              deco.push(hide.range(node.from, node.to));
            }
            return;
          case "FencedCode":
          case "CodeBlock":
            for (let pos = node.from; pos <= node.to; ) {
              const line = state.doc.lineAt(pos);
              lineClass(line.from, "cm-md-codeblock");
              pos = line.to + 1;
            }
            return;
        }
      },
    });
  }
  for (const { from, to } of view.visibleRanges) {
    for (let pos = from; pos <= to; ) {
      const line = state.doc.lineAt(pos);
      if (!active.has(line.number) && line.text.includes("$") && line.text.trim() !== "$$") {
        for (const m of line.text.matchAll(INLINE_MATH)) {
          const start = line.from + m.index!;
          if (inCode(state, start)) continue;
          deco.push(Decoration.replace({ widget: new InlineMathWidget(m[1]) }).range(start, start + m[0].length));
        }
      }
      pos = line.to + 1;
    }
  }
  return Decoration.set(deco, true);
}

export const livePreview = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = build(view);
    }
    update(u: ViewUpdate) {
      if (
        u.docChanged ||
        u.viewportChanged ||
        u.selectionSet ||
        u.focusChanged ||
        syntaxTree(u.startState) !== syntaxTree(u.state)
      )
        this.decorations = build(u.view);
    }
  },
  {
    decorations: (v) => v.decorations,
    eventHandlers: {
      mousedown(e, view) {
        const target = e.target as HTMLElement;
        if (target instanceof HTMLInputElement && target.classList.contains("cm-md-task")) {
          const pos = Number(target.dataset.pos);
          const text = view.state.doc.sliceString(pos, pos + 3);
          const next = /\[[xX]\]/.test(text) ? "[ ]" : "[x]";
          view.dispatch({ changes: { from: pos, to: pos + 3, insert: next } });
          e.preventDefault();
          return true;
        }
        const link = target.closest<HTMLElement>(".cm-md-link");
        if (link && (e.metaKey || e.ctrlKey) && link.dataset.url) {
          view.state.facet(linkOpener)(link.dataset.url);
          e.preventDefault();
          return true;
        }
        return false;
      },
    },
  },
);
