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

import { weekStart } from "./format";

describe("weekStart", () => {
  it("returns the Monday of the week", () => {
    expect(weekStart("2026-09-12")).toBe("2026-09-07"); // Saturday
    expect(weekStart("2026-09-07")).toBe("2026-09-07"); // Monday
    expect(weekStart("2026-09-13")).toBe("2026-09-07"); // Sunday
    expect(weekStart("2026-03-01")).toBe("2026-02-23"); // across months
  });
});
