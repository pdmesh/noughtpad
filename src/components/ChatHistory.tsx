import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { SquarePen, Trash2 } from "lucide-react";
import { api } from "../lib/api";

const SHOWN = 8;

/** Past chats in the left sidebar; clicking one opens it as a page. */
export function ChatHistory({
  currentId,
  running,
  onOpen,
  onNew,
  onDeleted,
}: {
  currentId: string | null;
  running: Set<string>;
  onOpen: (id: string) => void;
  onNew: () => void;
  onDeleted: (id: string) => void;
}) {
  const qc = useQueryClient();
  const { data: chats = [] } = useQuery({ queryKey: ["chats"], queryFn: api.chatList });
  const [all, setAll] = useState(false);
  const shown = all ? chats : chats.slice(0, SHOWN);
  return (
    <nav className="sidebar-section" aria-label="Chats">
      <h4 className="sidebar-heading sidebar-heading-row">
        Chats
        <button className="icon-btn icon-btn-small" onClick={onNew} title="New chat (⌘L)" aria-label="New chat">
          <SquarePen size={13} />
        </button>
      </h4>
      {shown.map((c) => (
        <div key={c.id} className={`side-item side-chat ${c.id === currentId ? "is-active" : ""}`}>
          <button className="side-chat-open" onClick={() => onOpen(c.id)} title={`${c.title}\n${c.scopeLabel}`}>
            <span className="side-label">{c.title}</span>
            {running.has(c.id) && <span className="dot-pulse" aria-label="answering" />}
          </button>
          <button
            className="side-chat-delete"
            aria-label={`Delete “${c.title}”`}
            title="Delete chat"
            disabled={running.has(c.id)}
            onClick={() =>
              void api.chatDelete(c.id).then(() => {
                void qc.invalidateQueries({ queryKey: ["chats"] });
                onDeleted(c.id);
              })
            }
          >
            <Trash2 size={12} />
          </button>
        </div>
      ))}
      {chats.length > SHOWN && (
        <button className="side-more" onClick={() => setAll((a) => !a)}>
          {all ? "Show fewer" : `Show all ${chats.length}`}
        </button>
      )}
    </nav>
  );
}
