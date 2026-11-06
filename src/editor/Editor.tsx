import { useEffect, useImperativeHandle, useRef, type Ref } from "react";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { markdown, markdownKeymap, markdownLanguage } from "@codemirror/lang-markdown";
import { languages } from "@codemirror/language-data";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { drawSelection, EditorView, keymap, placeholder as placeholderExt } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";
import { assetResolver, linkOpener, livePreview } from "./livePreview";
import { blockWidgets } from "./blocks";
import { markdownClipboard, pickImages, saveImages, type ClipboardOptions } from "./clipboard";
import { commandById, COMMANDS, cursorToEnd, imagePicker } from "./commands";
import { slashMenu } from "./slash";
import { nativeContextMenu } from "./contextMenu";
import { isTauri } from "../lib/api";
import { isCoarsePointer } from "../lib/platform";

// Formatting shortcuts (⌘B, ⌘I, ⌘E, ⇧⌘X, ⇧⌘7/8/9), ahead of the default keymap's ⌘I.
const formatKeymap = COMMANDS.filter((c) => c.key).map((c) => ({ key: c.key!, run: c.run, preventDefault: true }));

export interface EditorHandle {
  focus(): void;
  focusEnd(): void;
  setContent(text: string): void;
  getContent(): string;
  /** Save image files and insert them at the cursor, or at the end of the note (a drop beside or below the text). */
  insertImages(files: File[], at?: "cursor" | "end"): void;
  /** The bottom of the text on screen, for telling a drop below it from one beside it. */
  contentBottom(): number | null;
  /** Run one of `COMMANDS` by id (the format toolbar's tap alternative to the right-click menu). */
  runCommand(id: string): void;
}

interface Props extends ClipboardOptions {
  initial: string;
  onChange: (text: string) => void;
  placeholder?: string;
  autoFocus?: boolean;
  resolveAsset?: (src: string) => string;
  openLink?: (url: string) => void;
  ref?: Ref<EditorHandle>;
}

const highlight = HighlightStyle.define([
  { tag: t.strong, fontWeight: "700" },
  { tag: t.emphasis, fontStyle: "italic" },
  { tag: t.strikethrough, textDecoration: "line-through" },
  { tag: [t.processingInstruction, t.contentSeparator], class: "cm-md-syntax" },
  { tag: t.url, class: "cm-md-url" },
  { tag: t.monospace, class: "cm-md-mono" },
  { tag: [t.keyword, t.operatorKeyword, t.modifier], class: "tok-keyword" },
  { tag: [t.string, t.special(t.string), t.regexp], class: "tok-string" },
  { tag: [t.comment, t.lineComment, t.blockComment], class: "tok-comment" },
  { tag: [t.number, t.bool, t.null, t.atom], class: "tok-number" },
  { tag: [t.function(t.variableName), t.definition(t.variableName), t.className, t.typeName], class: "tok-def" },
]);

const theme = EditorView.theme({
  "&": { backgroundColor: "transparent", color: "var(--fg)" },
  "&.cm-focused": { outline: "none" },
  ".cm-content": { caretColor: "var(--accent)", padding: "0 0 1.5rem 0", fontFamily: "var(--font-text)" },
  ".cm-line": { padding: "0" },
  ".cm-cursor": { borderLeftColor: "var(--accent)", borderLeftWidth: "2px" },
  ".cm-placeholder": { color: "var(--fg-faint)", fontStyle: "normal" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--selection) !important",
  },
});

export function Editor({ initial, onChange, placeholder, autoFocus, resolveAsset, openLink, saveAttachment, pasteClipboardImage, onError, ref }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;
  const onErrorRef = useRef(onError);
  onErrorRef.current = onError;
  // Set while we replace content programmatically, so it isn't echoed as a user edit.
  const silent = useRef(false);

  useImperativeHandle(ref, () => ({
    focus: () => view.current?.focus(),
    focusEnd: () => {
      const v = view.current;
      if (!v) return;
      cursorToEnd(v);
      v.focus();
    },
    getContent: () => view.current?.state.doc.toString() ?? "",
    insertImages: (files: File[], at = "cursor") => {
      const v = view.current;
      if (!v || !files.length) return;
      const pos = at === "end" ? v.state.doc.length : null;
      void saveImages(v, files, { saveAttachment, onError: (m) => onErrorRef.current?.(m) }, pos);
    },
    contentBottom: () => view.current?.contentDOM.getBoundingClientRect().bottom ?? null,
    runCommand: (id: string) => {
      const v = view.current;
      const c = commandById(id);
      if (v && c) c.run(v);
    },
    setContent: (text: string) => {
      const v = view.current;
      if (!v || v.state.doc.toString() === text) return;
      silent.current = true;
      const head = Math.min(v.state.selection.main.head, text.length);
      v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: text }, selection: { anchor: head } });
      silent.current = false;
    },
  }));

  useEffect(() => {
    const state = EditorState.create({
      doc: initial,
      extensions: [
        history(),
        drawSelection(),
        EditorView.lineWrapping,
        EditorView.contentAttributes.of({ spellcheck: "true", autocorrect: "on", autocapitalize: "sentences" }),
        markdown({ base: markdownLanguage, codeLanguages: languages, addKeymap: true }),
        syntaxHighlighting(highlight),
        slashMenu,
        keymap.of([...formatKeymap, ...markdownKeymap, ...defaultKeymap, ...historyKeymap, indentWithTab]),
        livePreview,
        blockWidgets,
        markdownClipboard({ saveAttachment, pasteClipboardImage, onError: (m) => onErrorRef.current?.(m) }),
        imagePicker.of((v) => pickImages(v, { saveAttachment, onError: (m) => onErrorRef.current?.(m) })),
        // On a touch device there's no right-click to replace: leave the
        // webview's own text-selection menu (Copy/Paste) in place, and let
        // the format toolbar (src/components/FormatToolbar.tsx) stand in for
        // the rest (see docs/MOBILE.md § Phone layout).
        ...(isTauri && !isCoarsePointer ? [nativeContextMenu()] : []),
        assetResolver.of(resolveAsset ?? ((s) => s)),
        ...(openLink ? [linkOpener.of(openLink)] : []),
        placeholderExt(placeholder ?? ""),
        theme,
        EditorView.updateListener.of((u) => {
          if (u.docChanged && !silent.current) onChangeRef.current(u.state.doc.toString());
        }),
      ],
    });
    const v = new EditorView({ state, parent: host.current! });
    view.current = v;
    if (autoFocus) {
      v.focus();
      v.dispatch({ selection: { anchor: v.state.doc.length } });
    }
    return () => {
      v.destroy();
      view.current = null;
    };
    // The editor is created once per mount; parents remount it (via `key`) to switch notes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return <div ref={host} className="noughtpad-editor" />;
}
