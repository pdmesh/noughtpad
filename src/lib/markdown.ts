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

