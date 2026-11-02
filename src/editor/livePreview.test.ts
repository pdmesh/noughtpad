import { describe, expect, it } from "vitest";
import { INLINE_MATH } from "./livePreview";

const matches = (s: string) => [...s.matchAll(INLINE_MATH)].map((m) => m[1]);

describe("inline math detection", () => {
  it("finds TeX between single dollars", () => {
    expect(matches("Energy $E = mc^2$ and $x$.")).toEqual(["E = mc^2", "x"]);
  });
  it("ignores prices and escaped or display dollars", () => {
    expect(matches("It costs $5 and $10 today")).toEqual([]);
    expect(matches("a \\$x$ b")).toEqual([]);
    expect(matches("$$x$$")).toEqual([]);
    expect(matches("$ spaced $")).toEqual([]);
  });
});
