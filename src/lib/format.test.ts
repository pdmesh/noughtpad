import { describe, expect, it } from "vitest";
import { formatTime, plainExcerpt, snippetParts } from "./format";

describe("format", () => {
  it("splits search snippets on match markers", () => {
    expect(snippetParts("a \u0002b\u0003 c")).toEqual([["a ", false], ["b", true], [" c", false]]);
  });
  it("strips markdown for list excerpts", () => {
    expect(plainExcerpt("## Head\n\n- [ ] task with [link](http://x) and **bold**")).toBe("Head task with link and bold");
  });
  it("uses the wall-clock time stored in the note", () => {
    expect(formatTime("2026-09-12T23:10:00+05:30")).toMatch(/11:10|23:10/);
  });
});
