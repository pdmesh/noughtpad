import { describe, expect, it } from "vitest";
import { fitCount, headingLine, inNoteText } from "./metaLayout";

describe("first-line heading", () => {
  it("finds a heading on the first line", () => {
    expect(headingLine("# Hi\nbody")).toEqual({ level: 1, to: 4 });
    expect(headingLine("### Deep")).toEqual({ level: 3, to: 8 });
  });

  it("ignores anything else", () => {
    expect(headingLine("Hi\n# x")).toBeNull();
    expect(headingLine("#tag")).toBeNull();
    expect(headingLine("# ")).toBeNull();
    expect(headingLine("")).toBeNull();
  });
});

describe("how many tags fit on one line", () => {
  it("shows them all when they fit", () => {
    expect(fitCount([50, 50, 50], 200, 6, 30)).toBe(3);
  });

  it("leaves room for the +N chip when they don't", () => {
    expect(fitCount([50, 50, 50], 120, 6, 30)).toBe(1);
  });

  it("can show none", () => {
    expect(fitCount([300], 100, 6, 30)).toBe(0);
    expect(fitCount([], 100, 6, 30)).toBe(0);
  });
});

describe("is this the note's text", () => {
  it("counts the editor's text but not the details line placed inside it", () => {
    const host = document.createElement("div");
    host.innerHTML = `<div class="cm-editor"><div class="cm-content"><div class="cm-line"><span id="t">hi</span></div>
      <div class="note-header-slot"><input id="label" /></div></div></div><input id="outside" />`;
    expect(inNoteText(host.querySelector("#t"))).toBe(true);
    expect(inNoteText(host.querySelector("#label"))).toBe(false);
    expect(inNoteText(host.querySelector("#outside"))).toBe(false);
    expect(inNoteText(null)).toBe(false);
  });
});
