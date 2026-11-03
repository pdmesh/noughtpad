// Cheap clipboard checks the editor needs on its first frame. The unified
// pipeline in `markdown.ts` is heavy, so it loads after the editor is ready.

/** Marker attribute on HTML we put on the clipboard, so we can recognise our own copies. */
export const NOUGHTPAD_HTML_MARKER = "data-noughtpad-copy";

/** Decide whether a paste should be converted from HTML. */
export function shouldConvertHtml(types: readonly string[], html: string, plainOnly: boolean): boolean {
  if (plainOnly || !html.trim()) return false;
  if (html.includes(NOUGHTPAD_HTML_MARKER)) return false; // our own copy: text/plain is the Markdown
  if (types.includes("vscode-editor-data")) return false; // code editors: keep the source text
  // Only bother when the HTML carries real structure or formatting.
  return /<(p|h[1-6]|ul|ol|li|table|blockquote|pre|a|strong|b|em|i|img|br|code)\b/i.test(html);
}

export function isUrl(text: string): boolean {
  return /^(https?:\/\/|mailto:)\S+$/i.test(text.trim());
}
