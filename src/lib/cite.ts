import { untrustedMarkdownToHtml } from "./markdown";

/**
 * Render a model's Markdown answer, turning `[n]` / `[1, 3]` citations into
 * buttons for notes 1..count. Numbers outside that range are left as text.
 * Raw HTML in the answer is dropped, images become their alt text (nothing loads from the
 * network by itself) and links keep only http(s) and mailto targets. Dollar signs are
 * literal (answers talk about money far more often than they need maths).
 */
export function answerHtml(text: string, count: number): string {
  return untrustedMarkdownToHtml(text.replace(/\$/g, "\\$")).replace(/\[(\d+(?:\s*,\s*\d+)*)\]/g, (m, inner: string) => {
    const nums = inner.split(",").map((x) => Number(x.trim()));
    if (!nums.every((n) => n >= 1 && n <= count)) return m;
    return nums.map((n) => `<button type="button" class="cite" data-cite="${n}" aria-label="Open note ${n}">${n}</button>`).join("");
  });
}
