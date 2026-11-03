import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarDays, Cpu, FilePlus2, FolderOpen, MessageSquareText, Notebook, PanelLeft, PenLine, Search, Sprout, Waypoints, X } from "lucide-react";
import { api, isApiError, pickFolder, type VaultInfo } from "./lib/api";
import { useEvent } from "./lib/useEvent";
import { NotePane } from "./components/NotePane";
import { Stream } from "./components/Stream";
import { SearchPalette } from "./components/SearchPalette";
import { RelatedPanel } from "./components/RelatedPanel";
import { describeStatus, useSemanticStatus } from "./lib/useSemantic";
import { Journal } from "./views/Journal";
import { IdeasView, ThemeView } from "./views/Ideas";
import { ThinkView, type ScopeChoice } from "./views/Think";
import { ModelsView } from "./views/Models";
import type { EditorHandle } from "./editor/Editor";

/** "write" is the blank page; "notes" lists all notes (optionally one collection). */
type View =
  | { kind: "write" }
  | { kind: "notes"; label?: string }
  | { kind: "journal" }
  | { kind: "ideas" }
  | { kind: "theme"; id: string }
  | { kind: "think"; scope?: ScopeChoice }
  | { kind: "models"; focus?: "local" | "remote" };

export function App() {
  const [vault, setVault] = useState<VaultInfo | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.vaultInfo().then(setVault, (e) => setError(isApiError(e) ? e.message : String(e)));
  }, []);

  if (vault === undefined) return <div className="app-loading" />;
  if (vault === null) return <Welcome onOpened={setVault} error={error} setError={setError} />;
  return <Workspace vault={vault} onSwitchVault={() => setVault(null)} />;
}

function Welcome({ onOpened, error, setError }: { onOpened: (v: VaultInfo) => void; error: string | null; setError: (e: string | null) => void }) {
  const { data: defaultPath } = useQuery({ queryKey: ["defaultVault"], queryFn: api.defaultVault });
  const open = async (path: string | null) => {
    if (!path) return;
    try {
      onOpened(await api.openVault(path));
    } catch (e) {
      setError(isApiError(e) ? e.message : String(e));
    }
  };
  return (
    <main className="welcome">
      <h1>Nought</h1>
      <p className="welcome-lede">A blank page that quietly remembers. Your notes are plain Markdown files in a folder you choose.</p>
      <div className="welcome-actions">
        <button className="btn btn-primary" onClick={() => open(defaultPath ?? null)}>
          Start in {defaultPath ?? "~/Documents/Nought"}
        </button>
        <button className="btn" onClick={async () => open(await pickFolder())}>
          <FolderOpen size={16} aria-hidden /> Choose another folder…
        </button>
      </div>
      {error && <p className="error">{error}</p>}
    </main>
  );
}

function Workspace({ vault, onSwitchVault }: { vault: VaultInfo; onSwitchVault: () => void }) {
  const qc = useQueryClient();
  const [view, setView] = useState<View>({ kind: "write" });
  // `paneKey` remounts the editor; `currentId` tracks which note it shows.
  const [paneKey, setPaneKey] = useState("new-0");
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [chromePinned, setChromePinned] = useState(() => readPref("nought.chrome") === "pinned");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [toast, setToast] = useState<{ text: string; undo?: () => void } | null>(null);
  const setNotice = useCallback((text: string | null, undo?: () => void) => setToast(text ? { text, undo } : null), []);
  // "Explore this note": the related-notes panel, opened on demand only.
  const [relatedOpen, setRelatedOpen] = useState(false);
  const semantic = useSemanticStatus();
  const semanticLine = describeStatus(semantic);
  const editorRef = useRef<EditorHandle | null>(null);
  const newSeq = useRef(0);

  const { data: labels = [] } = useQuery({ queryKey: ["notes", "labels"], queryFn: api.labels });

  // The scope Think was last opened with (`undefined` until first opened).
  const [thinkScope, setThinkScope] = useState<ScopeChoice | null | undefined>(undefined);
  useEffect(() => {
    if (view.kind === "think") setThinkScope((s) => view.scope ?? s ?? null);
    // Pages other than Write start at the top.
    if (view.kind !== "write") window.scrollTo({ top: 0 });
  }, [view]);

  // Optional: show the note list under the editor (off by default: the page stays blank).
  const [listUnderEditor, setListUnderEditor] = useState(() => readPref("nought.listUnderEditor") === "on");
  // A new note started from a collection gets that collection's label.
  const [presetLabel, setPresetLabel] = useState<string | undefined>(undefined);

  const openNote = useCallback((id: string) => {
    setView({ kind: "write" });
    setCurrentId(id);
    setPaneKey(id);
    window.scrollTo({ top: 0 });
  }, []);

  const newNote = useCallback(() => {
    setView((v) => {
      setPresetLabel(v.kind === "notes" ? v.label : undefined);
      return { kind: "write" };
    });
    setCurrentId(null);
    setPaneKey(`new-${++newSeq.current}`);
    window.scrollTo({ top: 0 });
  }, []);

  const toggleListUnderEditor = useCallback(() => {
    setListUnderEditor((on) => {
      writePref("nought.listUnderEditor", on ? "off" : "on");
      return !on;
    });
  }, []);

  const toggleChrome = useCallback(() => {
    setChromePinned((p) => {
      writePref("nought.chrome", p ? "auto" : "pinned");
      return !p;
    });
  }, []);

  const showLabel = useCallback((label: string) => {
    setView({ kind: "notes", label });
    window.scrollTo({ top: 0 });
  }, []);

  // "Explore this note": think with the note and the notes related to it.
  const askAboutNote = useCallback(async (id: string) => {
    let label = "This note";
    try {
      const doc = await api.getNote(id);
      const first = (doc.title ?? doc.content.split("\n").find((l) => l.trim()) ?? "").replace(/^[#>\-*+\s]+/, "").trim();
      if (first) label = first.length > 40 ? `${first.slice(0, 40)}…` : first;
    } catch {
      /* keep the generic label */
    }
    setRelatedOpen(false);
    setView({ kind: "think", scope: { scope: { kind: "note", id }, label } });
  }, []);

  const rebuildIndex = useCallback(async () => {
    const r = await api.rebuildIndex();
    setNotice(`Index rebuilt: ${r.indexed} notes.`);
    void qc.invalidateQueries({ queryKey: ["notes"] });
  }, [qc]);

  // Report time-to-capture once the first editor has painted.
  useEffect(() => {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        void api.markReady().then((ms) => console.info(`[nought] editor ready ${ms} ms after start`));
      }),
    );
  }, []);

  // Keyboard shortcuts.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((o) => !o);
      } else if (mod && e.key.toLowerCase() === "n" && !e.shiftKey) {
        e.preventDefault();
        newNote();
      } else if (mod && e.key === "\\") {
        e.preventDefault();
        toggleChrome();
      } else if (mod && e.key.toLowerCase() === "j") {
        e.preventDefault();
        setView({ kind: "write" });
        setRelatedOpen((o) => !o);
      } else if (e.key === "Escape" && relatedOpen && !paletteOpen) {
        setRelatedOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [newNote, toggleChrome, relatedOpen, paletteOpen]);

  // Backend events: external file changes and the global quick-capture shortcut.
  useEvent("vault-changed", () => void qc.invalidateQueries({ queryKey: ["notes"] }));
  useEvent("quick-capture", () => newNote());

  // While typing, chrome stays hidden even if the pointer rests on it; moving the mouse brings it back.
  const [typing, setTyping] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const inEditor = (e.target as HTMLElement | null)?.closest?.(".cm-content");
      if (inEditor && !e.metaKey && !e.ctrlKey && e.key.length === 1) setTyping(true);
    };
    const onMove = () => setTyping(false);
    window.addEventListener("keydown", onKey);
    window.addEventListener("mousemove", onMove);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousemove", onMove);
    };
  }, []);

  useEffect(() => {
    if (!toast) return;
    const t = setTimeout(() => setNotice(null), toast?.undo ? 8000 : 5000);
    return () => clearTimeout(t);
  }, [toast, setNotice]);

  const label = view.kind === "notes" ? view.label : undefined;

  return (
    <div
      className={`app ${chromePinned ? "chrome-pinned" : "chrome-auto"} ${typing ? "is-typing" : ""} ${relatedOpen && view.kind === "write" ? "related-open" : ""}`}
    >
      <div className="sidebar-hotzone" aria-hidden />
      <aside className="sidebar chrome" aria-label="Navigation">
        <nav className="sidebar-section">
          <SideItem icon={<PenLine size={15} />} label="Write" active={view.kind === "write"} onClick={() => setView({ kind: "write" })} />
          <SideItem icon={<Notebook size={15} />} label="All notes" active={view.kind === "notes" && !label} onClick={() => setView({ kind: "notes" })} />
          <SideItem icon={<CalendarDays size={15} />} label="Journal" active={view.kind === "journal"} onClick={() => setView({ kind: "journal" })} />
          <SideItem
            icon={<Sprout size={15} />}
            label="Ideas over time"
            active={view.kind === "ideas" || view.kind === "theme"}
            onClick={() => setView({ kind: "ideas" })}
          />
          <SideItem icon={<MessageSquareText size={15} />} label="Think" active={view.kind === "think"} onClick={() => setView({ kind: "think" })} />
        </nav>
        {labels.length > 0 && (
          <nav className="sidebar-section" aria-label="Collections">
            <h4 className="sidebar-heading">Collections</h4>
            {labels.map((l) => (
              <SideItem key={l.label} label={l.label} count={l.count} active={label?.toLowerCase() === l.label.toLowerCase()} onClick={() => showLabel(l.label)} />
            ))}
          </nav>
        )}
        <div className="sidebar-footer">
          {semanticLine && <p className="sidebar-status">{semanticLine}</p>}
          <button className={`vault-path ${view.kind === "models" ? "is-active" : ""}`} onClick={() => setView({ kind: "models" })}>
            <Cpu size={13} aria-hidden /> Models
          </button>
          <button className="vault-path" title={`${vault.path}\nClick to switch folder`} onClick={onSwitchVault}>
            <FolderOpen size={13} aria-hidden /> {vault.path.split(/[\\/]/).pop()}
          </button>
        </div>
      </aside>

      <div className="topbar chrome">
        <button className="icon-btn" onClick={toggleChrome} title="Toggle sidebar (⌘\)" aria-label="Toggle sidebar">
          <PanelLeft size={16} />
        </button>
        <div className="topbar-spacer" />
        <button className="icon-btn" onClick={() => setPaletteOpen(true)} title="Search (⌘K)" aria-label="Search">
          <Search size={16} />
        </button>
        <button
          className={`icon-btn ${relatedOpen && view.kind === "write" ? "is-on" : ""}`}
          onClick={() => {
            setView({ kind: "write" });
            setRelatedOpen((o) => !o);
          }}
          title="Related notes (⌘J)"
          aria-label="Related notes"
          aria-pressed={relatedOpen && view.kind === "write"}
        >
          <Waypoints size={16} />
        </button>
        <button className="icon-btn" onClick={newNote} title="New note (⌘N)" aria-label="New note">
          <FilePlus2 size={16} />
        </button>
      </div>

      <main
        className="main"
        onMouseDown={(e) => {
          // On the writing page, clicking empty space puts the cursor at the end, like paper.
          const t = e.target as HTMLElement;
          if (view.kind === "write" && (t === e.currentTarget || t.classList.contains("note-pane") || t.dataset.page === "write")) {
            e.preventDefault();
            editorRef.current?.focusEnd();
          }
        }}
      >
        {/* The editor stays mounted while other views are open, so cursor and undo history survive. */}
        <div hidden={view.kind !== "write"} data-page="write">
          <NotePane
            key={paneKey}
            noteId={currentId ?? (paneKey.startsWith("new-") ? null : paneKey)}
            vaultRoot={vault.path}
            presetLabels={presetLabel ? [presetLabel] : undefined}
            onCreated={setCurrentId}
            onTrashed={newNote}
            onNotice={setNotice}
            editorRef={editorRef}
          />
          {listUnderEditor && <Stream query={{}} currentId={currentId} onOpen={openNote} onLabel={showLabel} />}
        </div>
        {view.kind === "notes" && (
          <>
            <div className="notes-head">
              <h2 className="journal-heading">{label ?? "All notes"}</h2>
              {label && (
                <button className="icon-btn" onClick={() => setView({ kind: "notes" })} title="Show all notes" aria-label="Show all notes">
                  <X size={15} />
                </button>
              )}
            </div>
            <Stream query={{ label }} currentId={currentId} onOpen={openNote} onLabel={showLabel} />
          </>
        )}
        {view.kind === "journal" && <Journal onOpen={openNote} onLabel={showLabel} />}
        {view.kind === "ideas" && <IdeasView onOpenNote={openNote} onOpenTheme={(id) => setView({ kind: "theme", id })} />}
        {view.kind === "theme" && (
          <ThemeView
            id={view.id}
            onBack={() => setView({ kind: "ideas" })}
            onOpenNote={openNote}
            onAsk={(name) => setView({ kind: "think", scope: { scope: { kind: "theme", id: view.id }, label: name } })}
          />
        )}
        {/* Like the editor, Think stays mounted once opened so this session's answers survive navigation. */}
        {thinkScope !== undefined && (
          <div hidden={view.kind !== "think"}>
            <ThinkView initial={thinkScope ?? undefined} onOpenNote={openNote} onModels={(focus) => setView({ kind: "models", focus })} />
          </div>
        )}
        {view.kind === "models" && <ModelsView focus={view.focus} />}
      </main>

      <SearchPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        openNote={openNote}
        newNote={newNote}
        showNotes={() => setView({ kind: "notes" })}
        listUnderEditor={listUnderEditor}
        toggleListUnderEditor={toggleListUnderEditor}
        showJournal={() => setView({ kind: "journal" })}
        showIdeas={() => setView({ kind: "ideas" })}
        showThink={() => setView({ kind: "think" })}
        showModels={() => setView({ kind: "models" })}
        askAboutNote={currentId ? () => void askAboutNote(currentId) : undefined}
        showLabel={showLabel}
        toggleChrome={toggleChrome}
        rebuildIndex={() => void rebuildIndex()}
        toggleRelated={() => {
          setView({ kind: "write" });
          setRelatedOpen((o) => !o);
        }}
        resetRelatedFeedback={() =>
          void api.resetRelatedFeedback().then(() => {
            setNotice("Related notes are back to their defaults.");
            void qc.invalidateQueries({ queryKey: ["related"] });
          })
        }
        semanticOn={semantic.state !== "off"}
        toggleSemantic={() =>
          void (semantic.state === "off" ? api.enableSemantic() : api.disableSemantic()).then(() =>
            qc.invalidateQueries({ queryKey: ["semantic-status"] }),
          )
        }
      />
      {relatedOpen && view.kind === "write" && (
        <RelatedPanel
          noteId={currentId}
          onOpen={openNote}
          onOpenTheme={(id) => setView({ kind: "theme", id })}
          onAsk={currentId ? () => void askAboutNote(currentId) : undefined}
          onClose={() => setRelatedOpen(false)}
          onNotice={setNotice}
        />
      )}
      {toast && (
        <div className="toast" role="status">
          {toast.text}
          {toast.undo && (
            <button
              className="toast-undo"
              onClick={() => {
                toast.undo?.();
                setNotice(null);
              }}
            >
              Undo
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function readPref(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function writePref(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* storage unavailable: the preference just won't persist */
  }
}

function SideItem({ icon, label, count, active, onClick }: { icon?: React.ReactNode; label: string; count?: number; active?: boolean; onClick: () => void }) {
  return (
    <button className={`side-item ${active ? "is-active" : ""}`} onClick={onClick}>
      {icon ?? <span className="side-dot" aria-hidden />}
      <span className="side-label">{label}</span>
      {count !== undefined && <span className="side-count">{count}</span>}
    </button>
  );
}
