import { describe, expect, it } from "vitest";
import { extensionFor, imageBlock } from "./clipboard";
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

describe("imageBlock", () => {
  const place = (doc: string, from: number, to = from, links = ["![](a.png)"]) => {
    const b = imageBlock(doc, from, to, links);
    return { text: doc.slice(0, b.from) + b.insert + doc.slice(b.to), cursor: b.cursor };
  };

  it("gives an image pasted mid-line a paragraph of its own", () => {
    expect(place("Alpha line.\n\nBeta.", 5).text).toBe("Alpha\n\n![](a.png)\n\nline.\n\nBeta.");
    expect(place("Alpha line.\n\nBeta.", 6).text).toBe("Alpha\n\n![](a.png)\n\nline.\n\nBeta.");
  });

  it("adds no extra blank lines where they already are", () => {
    expect(place("One.\n\nTwo.", 6).text).toBe("One.\n\n![](a.png)\n\nTwo.");
    expect(place("", 0).text).toBe("![](a.png)\n");
  });

  it("separates several images instead of putting them side by side", () => {
    expect(place("Text.", 5, 5, ["![](a.png)", "![](b.png)"]).text).toBe("Text.\n\n![](a.png)\n\n![](b.png)\n");
  });

  it("leaves the cursor on a fresh line at the end of the note", () => {
    const r = place("Text.", 5);
    expect(r.cursor).toBe(r.text.length);
  });

  it("leaves the cursor just after the image mid-note", () => {
    const r = place("One.\n\nTwo.", 6);
    expect(r.text.slice(0, r.cursor)).toBe("One.\n\n![](a.png)");
  });
});
