import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarDays, History, Settings as SettingsIcon, FilePlus2, FolderOpen, MessageSquareText, Notebook, PanelLeft, PenLine, Search, Sprout, Waypoints, X } from "lucide-react";
import { api, isApiError, pickFolder, type VaultInfo } from "./lib/api";
import { useEvent } from "./lib/useEvent";
import { NotePane } from "./components/NotePane";
import { Stream } from "./components/Stream";
import { SearchPalette } from "./components/SearchPalette";
import { RelatedPanel } from "./components/RelatedPanel";
import { describeStatus, useSemanticStatus } from "./lib/useSemantic";
import { ChatHistory } from "./components/ChatHistory";
import { useChatLive, type ScopeChoice } from "./lib/chat";
import type { EditorHandle } from "./editor/Editor";
import { loadMarkdownPipeline } from "./editor/clipboard";

// Pages other than writing load after the editor is ready, keeping them out of
// the startup bundle (G1: capture in < 300 ms).
const loadJournal = () => import("./views/Journal");
const loadIdeas = () => import("./views/Ideas");
const loadChat = () => import("./views/Chat");
const loadModels = () => import("./views/Models");
const loadSettings = () => import("./views/Settings");
const Journal = lazy(() => loadJournal().then((m) => ({ default: m.Journal })));
const IdeasView = lazy(() => loadIdeas().then((m) => ({ default: m.IdeasView })));
const ThemeView = lazy(() => loadIdeas().then((m) => ({ default: m.ThemeView })));
const ChatView = lazy(() => loadChat().then((m) => ({ default: m.ChatView })));
const ModelsView = lazy(() => loadModels().then((m) => ({ default: m.ModelsView })));
const SettingsView = lazy(() => loadSettings().then((m) => ({ default: m.SettingsView })));

/** Warm everything that was left out of the startup bundle, so first use is instant. */
function preloadDeferred() {
  for (const load of [loadMarkdownPipeline, loadChat, loadJournal, loadIdeas, loadSettings, loadModels]) void load();
}

/** "write" is the blank page; "notes" lists all notes (optionally one collection). */
type View =
  | { kind: "write" }
  | { kind: "notes"; label?: string }
  | { kind: "journal" }
  | { kind: "ideas" }
  | { kind: "theme"; id: string }
  | { kind: "models"; focus?: "local" | "remote" }
  | { kind: "settings" }
  /** `key` stays the same when a new chat gets its id, so the page isn't remounted. */
  | { kind: "chat"; id: string | null; start?: ScopeChoice; key: string };

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
      <h1>Noughtpad</h1>
      <p className="welcome-lede">A blank page that quietly remembers. Your notes are plain Markdown files in a folder you choose.</p>
      <div className="welcome-actions">
        <button className="btn btn-primary" onClick={() => open(defaultPath ?? null)}>
          Start in {defaultPath ?? "~/Documents/Noughtpad"}
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
  const [chromePinned, setChromePinned] = useState(() => readPref("noughtpad.chrome") === "pinned");
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

  // Chat with your notes: a page; past chats are listed in the sidebar.
  const chatLive = useChatLive();
  const openChat = useCallback((start?: ScopeChoice) => {
    setRelatedOpen(false);
    setView({ kind: "chat", id: null, start, key: `new-${Date.now()}` });
  }, []);
  const showChat = useCallback((id: string) => setView({ kind: "chat", id, key: id }), []);
  // Chat history: a right-hand panel on the chat page, open unless the user hid it.
  const [historyOpen, setHistoryOpen] = useState(() => readPref("noughtpad.chatHistory") !== "off");
  const toggleHistory = useCallback(() => {
    setHistoryOpen((o) => {
      writePref("noughtpad.chatHistory", o ? "off" : "on");
      return !o;
    });
  }, []);
  const showHistory = view.kind === "chat" && historyOpen;
  const { data: currentNoteChoice = null } = useQuery({
    queryKey: ["notes", "chat-label", currentId],
    enabled: !!currentId,
    queryFn: async (): Promise<ScopeChoice> => {
      const doc = await api.getNote(currentId!);
      const first = (doc.title ?? doc.content.split("\n").find((l) => l.trim()) ?? "").replace(/^[#>\-*+\s]+/, "").trim();
      return { scope: { kind: "note", id: doc.id }, label: first ? (first.length > 32 ? `${first.slice(0, 32)}…` : first) : "This note" };
    },
  });
  useEffect(() => {
    // Pages other than Write start at the top (a chat scrolls itself to its end).
    if (view.kind !== "write" && view.kind !== "chat") window.scrollTo({ top: 0 });
  }, [view]);

  // Optional: show the note list under the editor (off by default: the page stays blank).
  const [listUnderEditor, setListUnderEditor] = useState(() => readPref("noughtpad.listUnderEditor") === "on");
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
      writePref("noughtpad.listUnderEditor", on ? "off" : "on");
      return !on;
    });
  }, []);

  const toggleChrome = useCallback(() => {
    setChromePinned((p) => {
      writePref("noughtpad.chrome", p ? "auto" : "pinned");
      return !p;
    });
  }, []);

  const showLabel = useCallback((label: string) => {
    setView({ kind: "notes", label });
    window.scrollTo({ top: 0 });
  }, []);

  // "Explore this note": chat about the note and the notes related to it.
  const askAboutNote = useCallback(
    (id: string) => openChat(currentNoteChoice && currentNoteChoice.scope.kind === "note" && currentNoteChoice.scope.id === id ? currentNoteChoice : { scope: { kind: "note", id }, label: "This note" }),
    [openChat, currentNoteChoice],
  );

  const resetRelated = useCallback(
    () =>
      void api.resetRelatedFeedback().then(() => {
        setNotice("Related notes are back to their defaults.");
        void qc.invalidateQueries({ queryKey: ["related"] });
      }),
    [qc, setNotice],
  );

  const rebuildIndex = useCallback(async () => {
    const r = await api.rebuildIndex();
    setNotice(`Index rebuilt: ${r.indexed} notes.`);
    void qc.invalidateQueries({ queryKey: ["notes"] });
  }, [qc]);

  // Report time-to-capture once the first editor has painted.
  useEffect(() => {
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        void api.markReady().then((ms) => console.info(`[noughtpad] editor ready ${ms} ms after start`));
        setTimeout(preloadDeferred, 200);
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

  // ⌘L toggles chat. Caught before the editor, whose keymap uses Mod-L to select a line.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "l") {
        e.preventDefault();
        e.stopPropagation();
        openChat();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [openChat]);

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
      className={`app ${chromePinned ? "chrome-pinned" : "chrome-auto"} ${typing ? "is-typing" : ""} ${(relatedOpen && view.kind === "write") || showHistory ? "related-open" : ""}`}
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
          <SideItem icon={<MessageSquareText size={15} />} label="Chat" active={view.kind === "chat" && view.id === null} onClick={() => openChat()} />
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
          <button
            className={`vault-path ${view.kind === "settings" || view.kind === "models" ? "is-active" : ""}`}
            onClick={() => setView({ kind: "settings" })}
          >
            <SettingsIcon size={13} aria-hidden /> Settings
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
        {view.kind === "chat" && (
          <button
            className={`icon-btn ${historyOpen ? "is-on" : ""}`}
            onClick={toggleHistory}
            title="Chat history"
            aria-label="Chat history"
            aria-pressed={historyOpen}
          >
            <History size={16} />
          </button>
        )}
        <button
          className={`icon-btn ${view.kind === "chat" ? "is-on" : ""}`}
          onClick={() => openChat()}
          title="Chat with your notes (⌘L)"
          aria-label="Chat with your notes"
        >
          <MessageSquareText size={16} />
        </button>
        <button className="icon-btn" onClick={newNote} title="New note (⌘N)" aria-label="New note">
          <FilePlus2 size={16} />
        </button>
      </div>

      <main
        className={`main ${view.kind === "chat" ? "is-chat" : ""}`}
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
        <Suspense fallback={null}>
          {view.kind === "journal" && <Journal onOpen={openNote} onLabel={showLabel} />}
          {view.kind === "ideas" && <IdeasView onOpenNote={openNote} onOpenTheme={(id) => setView({ kind: "theme", id })} />}
          {view.kind === "theme" && (
            <ThemeView
              id={view.id}
              onBack={() => setView({ kind: "ideas" })}
              onOpenNote={openNote}
              onAsk={(name) => openChat({ scope: { kind: "theme", id: view.id }, label: name })}
            />
          )}
          {view.kind === "models" && <ModelsView focus={view.focus} />}
          {view.kind === "settings" && (
            <SettingsView
              vault={vault}
              onSwitchVault={onSwitchVault}
              chromePinned={chromePinned}
              toggleChrome={toggleChrome}
              listUnderEditor={listUnderEditor}
              toggleListUnderEditor={toggleListUnderEditor}
              onModels={() => setView({ kind: "models" })}
              onRebuildIndex={() => void rebuildIndex()}
              onResetRelated={resetRelated}
            />
          )}
          {view.kind === "chat" && (
            <ChatView
              key={view.key}
              chatId={view.id}
              start={view.start}
              currentNote={currentNoteChoice}
              live={view.id ? chatLive.live[view.id] : undefined}
              onBegin={chatLive.begin}
              onCreated={(id) => setView((v) => (v.kind === "chat" && v.id === null ? { ...v, id } : v))}
              onOpenNote={openNote}
              onModels={(focus) => setView({ kind: "models", focus })}
            />
          )}
        </Suspense>
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
        showThink={() => openChat()}
        showSettings={() => setView({ kind: "settings" })}
        showModels={() => setView({ kind: "models" })}
        askAboutNote={currentId ? () => askAboutNote(currentId) : undefined}
        showLabel={showLabel}
        toggleChrome={toggleChrome}
        rebuildIndex={() => void rebuildIndex()}
        toggleRelated={() => {
          setView({ kind: "write" });
          setRelatedOpen((o) => !o);
        }}
        resetRelatedFeedback={resetRelated}
        semanticOn={semantic.state !== "off"}
        toggleSemantic={() =>
          void (semantic.state === "off" ? api.enableSemantic() : api.disableSemantic()).then(() =>
            qc.invalidateQueries({ queryKey: ["semantic-status"] }),
          )
        }
      />
      {showHistory && (
        <ChatHistory
          currentId={view.kind === "chat" ? view.id : null}
          running={new Set(Object.keys(chatLive.live))}
          onOpen={showChat}
          onNew={() => openChat()}
          onDeleted={(id) => {
            if (view.kind === "chat" && view.id === id) openChat();
          }}
          onClose={toggleHistory}
        />
      )}
      {relatedOpen && view.kind === "write" && (
        <RelatedPanel
          noteId={currentId}
          onOpen={openNote}
          onOpenTheme={(id) => setView({ kind: "theme", id })}
          onAsk={currentId ? () => askAboutNote(currentId) : undefined}
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
