import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarDays, FilePlus2, FolderOpen, Notebook, PanelLeft, Search, X } from "lucide-react";
import { api, isApiError, onEvent, pickFolder, type VaultInfo } from "./lib/api";
import { NotePane } from "./components/NotePane";
import { Stream } from "./components/Stream";
import { SearchPalette } from "./components/SearchPalette";
import { Journal } from "./views/Journal";
import type { EditorHandle } from "./editor/Editor";

type View = { kind: "notebook"; label?: string } | { kind: "journal" };

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
  const [view, setView] = useState<View>({ kind: "notebook" });
  // `paneKey` remounts the editor; `currentId` tracks which note it shows.
  const [paneKey, setPaneKey] = useState("new-0");
  const [currentId, setCurrentId] = useState<string | null>(null);
  const [chromePinned, setChromePinned] = useState(() => localStorage.getItem("nought.chrome") === "pinned");
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const editorRef = useRef<EditorHandle | null>(null);
  const newSeq = useRef(0);

  const { data: labels = [] } = useQuery({ queryKey: ["notes", "labels"], queryFn: api.labels });

  const openNote = useCallback((id: string) => {
    setView((v) => (v.kind === "notebook" ? v : { kind: "notebook" }));
    setCurrentId(id);
    setPaneKey(id);
    window.scrollTo({ top: 0, behavior: "smooth" });
  }, []);

  const newNote = useCallback(() => {
    setView((v) => (v.kind === "notebook" ? v : { kind: "notebook" }));
    setCurrentId(null);
    setPaneKey(`new-${++newSeq.current}`);
    window.scrollTo({ top: 0 });
  }, []);

  const toggleChrome = useCallback(() => {
    setChromePinned((p) => {
      try {
        localStorage.setItem("nought.chrome", p ? "auto" : "pinned");
      } catch {
        /* storage unavailable: setting just won't persist */
      }
      return !p;
    });
  }, []);

  const showLabel = useCallback((label: string) => {
    setView({ kind: "notebook", label });
    window.scrollTo({ top: 0 });
  }, []);

  const rebuildIndex = useCallback(async () => {
    const r = await api.rebuildIndex();
    setNotice(`Index rebuilt: ${r.indexed} notes.`);
    void qc.invalidateQueries({ queryKey: ["notes"] });
  }, [qc]);

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
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [newNote, toggleChrome]);

  // Backend events: external file changes and the global quick-capture shortcut.
  useEffect(() => {
    const offs: Array<() => void> = [];
    void onEvent("vault-changed", () => qc.invalidateQueries({ queryKey: ["notes"] })).then((u) => offs.push(u));
    void onEvent("quick-capture", () => newNote()).then((u) => offs.push(u));
    return () => offs.forEach((f) => f());
  }, [qc, newNote]);

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
    if (!notice) return;
    const t = setTimeout(() => setNotice(null), 5000);
    return () => clearTimeout(t);
  }, [notice]);

  const label = view.kind === "notebook" ? view.label : undefined;

  return (
    <div className={`app ${chromePinned ? "chrome-pinned" : "chrome-auto"} ${typing ? "is-typing" : ""}`}>
      <div className="sidebar-hotzone" aria-hidden />
      <aside className="sidebar chrome" aria-label="Navigation">
        <nav className="sidebar-section">
          <SideItem icon={<Notebook size={15} />} label="Notebook" active={view.kind === "notebook" && !label} onClick={() => setView({ kind: "notebook" })} />
          <SideItem icon={<CalendarDays size={15} />} label="Journal" active={view.kind === "journal"} onClick={() => setView({ kind: "journal" })} />
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
        <button className="icon-btn" onClick={newNote} title="New note (⌘N)" aria-label="New note">
          <FilePlus2 size={16} />
        </button>
      </div>

      <main className="main">
        {view.kind === "notebook" && (
          <>
            <NotePane
              key={paneKey}
              noteId={paneKey.startsWith("new-") ? null : paneKey}
              vaultRoot={vault.path}
              presetLabels={label ? [label] : undefined}
              onCreated={setCurrentId}
              onTrashed={newNote}
              onNotice={setNotice}
              editorRef={editorRef}
            />
            {label && (
              <div className="filter-bar">
                Collection <span className="chip">{label}</span>
                <button className="icon-btn" onClick={() => setView({ kind: "notebook" })} aria-label="Clear collection filter">
                  <X size={14} />
                </button>
              </div>
            )}
            <Stream query={{ label }} currentId={currentId} onOpen={openNote} onLabel={showLabel} />
          </>
        )}
        {view.kind === "journal" && <Journal onOpen={openNote} onLabel={showLabel} />}
      </main>

      <SearchPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
        openNote={openNote}
        newNote={newNote}
        showNotebook={() => setView({ kind: "notebook" })}
        showJournal={() => setView({ kind: "journal" })}
        showLabel={showLabel}
        toggleChrome={toggleChrome}
        rebuildIndex={() => void rebuildIndex()}
      />
      {notice && (
        <div className="toast" role="status">
          {notice}
        </div>
      )}
    </div>
  );
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
