import { describe, expect, it } from "vitest";
import { extensionFor } from "./clipboard";
import { openableUrl } from "../components/MetaStrip";

describe("extensionFor", () => {
  it("maps image types to file extensions", () => {
    expect(extensionFor(new File([], "a", { type: "image/png" }))).toBe("png");
    expect(extensionFor(new File([], "a", { type: "image/jpeg" }))).toBe("jpg");
    expect(extensionFor(new File([], "a", { type: "image/svg+xml" }))).toBe("svg");
    expect(extensionFor(new File([], "shot.gif", { type: "" }))).toBe("gif");
  });
});

describe("openableUrl", () => {
  it("offers http(s) and bare domains", () => {
    expect(openableUrl("https://example.com/a?b=1")).toBe("https://example.com/a?b=1");
    expect(openableUrl("example.com/page")).toBe("https://example.com/page");
    expect(openableUrl(" http://localhost:3000 ")).toBe("http://localhost:3000/");
  });
  it("refuses anything else", () => {
    expect(openableUrl("")).toBeNull();
    expect(openableUrl("not a url")).toBeNull();
    expect(openableUrl("javascript:alert(1)")).toBeNull();
    expect(openableUrl("file:///etc/passwd")).toBeNull();
    expect(openableUrl("notes")).toBeNull();
  });
});
