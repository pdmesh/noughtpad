import { describe, expect, it } from "vitest";
import { DEFAULT_THEME, parseTheme, resolveTheme } from "./theme";

describe("theme preferences", () => {
  it("defaults to following the system, Paper by day and Graphite by night", () => {
    expect(parseTheme(null)).toEqual(DEFAULT_THEME);
    expect(parseTheme("not json")).toEqual(DEFAULT_THEME);
  });

  it("keeps the good fields of a saved value and resets the bad ones", () => {
    expect(parseTheme('{"mode":"dark","light":"white","dark":"neon"}')).toEqual({ mode: "dark", light: "white", dark: "graphite" });
  });

  it("follows the system in System mode", () => {
    expect(resolveTheme(DEFAULT_THEME, false)).toEqual({ name: "paper", scheme: "light" });
    expect(resolveTheme(DEFAULT_THEME, true)).toEqual({ name: "graphite", scheme: "dark" });
  });

  it("ignores the system when the mode is fixed", () => {
    expect(resolveTheme({ mode: "light", light: "white", dark: "midnight" }, true)).toEqual({ name: "white", scheme: "light" });
    expect(resolveTheme({ mode: "dark", light: "white", dark: "midnight" }, false)).toEqual({ name: "midnight", scheme: "dark" });
  });
});
