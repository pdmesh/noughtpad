import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Trash2 } from "lucide-react";
import { useEvent } from "../lib/useEvent";
import { api, assetUrl, isApiError, mayHaveChanged, openExternal, type Change, type EditableMeta, type NoteDoc } from "../lib/api";
import { Editor, type EditorHandle } from "../editor/Editor";
import { MetaStrip } from "./MetaStrip";
import { formatDateTime } from "../lib/format";

const SAVE_DELAY_MS = 500;
const emptyMeta: EditableMeta = { title: null, labels: [], source: null, kindOverride: null };

interface Props {
  /** Existing note to open, or null for a fresh blank page. */
  noteId: string | null;
  vaultRoot: string;
  /** Labels to pre-fill on a new note (e.g. when a collection is open). */
  presetLabels?: string[];
  onCreated?: (id: string) => void;
  onTrashed?: () => void;
  onNotice?: (msg: string) => void;
  editorRef?: React.RefObject<EditorHandle | null>;
}

/**
 * One note: optional metadata strip + the editor, with debounced autosave.
 * Saving is serialised; the first save of a blank page creates the file.
 */
export function NotePane({ noteId, vaultRoot, presetLabels, onCreated, onTrashed, onNotice, editorRef }: Props) {
  const qc = useQueryClient();
  const [doc, setDoc] = useState<NoteDoc | null>(null);
  const [loaded, setLoaded] = useState(noteId === null);
  const [meta, setMeta] = useState<EditableMeta>({ ...emptyMeta, labels: presetLabels ?? [] });
  const [status, setStatus] = useState<"idle" | "pending" | "saving" | "saved" | "error">("idle");

  const localRef = useRef<EditorHandle | null>(null);
  const editor = editorRef ?? localRef;
  const docRef = useRef<NoteDoc | null>(null);
  const content = useRef("");
  const metaRef = useRef(meta);
  const dirty = useRef({ content: false, meta: false });
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const chain = useRef<Promise<void>>(Promise.resolve());

  // ---- load ---------------------------------------------------------------
  useEffect(() => {
    if (noteId === null) return;
    let cancelled = false;
    api.getNote(noteId).then(
      (d) => {
        if (cancelled) return;
        docRef.current = d;
        content.current = d.content;
        const m = { title: d.title, labels: d.labels, source: d.source, kindOverride: d.kindOverride };
        metaRef.current = m;
        setDoc(d);
        setMeta(m);
        setLoaded(true);
      },
      (e) => onNotice?.(isApiError(e) ? e.message : String(e)),
    );
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [noteId]);

  // ---- save ---------------------------------------------------------------
  const doSave = useCallback(async () => {
    const { content: cDirty, meta: mDirty } = dirty.current;
    if (!cDirty && !mDirty) return;
    dirty.current = { content: false, meta: false };
    const current = docRef.current;
    const text = content.current;
    const m = metaRef.current;
    setStatus("saving");
    try {
      let next: NoteDoc;
      if (!current) {
        // Nothing written yet: don't create files for an empty page.
        if (!text.trim()) {
          setStatus("idle");
          return;
        }
        next = await api.createNote(text, m);
        onCreated?.(next.id);
      } else {
        next = await api.updateNote(current.id, {
          content: cDirty ? text : undefined,
          meta: mDirty ? m : undefined,
          baseHash: current.hash,
        });
      }
      docRef.current = next;
      setDoc(next);
      setStatus(dirty.current.content || dirty.current.meta ? "pending" : "saved");
      void qc.invalidateQueries({ queryKey: ["notes"] });
    } catch (e) {
      if (isApiError(e) && e.kind === "conflict" && current) {
        // The file changed on disk (another app, sync). Disk wins; tell the user.
        const fresh = await api.getNote(current.id);
        docRef.current = fresh;
        content.current = fresh.content;
        editor.current?.setContent(fresh.content);
        setDoc(fresh);
        onNotice?.("This note changed on disk, so the latest version was loaded.");
        setStatus("idle");
      } else {
        dirty.current = { content: dirty.current.content || cDirty, meta: dirty.current.meta || mDirty };
        setStatus("error");
        onNotice?.(`Couldn't save: ${isApiError(e) ? e.message : String(e)}`);
      }
    }
  }, [qc, onCreated, onNotice, editor]);

  const flush = useCallback(() => {
    if (timer.current) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    chain.current = chain.current.then(doSave);
    return chain.current;
  }, [doSave]);

  const schedule = useCallback(() => {
    setStatus("pending");
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(flush, SAVE_DELAY_MS);
  }, [flush]);

  // Flush on unmount (switching notes) and when the window is hidden/closed.
  useEffect(() => {
    const onHide = () => void flush();
    window.addEventListener("blur", onHide);
    window.addEventListener("beforeunload", onHide);
    return () => {
      window.removeEventListener("blur", onHide);
      window.removeEventListener("beforeunload", onHide);
      void flush();
    };
  }, [flush]);

  // Reload if this note was edited elsewhere (another app, or another device
  // through sync) and we have no pending edits.
  useEvent<Change[]>("vault-changed", async (changes) => {
    const cur = docRef.current;
    if (!cur || dirty.current.content || dirty.current.meta) return;
    if (!mayHaveChanged(changes, cur.id)) return;
    const fresh = await api.getNote(cur.id).catch(() => null);
    // Typed meanwhile, switched notes, or nothing new: leave the editor alone.
    if (!fresh || docRef.current !== cur || dirty.current.content || dirty.current.meta) return;
    if (fresh.hash === cur.hash) return;
    docRef.current = fresh;
    content.current = fresh.content;
    editor.current?.setContent(fresh.content);
    const m = { title: fresh.title, labels: fresh.labels, source: fresh.source, kindOverride: fresh.kindOverride };
    metaRef.current = m;
    setMeta(m);
    setDoc(fresh);
  });

  const onContent = useCallback(
    (text: string) => {
      content.current = text;
      dirty.current.content = true;
      schedule();
    },
    [schedule],
  );

  const onMeta = useCallback(
    (m: EditableMeta) => {
      metaRef.current = m;
      setMeta(m);
      dirty.current.meta = true;
      // A new note with only metadata isn't worth a file yet; it'll be saved with the first words.
      if (docRef.current || content.current.trim()) schedule();
    },
    [schedule],
  );

  const trash = async () => {
    const cur = docRef.current;
    if (!cur) return;
    if (!window.confirm("Move this note to the trash? It stays recoverable in .noughtpad/trash.")) return;
    if (timer.current) clearTimeout(timer.current);
    dirty.current = { content: false, meta: false };
    await api.trashNote(cur.id);
    void qc.invalidateQueries({ queryKey: ["notes"] });
    onTrashed?.();
  };

  if (!loaded) return <div className="note-pane" aria-busy="true" />;

  return (
    <article className="note-pane">
      <header className="note-header">
        <MetaStrip meta={meta} onChange={onMeta} />
        <div className="note-header-right chrome">
          {doc && <span className="note-date" title={doc.path}>{formatDateTime(doc.created)}</span>}
          <span className={`save-dot save-${status}`} title={statusLabel(status)} aria-label={statusLabel(status)} />
          {doc && (
            <button className="icon-btn" onClick={trash} title="Move to trash" aria-label="Move to trash">
              <Trash2 size={15} />
            </button>
          )}
        </div>
      </header>
      {/* Optional title: invisible until the top of the note is hovered, unless one is set. */}
      <input
        className={`note-title ${meta.title ? "has-title" : "no-title"}`}
        value={meta.title ?? ""}
        placeholder="Title"
        aria-label="Title (optional)"
        onChange={(e) => onMeta({ ...meta, title: e.target.value || null })}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === "ArrowDown") {
            e.preventDefault();
            editor.current?.focus();
          }
        }}
      />
      <Editor
        ref={editor}
        initial={doc?.content ?? ""}
        onChange={onContent}
        autoFocus
        placeholder={noteId ? "" : "Write anything…"}
        resolveAsset={(src) => assetUrl(vaultRoot, src)}
        openLink={(url) => void openExternal(url)}
        saveAttachment={api.saveAttachment}
      />
    </article>
  );
}

function statusLabel(s: string) {
  return { idle: "", pending: "Unsaved changes", saving: "Saving…", saved: "Saved", error: "Save failed" }[s] ?? "";
}
