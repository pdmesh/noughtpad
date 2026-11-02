import { describe, expect, it } from "vitest";
import { clipboardHtml, htmlToMarkdown, isUrl, markdownToHtml, NOUGHT_HTML_MARKER, shouldConvertHtml } from "./markdown";

describe("markdownToHtml", () => {
  it("renders GFM and math", () => {
    const html = markdownToHtml("# Hi\n\n- [x] done\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n$$\nx^2\n$$");
    expect(html).toContain("<h1>Hi</h1>");
    expect(html).toContain('type="checkbox"');
    expect(html).toContain("<table>");
    expect(html).toContain("katex");
  });
  it("tags clipboard html so Nought can recognise its own copies", () => {
    expect(clipboardHtml("**b**")).toContain(NOUGHT_HTML_MARKER);
  });
});

describe("htmlToMarkdown", () => {
  it("converts common rich text", () => {
    const md = htmlToMarkdown(
      '<h2>Title</h2><p>Some <strong>bold</strong> and <em>it</em> with a <a href="https://x.y">link</a>.</p><ul><li>one</li><li>two</li></ul>',
    );
    expect(md).toBe("## Title\n\nSome **bold** and *it* with a [link](https://x.y).\n\n- one\n- two");
  });
  it("converts tables", () => {
    const md = htmlToMarkdown("<table><tr><th>a</th><th>b</th></tr><tr><td>1</td><td>2</td></tr></table>");
    expect(md).toContain("| a | b |");
  });
  it("unwraps the Google Docs bold wrapper", () => {
    const md = htmlToMarkdown('<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-abc"><p>plain</p></b>');
    expect(md).toBe("plain");
  });
  it("round-trips Markdown through rich copy and paste", () => {
    const src = "Quote:\n\n> Customers describe what they want.\n\n1. first\n2. second";
    expect(htmlToMarkdown(markdownToHtml(src))).toBe(src);
  });
});

describe("paste heuristics", () => {
  it("keeps plain text for our own copies, code editors and explicit plain paste", () => {
    expect(shouldConvertHtml(["text/html"], `<div ${NOUGHT_HTML_MARKER}="1"><p>x</p></div>`, false)).toBe(false);
    expect(shouldConvertHtml(["text/html", "vscode-editor-data"], "<p>x</p>", false)).toBe(false);
    expect(shouldConvertHtml(["text/html"], "<p>x</p>", true)).toBe(false);
    expect(shouldConvertHtml(["text/html"], "<span>x</span>", false)).toBe(false);
    expect(shouldConvertHtml(["text/html"], "<p>x <b>y</b></p>", false)).toBe(true);
  });
  it("detects urls", () => {
    expect(isUrl(" https://example.com/a?b=c ")).toBe(true);
    expect(isUrl("see https://example.com")).toBe(false);
  });
});
