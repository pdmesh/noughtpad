// Right-click menu in the app: a native macOS/Windows menu with Format,
// Paragraph and Insert submenus, plus Cut/Copy/Paste. In a plain browser the
// browser's own menu shows instead.

import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { commandById, type EditorCommand } from "./commands";
import { linkOpener } from "./livePreview";

type Item =
  | { text: string; action?: () => void; accelerator?: string; checked?: boolean; items?: Item[] }
  | { item: "Separator" | "Cut" | "Copy" | "Paste" | "SelectAll" };

const SEP: Item = { item: "Separator" };

function entries(view: EditorView, ids: Array<string | null>, checks = false): Item[] {
  return ids.map((id) => {
    if (id === null) return SEP;
    const c = commandById(id) as EditorCommand;
    return {
      text: c.label,
      accelerator: c.accelerator,
      checked: checks ? (c.active?.(view.state) ?? false) : undefined,
      action: () => {
        c.run(view);
      },
    };
  });
}

export function buildMenu(view: EditorView, linkUrl: string | null): Item[] {
  const items: Item[] = [];
  if (linkUrl) items.push({ text: "Open link", action: () => view.state.facet(linkOpener)(linkUrl) }, SEP);
  items.push(
    ...entries(view, ["link"]).map((i) => ({ ...i, text: "Add link" })),
    SEP,
    { text: "Format", items: entries(view, ["bold", "italic", "strike", null, "code", "math", null, "clear"]) },
    {
      text: "Paragraph",
      items: entries(view, ["bullet", "numbered", "task", null, "h1", "h2", "h3", "h4", "h5", "h6", "body", null, "quote"], true),
    },
    { text: "Insert", items: entries(view, ["table", "image", "hr", null, "codeblock", "mathblock", "mermaid"]) },
    SEP,
    { item: "Cut" },
    { item: "Copy" },
    { item: "Paste" },
    { item: "SelectAll" },
  );
  return items;
}

/** Turn plain item descriptions into the options `Menu.new` takes. */
async function toNative(items: Item[]): Promise<unknown[]> {
  const { CheckMenuItem, MenuItem, PredefinedMenuItem, Submenu } = await import("@tauri-apps/api/menu");
  const make = async (i: Item): Promise<unknown> => {
    if ("item" in i) return PredefinedMenuItem.new({ item: i.item });
    if (i.items) return Submenu.new({ text: i.text, items: (await Promise.all(i.items.map(make))) as never[] });
    if (i.checked !== undefined) return CheckMenuItem.new({ text: i.text, checked: i.checked, accelerator: i.accelerator, action: i.action });
    return MenuItem.new({ text: i.text, accelerator: i.accelerator, action: i.action });
  };
  return Promise.all(items.map(make));
}

export function nativeContextMenu(): Extension {
  return EditorView.domEventHandlers({
    contextmenu(e, view) {
      e.preventDefault();
      const link = (e.target as HTMLElement).closest<HTMLElement>(".cm-md-link")?.dataset.url ?? null;
      void (async () => {
        const { Menu } = await import("@tauri-apps/api/menu");
        const menu = await Menu.new({ items: (await toNative(buildMenu(view, link))) as never[] });
        await menu.popup();
      })().catch((err) => console.error("context menu", err));
      return true;
    },
  });
}
