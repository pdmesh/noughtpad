import { useEffect, useState, type RefObject } from "react";
import { Bold, Code, Heading1, Heading2, Image, Italic, Link2, List, ListOrdered, ListTodo, Minus, Quote, Strikethrough, Table2 } from "lucide-react";
import type { EditorHandle } from "../editor/Editor";

const BUTTONS: { id: string; label: string; icon: React.ReactNode }[] = [
  { id: "bold", label: "Bold", icon: <Bold size={16} /> },
  { id: "italic", label: "Italic", icon: <Italic size={16} /> },
  { id: "strike", label: "Strikethrough", icon: <Strikethrough size={16} /> },
  { id: "code", label: "Code", icon: <Code size={16} /> },
  { id: "link", label: "Link", icon: <Link2 size={16} /> },
  { id: "h1", label: "Heading 1", icon: <Heading1 size={16} /> },
  { id: "h2", label: "Heading 2", icon: <Heading2 size={16} /> },
  { id: "bullet", label: "Bullet list", icon: <List size={16} /> },
  { id: "numbered", label: "Numbered list", icon: <ListOrdered size={16} /> },
  { id: "task", label: "Task list", icon: <ListTodo size={16} /> },
  { id: "quote", label: "Quote", icon: <Quote size={16} /> },
  { id: "table", label: "Table", icon: <Table2 size={16} /> },
  { id: "image", label: "Image", icon: <Image size={16} /> },
  { id: "hr", label: "Divider", icon: <Minus size={16} /> },
];

/**
 * A tap alternative to the right-click menu (`src/editor/contextMenu.ts`),
 * which needs a native popup `nativeContextMenu()` skips on a touch device.
 * Runs the same `COMMANDS` the slash menu and keyboard shortcuts use
 * (`src/editor/commands.ts`), through `EditorHandle.runCommand`, so there's
 * one definition of what each command does. Shown only on a coarse pointer
 * (see docs/MOBILE.md § Phone layout); cut/copy/paste are left to the
 * webview's own text-selection toolbar.
 *
 * Docked to the bottom of the screen rather than following the text. iOS
 * doesn't shrink the layout viewport when the on-screen keyboard opens, so a
 * plain `bottom: 0` would sit behind the keyboard: the visual viewport says
 * how much of the screen the keyboard covers, and the toolbar is lifted by that.
 */
export function FormatToolbar({ editorRef }: { editorRef: RefObject<EditorHandle | null> }) {
  const keyboard = useKeyboardInset();
  return (
    <div
      className={`format-toolbar ${keyboard > 0 ? "is-raised" : ""}`}
      role="toolbar"
      aria-label="Formatting"
      style={{ bottom: keyboard }}
    >
      {BUTTONS.map((b) => (
        <button
          key={b.id}
          className="format-toolbar-btn"
          title={b.label}
          aria-label={b.label}
          // Keep the editor's focus and on-screen keyboard open.
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => editorRef.current?.runCommand(b.id)}
        >
          {b.icon}
        </button>
      ))}
    </div>
  );
}

/** How far the on-screen keyboard covers the bottom of the layout viewport, in px. */
function useKeyboardInset(): number {
  const [inset, setInset] = useState(0);
  useEffect(() => {
    const vv = window.visualViewport;
    if (!vv) return;
    const update = () => setInset(Math.max(0, Math.round(window.innerHeight - vv.height - vv.offsetTop)));
    update();
    vv.addEventListener("resize", update);
    vv.addEventListener("scroll", update);
    return () => {
      vv.removeEventListener("resize", update);
      vv.removeEventListener("scroll", update);
    };
  }, []);
  return inset;
}
