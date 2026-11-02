import { useState } from "react";
import { Command } from "cmdk";
import { Plus } from "lucide-react";
import type { LabelCount } from "../lib/api";

interface Props {
  /** Labels already on the note (excluded from suggestions). */
  current: string[];
  all: LabelCount[];
  onAdd: (labels: string[]) => void;
  onRemoveLast: () => void;
}

const MAX_SUGGESTIONS = 8;

/**
 * Label entry with a suggestion dropdown (cmdk, the same primitive as the ⌘K
 * palette). Enter or comma adds, Backspace on an empty field removes the last
 * label, Escape closes the list.
 */
export function LabelInput({ current, all, onAdd, onRemoveLast }: Props) {
  const [draft, setDraft] = useState("");
  const [open, setOpen] = useState(false);

  const taken = new Set(current.map((l) => l.toLowerCase()));
  const q = draft.trim().replace(/^#/, "").toLowerCase();
  const matches = all
    .filter((l) => !taken.has(l.label.toLowerCase()) && l.label.toLowerCase().includes(q))
    .sort((a, b) => Number(b.label.toLowerCase().startsWith(q)) - Number(a.label.toLowerCase().startsWith(q)))
    .slice(0, MAX_SUGGESTIONS);
  const exact = all.some((l) => l.label.toLowerCase() === q) || taken.has(q);
  const showCreate = q.length > 0 && !exact;

  const add = (raw: string) => {
    const parts = raw
      .split(",")
      .map((s) => s.trim().replace(/^#/, ""))
      .filter(Boolean)
      // Reuse an existing label's spelling ("vc" -> "VC").
      .map((p) => all.find((l) => l.label.toLowerCase() === p.toLowerCase())?.label ?? p)
      .filter((p) => !taken.has(p.toLowerCase()));
    if (parts.length) onAdd(parts);
    setDraft("");
  };

  return (
    <Command className="label-combo" shouldFilter={false} loop label="Add label">
      <Command.Input
        className="meta-input"
        value={draft}
        placeholder={current.length ? "" : "Add label"}
        aria-label="Add label"
        onValueChange={(v) => {
          if (v.endsWith(",")) add(v);
          else setDraft(v);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onBlur={() => {
          setOpen(false);
          if (draft.trim()) add(draft);
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            setOpen(false);
          } else if (e.key === "Backspace" && !draft && current.length) {
            onRemoveLast();
          } else if (e.key === "Enter" && !matches.length && !showCreate) {
            e.preventDefault();
          }
        }}
      />
      {open && (matches.length > 0 || showCreate) && (
        // preventDefault keeps focus in the input while clicking an option.
        <Command.List className="label-list" onMouseDown={(e) => e.preventDefault()}>
          {matches.map((l) => (
            <Command.Item key={l.label} value={l.label} onSelect={() => add(l.label)} className="label-option">
              <span>{l.label}</span>
              <span className="faint">{l.count}</span>
            </Command.Item>
          ))}
          {showCreate && (
            <Command.Item value={`create:${q}`} onSelect={() => add(draft)} className="label-option label-create">
              <Plus size={12} aria-hidden /> Create “{draft.trim().replace(/^#/, "")}”
            </Command.Item>
          )}
        </Command.List>
      )}
    </Command>
  );
}
