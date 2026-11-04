// Markdown <-> HTML conversion built on the unified ecosystem
// (remark/rehype). The same pipeline drives "copy as rich text", paste
// conversion, and any future export, so behaviour stays consistent.

import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import remarkRehype from "remark-rehype";
import rehypeKatex from "rehype-katex";
import rehypeStringify from "rehype-stringify";
import rehypeParse from "rehype-parse";
import rehypeRemark from "rehype-remark";
import remarkStringify from "remark-stringify";

import { NOUGHTPAD_HTML_MARKER } from "./pasteRules";

export { isUrl, NOUGHTPAD_HTML_MARKER, shouldConvertHtml } from "./pasteRules";

const toHtml = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkMath)
  .use(remarkRehype)
  .use(rehypeKatex)
  .use(rehypeStringify);

// Model answers are untrusted: a note clipped from the web can steer a model into writing
// anything. Images are replaced by their alt text, so nothing loads from the network on its
// own (an image URL could carry note text out). Links keep only http(s) and mailto targets,
// and are opened by the caller in the system browser.
type HastNode = { type: string; tagName?: string; properties?: Record<string, unknown>; children?: HastNode[]; value?: string };
const SAFE_HREF = /^(https?:|mailto:)/i;
function untrustedContent() {
  const walk = (node: HastNode) => {
    node.children = node.children?.map((c) => {
      if (c.type === "element" && c.tagName === "img") {
        const alt = String(c.properties?.alt ?? "");
        return { type: "text", value: alt };
      }
      if (c.type === "element" && c.tagName === "a") {
        const href = String(c.properties?.href ?? "");
        if (!SAFE_HREF.test(href)) return { type: "element", tagName: "span", properties: {}, children: c.children ?? [] };
        c.properties = { href, rel: "noopener noreferrer" };
      }
      walk(c);
      return c;
    });
  };
  return (tree: HastNode) => walk(tree);
}

const toUntrustedHtml = unified()
  .use(remarkParse)
  .use(remarkGfm)
  .use(remarkMath)
  .use(remarkRehype)
  .use(untrustedContent)
  .use(rehypeKatex)
  .use(rehypeStringify);

/** Like markdownToHtml, for text we didn't write (model answers): no images, safe links only. */
export function untrustedMarkdownToHtml(md: string): string {
  return String(toUntrustedHtml.processSync(md));
}

const toMarkdown = unified()
  .use(rehypeParse, { fragment: true })
  .use(rehypeRemark)
  .use(remarkGfm)
  .use(remarkStringify, {
    bullet: "-",
    emphasis: "*",
    strong: "*",
    fences: true,
    rule: "-",
    listItemIndent: "one",
  });

export function markdownToHtml(md: string): string {
  return String(toHtml.processSync(md));
}

/** HTML for the clipboard, tagged so a paste back into Noughtpad uses the Markdown. */
export function clipboardHtml(md: string): string {
  return `<div ${NOUGHTPAD_HTML_MARKER}="1">${markdownToHtml(md)}</div>`;
}

export function htmlToMarkdown(html: string): string {
  const cleaned = html
    // Google Docs wraps everything in <b style="font-weight:normal" id="docs-internal-guid-…">.
    .replace(/<b[^>]*id="docs-internal-guid[^"]*"[^>]*>([\s\S]*)<\/b>/i, "$1")
    // Clipboard fragments from browsers carry these comments.
    .replace(/<!--StartFragment-->|<!--EndFragment-->/g, "");
  return String(toMarkdown.processSync(cleaned)).replace(/\n+$/, "");
}

