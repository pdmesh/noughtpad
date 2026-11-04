import { describe, expect, it } from "vitest";
import { DEFAULT_STYLE, fontFamily, parseStyle } from "./textStyle";

describe("text style", () => {
  it("defaults to the system font at the system size", () => {
    expect(parseStyle(null)).toEqual(DEFAULT_STYLE);
    expect(fontFamily(DEFAULT_STYLE)).toMatch(/^system-ui/);
    expect(DEFAULT_STYLE.size).toBeNull();
  });

  it("ignores malformed or out-of-range saved values", () => {
    expect(parseStyle("not json")).toEqual(DEFAULT_STYLE);
    expect(parseStyle(JSON.stringify({ font: "comic", size: 400 }))).toEqual(DEFAULT_STYLE);
    expect(parseStyle(JSON.stringify({ font: "serif", size: 20 }))).toEqual({ font: "serif", custom: "", size: 20 });
  });

  it("puts a custom font first, with the system font behind it", () => {
    expect(fontFamily({ font: "custom", custom: "Charter", size: null })).toMatch(/^"Charter", system-ui/);
    expect(fontFamily({ font: "custom", custom: 'Evil"; x', size: null })).toMatch(/^"Evil; x", /);
    expect(fontFamily({ font: "custom", custom: "  ", size: null })).toMatch(/^system-ui/);
  });
});
