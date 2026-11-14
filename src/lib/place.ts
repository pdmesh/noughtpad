// Where the app is, for back/forward: the page, and on Write which note and which
// editor pane (a blank page gets a fresh pane each time it's started).
import type { ScopeChoice } from "./chat";
import type { Relation } from "./navHistory";

/** "write" is the blank page; "notes" lists all notes (optionally one collection). */
export type View =
  | { kind: "write" }
  | { kind: "notes"; label?: string }
  | { kind: "journal" }
  | { kind: "ideas" }
  | { kind: "theme"; id: string }
  | { kind: "models"; focus?: "local" | "remote" }
  | { kind: "settings" }
  /** `key` stays the same when a new chat gets its id, so the page isn't remounted. */
  | { kind: "chat"; id: string | null; start?: ScopeChoice; key: string };

export type Place = { view: View; noteId: string | null; paneKey: string };

export function relatePlaces(prev: Place, next: Place): Relation {
  const a = prev.view;
  const b = next.view;
  if (a.kind === "write" && b.kind === "write") {
    // Two blank pages are one place, whichever pane each started in.
    if (prev.noteId === next.noteId) return "same";
    // A new note that just got saved is the same page, now with an id; opening
    // another note from a blank page is going somewhere new.
    return prev.noteId === null && prev.paneKey === next.paneKey ? "replace" : "new";
  }
  // A new chat keeps its key when it gets its id.
  if (a.kind === "chat" && b.kind === "chat" && a.key === b.key) return a.id === b.id ? "same" : "replace";
  return JSON.stringify(a) === JSON.stringify(b) ? "same" : "new";
}
