import { useQuery, useQueryClient } from "@tanstack/react-query";
import { SquarePen, Trash2, X } from "lucide-react";
import { api } from "../lib/api";
import { formatDay } from "../lib/format";

/** Past chats, in the right-hand panel on the chat page (like Related). */
export function ChatHistory({
  currentId,
  running,
  onOpen,
  onNew,
  onDeleted,
  onClose,
}: {
  currentId: string | null;
  running: Set<string>;
  onOpen: (id: string) => void;
  onNew: () => void;
  onDeleted: (id: string) => void;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const { data: chats = [], isLoading } = useQuery({ queryKey: ["chats"], queryFn: api.chatList });
  return (
    <aside className="related chat-history" aria-label="Chat history">
      <header className="related-head">
        <h2>Chats</h2>
        <div className="chat-history-actions">
          <button className="icon-btn" onClick={onNew} title="New chat (⌘L)" aria-label="New chat">
            <SquarePen size={15} />
          </button>
          <button className="icon-btn" onClick={onClose} title="Hide history" aria-label="Hide chat history">
            <X size={15} />
          </button>
        </div>
      </header>
      {!isLoading && chats.length === 0 && <p className="faint chat-history-empty">Your chats will be listed here.</p>}
      <ul className="chat-history-list">
        {chats.map((c) => (
          <li key={c.id} className={c.id === currentId ? "is-current" : ""}>
            <button className="chat-history-item" onClick={() => onOpen(c.id)} aria-current={c.id === currentId ? "page" : undefined}>
              <span className="chat-history-title">
                {c.title}
                {running.has(c.id) && <span className="dot-pulse" aria-label="answering" />}
              </span>
              <span className="faint chat-history-meta">
                {formatDay(c.updated.slice(0, 10))} · {c.scopeLabel} · {c.questions} {c.questions === 1 ? "question" : "questions"}
              </span>
            </button>
            <button
              className="chat-history-delete"
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
              <Trash2 size={13} />
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
