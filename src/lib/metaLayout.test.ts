import { describe, expect, it } from "vitest";
import { fitCount, headingLine } from "./metaLayout";

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
