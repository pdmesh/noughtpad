// Obsidian-like clipboard behaviour:
//  * Copy/cut puts Markdown on text/plain and rendered HTML on text/html.
//  * Pasting rich HTML (browser, Docs, Word) converts it to Markdown.
//  * Pasting a URL over a selection makes a link.
//  * Pasting or dropping an image saves it into the vault and inserts ![](path).
//  * Cmd/Ctrl+Shift+V always pastes plain text.

import { EditorSelection, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { isUrl, shouldConvertHtml } from "../lib/pasteRules";

// The Markdown <-> HTML pipeline is the heaviest part of the editor, so it
// stays out of the startup bundle and loads once the editor is idle.
type Pipeline = typeof import("../lib/markdown");
let pipeline: Pipeline | undefined;
let loading: Promise<Pipeline> | undefined;

export function loadMarkdownPipeline(): Promise<Pipeline> {
  return (loading ??= import("../lib/markdown").then((m) => (pipeline = m)));
}

export interface ClipboardOptions {
  /** Persist binary data, returning a vault-relative path. */
  saveAttachment?: (bytes: Uint8Array, extension: string) => Promise<string>;
  /**
   * Save the image on the system clipboard, returning its path, or null if there
   * is none. Used when a paste reaches the page with nothing usable in it, which
   * the macOS webview does for some screenshots and images copied from apps.
   */
  pasteClipboardImage?: () => Promise<string | null>;
  /** Report a failure (saving an image) to the person. */
  onError?: (message: string) => void;
}

/** Image files in a paste or drop, from `files` or, failing that, `items`. */
export function imageFiles(data: DataTransfer): File[] {
  const files = Array.from(data.files ?? []).filter((f) => f.type.startsWith("image/"));
  if (files.length) return files;
  return Array.from(data.items ?? [])
    .filter((i) => i.kind === "file" && i.type.startsWith("image/"))
    .map((i) => i.getAsFile())
    .filter((f): f is File => !!f);
}

export function extensionFor(file: File): string {
  const sub = file.type.split("/")[1]?.split("+")[0] ?? "";
  if (sub === "jpeg") return "jpg";
  if (sub) return sub;
  return file.name.split(".").pop() || "png";
}

const imageLink = (rel: string) => `![](${encodeURI(rel)})`;

function selectedMarkdown(view: EditorView): string {
  return view.state.selection.ranges
    .filter((r) => !r.empty)
    .map((r) => view.state.sliceDoc(r.from, r.to))
    .join("\n");
}

function writeClipboard(e: ClipboardEvent, md: string) {
  if (!e.clipboardData) return false;
  e.clipboardData.setData("text/plain", md);
  try {
    // In the first moments after launch the pipeline may still be loading: copy plain text only.
    if (pipeline) e.clipboardData.setData("text/html", pipeline.clipboardHtml(md));
  } catch {
    // Conversion failure must never break copying the Markdown itself.
  }
  e.preventDefault();
  return true;
}

function message(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) return String((err as { message: unknown }).message);
  return String(err);
}

/**
 * The text that puts image links in place of doc[from..to], each in a paragraph of its own
 * (an image inside a line of text is drawn over that text, and side by side images crowd
 * the page). Returns the range to replace (widened over spaces at a split), the text, and
 * where the cursor goes: just after the last image, or on the fresh line below it at the
 * end of the note.
 */
export function imageBlock(doc: string, from: number, to: number, links: string[]) {
  // Splitting a line: drop the spaces either side of the split.
  while (from > 0 && (doc[from - 1] === " " || doc[from - 1] === "\t")) from--;
  while (to < doc.length && (doc[to] === " " || doc[to] === "\t")) to++;
  const before = doc.slice(Math.max(0, from - 2), from);
  const after = doc.slice(to, to + 2);
  const prefix = from === 0 || before.endsWith("\n\n") ? "" : before.endsWith("\n") ? "\n" : "\n\n";
  const body = links.join("\n\n");
  if (to >= doc.length) return { from, to, insert: prefix + body + "\n", cursor: from + prefix.length + body.length + 1 };
  const suffix = after.startsWith("\n\n") ? "" : after.startsWith("\n") ? "\n" : "\n\n";
  return { from, to, insert: prefix + body + suffix, cursor: from + prefix.length + body.length };
}

/** Put image links over from..to (clamped to the document), each on a paragraph of its own. */
function insertImageLinks(view: EditorView, from: number, to: number, links: string[]) {
  const len = view.state.doc.length;
  const a = Math.min(from, len);
  const b = Math.min(Math.max(to, a), len);
  const block = imageBlock(view.state.doc.toString(), a, b, links);
  view.dispatch({
    changes: { from: block.from, to: block.to, insert: block.insert },
    selection: EditorSelection.cursor(block.cursor),
    scrollIntoView: true,
    userEvent: "input.paste",
  });
}

/** Save images and insert their links at `pos`, or over the selection when null. */
export async function saveImages(view: EditorView, images: File[], opts: ClipboardOptions, pos: number | null) {
  const sel = view.state.selection.main;
  // Remember the place across the save, which can take a moment for a large image.
  const [from, to] = pos === null ? [sel.from, sel.to] : [pos, pos];
  const links: string[] = [];
  for (const f of images) {
    try {
      links.push(imageLink(await opts.saveAttachment!(new Uint8Array(await f.arrayBuffer()), extensionFor(f))));
    } catch (err) {
      opts.onError?.(`Couldn't save the image: ${message(err)}`);
    }
  }
  if (links.length) insertImageLinks(view, from, to, links);
}

/** Ask for image files and insert them at the cursor (Insert → Image…). */
export function pickImages(view: EditorView, opts: ClipboardOptions) {
  if (!opts.saveAttachment) return;
  const input = document.createElement("input");
  input.type = "file";
  input.accept = "image/*";
  input.multiple = true;
  input.addEventListener("change", () => {
    const files = Array.from(input.files ?? []);
    if (files.length) void saveImages(view, files, opts, null);
  });
  input.click();
}

function insert(view: EditorView, text: string) {
  view.dispatch(view.state.replaceSelection(text), { scrollIntoView: true, userEvent: "input.paste" });
}

export function markdownClipboard(opts: ClipboardOptions = {}): Extension {
  let plainNext = false;

  return EditorView.domEventHandlers({
    keydown(e) {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "v") plainNext = true;
      return false;
    },
    copy(e, view) {
      const md = selectedMarkdown(view);
      return md ? writeClipboard(e, md) : false;
    },
    cut(e, view) {
      const md = selectedMarkdown(view);
      if (!md || !writeClipboard(e, md)) return false;
      view.dispatch(view.state.replaceSelection(""), { userEvent: "delete.cut" });
      return true;
    },
    dragover(e) {
      // Allow dropping image files (the default would open them in the window).
      if (opts.saveAttachment && e.dataTransfer && Array.from(e.dataTransfer.types).includes("Files")) {
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
        return true;
      }
      return false;
    },
    drop(e, view) {
      if (!e.dataTransfer || !opts.saveAttachment) return false;
      const images = imageFiles(e.dataTransfer);
      if (!images.length) return false;
      e.preventDefault();
      // Below the line it lands on, never inside a word.
      const at = view.posAtCoords({ x: e.clientX, y: e.clientY }) ?? view.state.selection.main.head;
      void saveImages(view, images, opts, view.state.doc.lineAt(at).to);
      return true;
    },
    paste(e, view) {
      const data = e.clipboardData;
      const plainOnly = plainNext;
      plainNext = false;
      if (!data) return false;

      const images = imageFiles(data);
      if (images.length && opts.saveAttachment) {
        e.preventDefault();
        void saveImages(view, images, opts, null);
        return true;
      }

      const text = data.getData("text/plain");
      const html = data.getData("text/html");
      // Nothing the page can use: ask the app for the clipboard's image directly.
      if (!text && !html && opts.pasteClipboardImage) {
        e.preventDefault();
        const at = view.state.selection.main;
        void opts
          .pasteClipboardImage()
          .then((rel) => rel && insertImageLinks(view, at.from, at.to, [imageLink(rel)]))
          .catch((err) => opts.onError?.(`Couldn't paste the image: ${message(err)}`));
        return true;
      }

      const sel = view.state.selection.main;
      if (!plainOnly && isUrl(text) && !sel.empty) {
        const label = view.state.sliceDoc(sel.from, sel.to);
        if (!label.includes("\n")) {
          e.preventDefault();
          const link = `[${label}](${text.trim()})`;
          view.dispatch({
            changes: { from: sel.from, to: sel.to, insert: link },
            selection: EditorSelection.cursor(sel.from + link.length),
            userEvent: "input.paste",
          });
          return true;
        }
      }

      if (shouldConvertHtml(Array.from(data.types), html, plainOnly)) {
        if (!pipeline) {
          // Not loaded yet: take over the paste, convert once it arrives, and fall back to plain text.
          e.preventDefault();
          void loadMarkdownPipeline()
            .then((p) => p.htmlToMarkdown(html))
            .catch(() => "")
            .then((md) => insert(view, md.trim() ? md : text));
          return true;
        }
        try {
          const md = pipeline.htmlToMarkdown(html);
          if (md.trim()) {
            e.preventDefault();
            insert(view, md);
            return true;
          }
        } catch {
          // Fall through to the default plain-text paste.
        }
      }
      return false;
    },
  });
}
