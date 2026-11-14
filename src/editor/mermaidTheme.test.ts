import { afterEach, describe, expect, it } from "vitest";
import { mermaidTheme } from "./blocks";

describe("mermaid theme", () => {
  afterEach(() => delete document.documentElement.dataset.scheme);

  it("follows the app's chosen scheme, not the system's", () => {
    document.documentElement.dataset.scheme = "dark";
    expect(mermaidTheme()).toBe("dark");
    document.documentElement.dataset.scheme = "light";
    expect(mermaidTheme()).toBe("neutral");
  });
});
