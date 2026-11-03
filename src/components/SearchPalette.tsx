import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, CalendarDays, Cpu, FilePlus2, MessageSquareText, Notebook, PanelLeft, RefreshCw, RotateCcw, Rows3, Sparkle, Sprout, Tag, Waypoints } from "lucide-react";
import { api } from "../lib/api";
import { formatDay, plainExcerpt, snippetParts } from "../lib/format";

export interface PaletteActions {
  openNote: (id: string) => void;
  newNote: () => void;
  showNotes: () => void;
  listUnderEditor: boolean;
  toggleListUnderEditor: () => void;
  showJournal: () => void;
  showIdeas: () => void;
  showThink: () => void;
  showModels: () => void;
  /** Present when a note is open. */
  askAboutNote?: () => void;
  showLabel: (label: string) => void;
  toggleChrome: () => void;
  rebuildIndex: () => void;
  toggleRelated: () => void;
  resetRelatedFeedback: () => void;
  semanticOn: boolean;
  toggleSemantic: () => void;
}

interface Props extends PaletteActions {
  open: boolean;
  onClose: () => void;
}

function useDebounced<T>(value: T, ms: number): T {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

/** ⌘K: full-text search over the notebook, plus a few commands. */
export function SearchPalette({ open, onClose, ...actions }: Props) {
  const [input, setInput] = useState("");
  const query = useDebounced(input.trim() ? input : "", 120);
  const { data: hits = [], isFetching } = useQuery({
    queryKey: ["notes", "search", query],
    queryFn: () => api.search(query, 40),
    enabled: open && query.length > 0,
    placeholderData: (prev) => prev,
  });
  const { data: labels = [] } = useQuery({ queryKey: ["notes", "labels"], queryFn: api.labels, enabled: open });

  useEffect(() => {
    if (open) setInput("");
  }, [open]);

  const commands: Array<{ label: string; icon: React.ReactNode; hint?: string; run: () => void }> = [
    { label: "New note", icon: <FilePlus2 size={14} />, hint: "⌘N", run: actions.newNote },
    { label: "All notes", icon: <Notebook size={14} />, run: actions.showNotes },
    { label: "Journal", icon: <CalendarDays size={14} />, run: actions.showJournal },
    { label: "Ideas over time", icon: <Sprout size={14} />, run: actions.showIdeas },
    { label: "Chat with your notes", icon: <MessageSquareText size={14} />, hint: "⌘L", run: actions.showThink },
    ...(actions.askAboutNote ? [{ label: "Ask about this note", icon: <MessageSquareText size={14} />, run: actions.askAboutNote }] : []),
    { label: "Related notes", icon: <Waypoints size={14} />, hint: "⌘J", run: actions.toggleRelated },
    { label: "Toggle sidebar & toolbar", icon: <PanelLeft size={14} />, hint: "⌘\\", run: actions.toggleChrome },
    {
      label: actions.listUnderEditor ? "Hide note list under the editor" : "Show note list under the editor",
      icon: <Rows3 size={14} />,
      run: actions.toggleListUnderEditor,
    },
    {
      label: actions.semanticOn ? "Turn off search by meaning (local model)" : "Turn on search by meaning (local model)",
      icon: <Sparkle size={14} />,
      run: actions.toggleSemantic,
    },
    { label: "Models (language models for thinking)", icon: <Cpu size={14} />, run: actions.showModels },
    { label: "Rebuild search index", icon: <RefreshCw size={14} />, run: actions.rebuildIndex },
    { label: "Reset what related notes learned", icon: <RotateCcw size={14} />, run: actions.resetRelatedFeedback },
  ];
  // With a query, commands whose name contains every typed word come first.
  const words = input.toLowerCase().split(/\s+/).filter(Boolean);
  const matchingCommands = words.length
    ? commands.filter((c) => words.every((w) => c.label.toLowerCase().includes(w)))
    : commands;

  // Results arrive asynchronously; keep the top item selected so Enter opens it.
  const [selected, setSelected] = useState("");
  const first = (words.length ? matchingCommands[0]?.label : undefined) ?? (query ? hits[0]?.note.id : undefined);
  useEffect(() => {
    setSelected(first ?? "New note");
  }, [first, query]);

  if (!open) return null;
  const run = (fn: () => void) => () => {
    onClose();
    fn();
  };
  const lower = input.toLowerCase().replace(/^#/, "");

  return (
    <div className="palette-backdrop" onMouseDown={onClose}>
      <Command
        className="palette"
        label="Search notes"
        shouldFilter={false}
        value={selected}
        onValueChange={setSelected}
        onMouseDown={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
      >
        <Command.Input
          autoFocus
          value={input}
          onValueChange={setInput}
          placeholder="Search notes…   (#label, type:personal, “exact phrase”)"
          className="palette-input"
        />
        <Command.List className="palette-list">
          {matchingCommands.length > 0 && (
            <Command.Group heading={query ? "Commands" : "Go to"}>
              {matchingCommands.map((c) => (
                <Cmd key={c.label} icon={c.icon} label={c.label} hint={c.hint} onSelect={run(c.run)} />
              ))}
            </Command.Group>
          )}
          {query && !isFetching && hits.length === 0 && <Command.Empty className="palette-empty">No matching notes.</Command.Empty>}
          {hits.length > 0 && query && (
            <Command.Group heading="Notes">
              {hits.map((h) => (
                <Command.Item key={h.note.id} value={h.note.id} onSelect={run(() => actions.openNote(h.note.id))} className="palette-item">
                  {h.note.hasTitle && <div className="palette-title">{h.note.title}</div>}
                  <div className="palette-snippet">
                    {snippetParts(plainSnippet(h.snippet)).map(([t, m], i) => (m ? <mark key={i}>{t}</mark> : <span key={i}>{t}</span>))}
                  </div>
                  <div className="palette-meta">
                    {h.matched === "meaning" && <span className="palette-badge">similar meaning</span>}
                    {formatDay(h.note.day)}
                    {h.note.source?.title && (
                      <>
                        {" · "}
                        <BookOpen size={11} aria-hidden /> {h.note.source.title}
                      </>
                    )}
                    {h.note.labels.length > 0 && ` · ${h.note.labels.join(", ")}`}
                  </div>
                </Command.Item>
              ))}
            </Command.Group>
          )}
          {lower && labels.some((l) => l.label.toLowerCase().includes(lower)) && (
            <Command.Group heading="Labels">
              {labels
                .filter((l) => l.label.toLowerCase().includes(lower))
                .slice(0, 6)
                .map((l) => (
                  <Command.Item key={l.label} value={`label:${l.label}`} onSelect={run(() => actions.showLabel(l.label))} className="palette-item palette-cmd">
                    <Tag size={14} aria-hidden /> {l.label} <span className="faint">{l.count}</span>
                  </Command.Item>
                ))}
            </Command.Group>
          )}
        </Command.List>
      </Command>
    </div>
  );
}

function Cmd({ icon, label, hint, onSelect }: { icon: React.ReactNode; label: string; hint?: string; onSelect: () => void }) {
  return (
    <Command.Item value={label} onSelect={onSelect} className="palette-item palette-cmd">
      {icon} {label} {hint && <kbd className="kbd">{hint}</kbd>}
    </Command.Item>
  );
}

/** Strip Markdown from a snippet while keeping the match markers. */
function plainSnippet(s: string) {
  return plainExcerpt(s);
}
