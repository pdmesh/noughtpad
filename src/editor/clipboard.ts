// Obsidian-like clipboard behaviour:
//  * Copy/cut puts Markdown on text/plain and rendered HTML on text/html.
//  * Pasting rich HTML (browser, Docs, Word) converts it to Markdown.
//  * Pasting a URL over a selection makes a link.
//  * Pasting an image saves it into the vault and inserts ![](path).
//  * Cmd/Ctrl+Shift+V always pastes plain text.

import { EditorSelection, type Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { clipboardHtml, htmlToMarkdown, isUrl, shouldConvertHtml } from "../lib/markdown";

export interface ClipboardOptions {
  /** Persist binary data, returning a vault-relative path. */
  saveAttachment?: (bytes: Uint8Array, extension: string) => Promise<string>;
}

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
    e.clipboardData.setData("text/html", clipboardHtml(md));
  } catch {
    // Conversion failure must never break copying the Markdown itself.
  }
  e.preventDefault();
  return true;
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
    paste(e, view) {
      const data = e.clipboardData;
      const plainOnly = plainNext;
      plainNext = false;
      if (!data) return false;

      const images = Array.from(data.files).filter((f) => f.type.startsWith("image/"));
      if (images.length && opts.saveAttachment) {
        e.preventDefault();
        void (async () => {
          const links: string[] = [];
          for (const f of images) {
            const ext = f.type.split("/")[1]?.replace("jpeg", "jpg") ?? "png";
            const rel = await opts.saveAttachment!(new Uint8Array(await f.arrayBuffer()), ext);
            links.push(`![](${encodeURI(rel)})`);
          }
          insert(view, links.join("\n"));
        })();
        return true;
      }

      const text = data.getData("text/plain");
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

      const html = data.getData("text/html");
      if (shouldConvertHtml(Array.from(data.types), html, plainOnly)) {
        try {
          const md = htmlToMarkdown(html);
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
