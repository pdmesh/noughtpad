// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";
import { EditorSelection, EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { insertBlock, parseLine, setBlock, stripInline, toggleMark } from "./commands";
import { matchCommands } from "./slash";

// jsdom has no layout: give CodeMirror's measuring something to call, and
// destroy each view so no measurement runs after its test.
Range.prototype.getClientRects ??= () => ({ length: 0, item: () => null, [Symbol.iterator]: [][Symbol.iterator] }) as unknown as DOMRectList;
Range.prototype.getBoundingClientRect ??= () => new DOMRect();
const views: EditorView[] = [];
afterEach(() => views.splice(0).forEach((v) => v.destroy()));

/** A view over `doc`, where `|` marks the cursor and `[`…`]` a selection. */
function view(doc: string) {
  let sel: EditorSelection;
  const a = doc.indexOf("[");
  if (a >= 0) {
    const b = doc.indexOf("]") - 1;
    doc = doc.replace("[", "").replace("]", "");
    sel = EditorSelection.single(a, b);
  } else {
    const c = doc.indexOf("|");
    doc = doc.replace("|", "");
    sel = EditorSelection.single(c);
  }
  const v = new EditorView({ state: EditorState.create({ doc, selection: sel, extensions: [EditorState.allowMultipleSelections.of(true)] }) });
  views.push(v);
  return v;
}

/** The document with the cursor/selection marked the same way. */
function show(v: EditorView) {
  const { from, to } = v.state.selection.main;
  const d = v.state.doc.toString();
  return from === to ? d.slice(0, from) + "|" + d.slice(from) : d.slice(0, from) + "[" + d.slice(from, to) + "]" + d.slice(to);
}

describe("toggleMark", () => {
  it("wraps, unwraps and inserts empty marks", () => {
    const v = view("a [word] b");
    toggleMark(v, "**");
    expect(show(v)).toBe("a **[word]** b");
    toggleMark(v, "**");
    expect(show(v)).toBe("a [word] b");
    const e = view("a | b");
    toggleMark(e, "*");
    expect(show(e)).toBe("a *|* b");
  });
  it("doesn't take bold for italic", () => {
    const v = view("**[x]**");
    toggleMark(v, "*");
    expect(v.state.doc.toString()).toBe("***x***");
  });
});

describe("setBlock", () => {
  it("switches paragraph types and toggles back to body", () => {
    const v = view("hel|lo");
    setBlock(v, "h2");
    expect(show(v)).toBe("## hel|lo");
    setBlock(v, "task");
    expect(show(v)).toBe("- [ ] hel|lo");
    setBlock(v, "task");
    expect(show(v)).toBe("hel|lo");
  });
  it("numbers selected lines", () => {
    const v = view("[a\nb\nc]");
    setBlock(v, "numbered");
    expect(v.state.doc.toString()).toBe("1. a\n2. b\n3. c");
  });
  it("parses line prefixes", () => {
    expect(parseLine("  - [x] done")).toEqual({ indent: "  ", block: "task", rest: "done" });
    expect(parseLine("### Title").block).toBe("h3");
    expect(parseLine("> quoted").block).toBe("quote");
    expect(parseLine("#hashtag").block).toBe("body");
  });
});

describe("insertBlock", () => {
  it("replaces an empty line, or goes below text with a blank line", () => {
    const v = view("|");
    insertBlock(v, "---\n");
    expect(v.state.doc.toString()).toBe("---\n");
    const t = view("para|graph\nnext");
    insertBlock(t, "```\n\n```", [4, 4]);
    expect(show(t)).toBe("paragraph\n\n```\n|\n```\n\nnext");
  });
});

describe("stripInline", () => {
  it("removes inline formatting", () => {
    expect(stripInline("**b** *i* ~~s~~ `c` $x$ snake_case")).toBe("b i s c x snake_case");
  });
});

describe("matchCommands", () => {
  it("finds commands by label and keyword", () => {
    expect(matchCommands("tab")[0].id).toBe("table");
    expect(matchCommands("h2")[0].id).toBe("h2");
    expect(matchCommands("todo")[0].id).toBe("task");
    expect(matchCommands("mermaid")[0].id).toBe("mermaid");
    expect(matchCommands("zzz")).toEqual([]);
  });
});
