import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, CalendarDays, FilePlus2, Notebook, PanelLeft, RefreshCw, Rows3, Tag } from "lucide-react";
import { api } from "../lib/api";
import { formatDay, plainExcerpt, snippetParts } from "../lib/format";

export interface PaletteActions {
  openNote: (id: string) => void;
  newNote: () => void;
  showNotes: () => void;
  listUnderEditor: boolean;
  toggleListUnderEditor: () => void;
  showJournal: () => void;
  showLabel: (label: string) => void;
  toggleChrome: () => void;
  rebuildIndex: () => void;
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
          {query && !isFetching && hits.length === 0 && <Command.Empty className="palette-empty">No matching notes.</Command.Empty>}
          {hits.length > 0 && query && (
            <Command.Group heading="Notes">
              {hits.map((h) => (
                <Command.Item key={h.note.id} value={h.note.id} onSelect={run(() => actions.openNote(h.note.id))} className="palette-item">
                  <div className="palette-snippet">
                    {snippetParts(plainSnippet(h.snippet)).map(([t, m], i) => (m ? <mark key={i}>{t}</mark> : <span key={i}>{t}</span>))}
                  </div>
                  <div className="palette-meta">
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
          {!query && (
            <Command.Group heading="Go to">
              <Cmd icon={<FilePlus2 size={14} />} label="New note" hint="⌘N" onSelect={run(actions.newNote)} />
              <Cmd icon={<Notebook size={14} />} label="All notes" onSelect={run(actions.showNotes)} />
              <Cmd icon={<CalendarDays size={14} />} label="Journal" onSelect={run(actions.showJournal)} />
              <Cmd icon={<PanelLeft size={14} />} label="Toggle sidebar & toolbar" hint="⌘\" onSelect={run(actions.toggleChrome)} />
              <Cmd
                icon={<Rows3 size={14} />}
                label={actions.listUnderEditor ? "Hide note list under the editor" : "Show note list under the editor"}
                onSelect={run(actions.toggleListUnderEditor)}
              />
              <Cmd icon={<RefreshCw size={14} />} label="Rebuild search index" onSelect={run(actions.rebuildIndex)} />
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
