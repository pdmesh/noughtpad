import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowUp, ChevronRight, Cloud, HardDrive, History, Search, SquarePen, Square, Trash2, X } from "lucide-react";
import {
  api,
  isApiError,
  type ChatDeltaEvent,
  type ChatDoneEvent,
  type ChatNote,
  type ChatScope,
  type ChatStep,
  type ChatStepEvent,
} from "../lib/api";
import { answerHtml } from "../lib/cite";
import { formatDay } from "../lib/format";
import { useEvent } from "../lib/useEvent";
import { useLlm } from "../views/Models";

export interface ScopeChoice {
  scope: ChatScope;
  /** Shown on the chip, e.g. the note's title or the theme's name. */
  label: string;
}

/** A request to start a new chat with a given scope (from "Ask about this note" etc.). */
export interface ChatStart extends ScopeChoice {
  nonce: number;
}

/** What is streaming in for a chat right now. */
interface Live {
  question: string;
  steps: ChatStep[];
  notes: ChatNote[];
  text: string;
}

const thisMonth = () => new Date().toISOString().slice(0, 7);
const message = (e: unknown) => (isApiError(e) ? e.message : String(e));

/**
 * Chat with your notes, in the right-hand panel. Online models search and
 * read notes themselves (each step shows here); local models get the most
 * relevant notes per question. Chats are saved as history.
 */
export function ChatPanel({
  start,
  currentNote,
  onOpenNote,
  onModels,
  onClose,
}: {
  start: ChatStart | null;
  currentNote: ScopeChoice | null;
  onOpenNote: (id: string) => void;
  onModels: (focus?: "local" | "remote") => void;
  onClose: () => void;
}) {
  const qc = useQueryClient();
  const { data: llm } = useLlm();
  const [chatId, setChatId] = useState<string | null>(null);
  const [showHistory, setShowHistory] = useState(false);
  const [choice, setChoice] = useState<ScopeChoice>(start ?? { scope: { kind: "notebook" }, label: "Whole notebook" });
  const [input, setInput] = useState("");
  const [pending, setPending] = useState<string | null>(null); // a question awaiting consent
  const [error, setError] = useState<string | null>(null);
  const [live, setLive] = useState<Record<string, Live>>({});

  // "Ask about this note" and friends start a fresh chat with that scope.
  useEffect(() => {
    if (!start) return;
    setChatId(null);
    setShowHistory(false);
    setChoice(start);
    setPending(null);
    setError(null);
  }, [start?.nonce]);

  // A local model takes a while to load the first time: start now, while the question is typed.
  const localModel = llm?.active?.kind === "local" ? llm.active.model : null;
  useEffect(() => {
    if (localModel) void api.llmWarm();
  }, [localModel]);

  const { data: chat } = useQuery({ queryKey: ["chat", chatId], queryFn: () => api.chatGet(chatId!), enabled: !!chatId });
  const cur = chatId ? live[chatId] : undefined;
  const running = !!cur;

  const patch = (id: string, f: (l: Live) => Live) => setLive((m) => ({ ...m, [id]: f(m[id] ?? { question: "", steps: [], notes: [], text: "" }) }));
  useEvent<ChatStepEvent>("chat-step", (e) =>
    patch(e.chatId, (l) => ({ ...l, steps: [...l.steps, e.step], notes: [...l.notes.filter((n) => !e.notes.some((x) => x.n === n.n)), ...e.notes] })),
  );
  useEvent<ChatDeltaEvent>("chat-delta", (e) => patch(e.chatId, (l) => ({ ...l, text: l.text + e.text })));
  useEvent<ChatDoneEvent>("chat-done", (e) => {
    void Promise.all([qc.invalidateQueries({ queryKey: ["chat", e.chatId] }), qc.invalidateQueries({ queryKey: ["chats"] })]).then(() =>
      setLive((m) => {
        const { [e.chatId]: _, ...rest } = m;
        return rest;
      }),
    );
  });

  // Scrolling: follow new text unless the reader has scrolled up.
  const scroller = useRef<HTMLDivElement | null>(null);
  const stick = useRef(true);
  const onScroll = () => {
    const el = scroller.current;
    if (el) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 60;
  };
  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [chat, cur, pending, showHistory]);

  const scopeChoices = useMemo(() => {
    const base: ScopeChoice[] = [
      { scope: { kind: "notebook" }, label: "Whole notebook" },
      ...(currentNote ? [currentNote] : []),
      { scope: { kind: "month", month: thisMonth() }, label: "This month" },
      { scope: { kind: "openQuestions" }, label: "Open questions" },
    ];
    if (start && !base.some((b) => b.label === start.label)) base.splice(1, 0, start);
    return base;
  }, [currentNote, start]);

  const needsConsent = llm?.active?.kind === "remote" && (!chat || chat.approved !== llm.activeDestination);

  const send = useCallback(
    async (text: string, consent: boolean) => {
      setError(null);
      stick.current = true;
      try {
        const scope = chat?.scope ?? choice.scope;
        const scopeLabel = chat?.scopeLabel ?? choice.label;
        // Show the question at once; the saved chat replaces it when the answer is done.
        const { chatId: id } = await api.chatSend({ chatId, scope, scopeLabel, text, consent });
        setLive((m) => ({ ...m, [id]: { ...(m[id] ?? { steps: [], notes: [], text: "" }), question: text } }));
        if (id !== chatId) setChatId(id);
        void qc.invalidateQueries({ queryKey: ["chat", id] });
        setPending(null);
      } catch (e) {
        if (isApiError(e) && e.kind === "consent") setPending(text);
        else setError(message(e));
      }
    },
    [chat, chatId, choice, qc],
  );

  const submit = () => {
    const text = input.trim();
    if (!text || running) return;
    setInput("");
    if (needsConsent) setPending(text);
    else void send(text, false);
  };

  const newChat = () => {
    setChatId(null);
    setShowHistory(false);
    setPending(null);
    setError(null);
    setChoice(currentNote && choice.scope.kind === "note" ? currentNote : choice);
  };

  const scopeLabel = chat?.scopeLabel ?? choice.label;
  const notesById = (n: number) => (chat?.notes ?? []).concat(cur?.notes ?? []).find((x) => x.n === n);

  return (
    <aside className="chat-panel" aria-label="Chat with your notes">
      <header className="chat-head">
        <h2 title={chat?.title}>{showHistory ? "History" : (chat?.title ?? "New chat")}</h2>
        <div className="chat-head-actions">
          <button
            className={`icon-btn ${showHistory ? "is-on" : ""}`}
            onClick={() => setShowHistory((h) => !h)}
            title="History"
            aria-label="Chat history"
            aria-pressed={showHistory}
          >
            <History size={15} />
          </button>
          <button className="icon-btn" onClick={newChat} title="New chat" aria-label="New chat">
            <SquarePen size={15} />
          </button>
          <button className="icon-btn" onClick={onClose} title="Close" aria-label="Close chat">
            <X size={15} />
          </button>
        </div>
      </header>

      <div className="chat-scroll" ref={scroller} onScroll={onScroll}>
        {!llm ? null : !llm.active ? (
          <ChatIntro onModels={onModels} />
        ) : showHistory ? (
          <HistoryList
            currentId={chatId}
            onOpen={(id) => {
              setChatId(id);
              setShowHistory(false);
              stick.current = true;
            }}
          />
        ) : (
          <>
            {!chat && !running && !pending && (
              <div className="chat-empty">
                <p>Ask anything about your notes.</p>
                <p className="faint">
                  {llm.active.kind === "local"
                    ? "Runs on this computer; your notes stay here."
                    : "The model searches and reads notes within the scope below; each one it reads shows up here."}
                </p>
              </div>
            )}
            {chat?.messages.map((m, i) =>
              m.role === "user" ? (
                <UserBubble key={i} text={m.text} />
              ) : (
                <Answer
                  key={i}
                  steps={m.steps}
                  text={m.text}
                  error={m.error}
                  model={m.model}
                  notes={chat.notes}
                  noteOf={notesById}
                  onOpenNote={onOpenNote}
                />
              ),
            )}
            {cur && (
              <>
                {chat?.messages[chat.messages.length - 1]?.role !== "user" && cur.question && <UserBubble text={cur.question} />}
                <Answer steps={cur.steps} text={cur.text} streaming notes={cur.notes} noteOf={notesById} onOpenNote={onOpenNote} />
              </>
            )}
            {pending && (
              <>
                <UserBubble text={pending} />
                <div className="consent" role="group" aria-label="Approve sending notes">
                  <p>
                    <strong>Send notes to {llm.activeLabel}?</strong> To answer, the model will search and read your notes in <strong>{scopeLabel}</strong>.
                    They go to <code>{llm.activeWhere}</code>. Each note it reads is listed in this chat. You approve once per chat.
                  </p>
                  <div className="field-row">
                    <button className="btn btn-small btn-primary" onClick={() => void send(pending, true)}>
                      Send and ask
                    </button>
                    <button
                      className="btn btn-small"
                      onClick={() => {
                        setInput(pending);
                        setPending(null);
                      }}
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              </>
            )}
            {error && (
              <p className="error chat-error" role="alert">
                {error}
              </p>
            )}
          </>
        )}
      </div>

      {llm?.active && !showHistory && (
        <footer className="chat-foot">
          {chat ? (
            <div className="chat-scope faint">
              <Search size={11} aria-hidden /> {chat.scopeLabel}
            </div>
          ) : (
            <div className="think-scopes" role="radiogroup" aria-label="Notes this chat can read">
              {scopeChoices.map((c) => (
                <button
                  type="button"
                  key={c.label}
                  role="radio"
                  aria-checked={c.label === choice.label}
                  className={`chip ${c.label === choice.label ? "is-on" : "chip-off"}`}
                  onClick={() => setChoice(c)}
                >
                  {c.label}
                </button>
              ))}
            </div>
          )}
          <div className="chat-input">
            <AutoGrow
              value={input}
              disabled={!!pending}
              onChange={setInput}
              onSubmit={submit}
              placeholder={chat ? "Ask a follow-up…" : "Ask your notes…"}
            />
            {running ? (
              <button className="icon-btn think-send" onClick={() => chatId && void api.chatCancel(chatId)} aria-label="Stop" title="Stop">
                <Square size={13} />
              </button>
            ) : (
              <button className="icon-btn think-send" onClick={submit} disabled={!input.trim() || !!pending} aria-label="Send" title="Send (Enter)">
                <ArrowUp size={16} />
              </button>
            )}
          </div>
          <div className="chat-model faint">
            {llm.active.kind === "local" ? <HardDrive size={11} aria-hidden /> : <Cloud size={11} aria-hidden />}{" "}
            <button className="link-btn" onClick={() => onModels()}>
              {llm.activeLabel}
            </button>
          </div>
        </footer>
      )}
    </aside>
  );
}

function AutoGrow({
  value,
  disabled,
  onChange,
  onSubmit,
  placeholder,
}: {
  value: string;
  disabled: boolean;
  onChange: (v: string) => void;
  onSubmit: () => void;
  placeholder: string;
}) {
  const ref = useRef<HTMLTextAreaElement | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [value]);
  return (
    <textarea
      ref={ref}
      rows={1}
      aria-label="Ask your notes"
      value={value}
      disabled={disabled}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
          e.preventDefault();
          onSubmit();
        }
      }}
    />
  );
}

function UserBubble({ text }: { text: string }) {
  return <div className="chat-user">{text}</div>;
}

function Answer({
  steps,
  text,
  error,
  model,
  streaming,
  notes,
  noteOf,
  onOpenNote,
}: {
  steps: ChatStep[];
  text: string;
  error?: string | null;
  model?: string | null;
  streaming?: boolean;
  notes: ChatNote[];
  noteOf: (n: number) => ChatNote | undefined;
  onOpenNote: (id: string) => void;
}) {
  const max = Math.max(0, ...notes.map((n) => n.n));
  const html = useMemo(() => answerHtml(text.trim(), max), [text, max]);
  return (
    <div className="chat-answer">
      {steps.length > 0 && (
        <ul className="chat-steps">
          {steps.map((s, i) => (
            <StepLine key={i} step={s} noteOf={noteOf} onOpenNote={onOpenNote} />
          ))}
        </ul>
      )}
      {text.trim() ? (
        <div
          className="answer"
          onClick={(e) => {
            const t = (e.target as HTMLElement).closest<HTMLElement>("[data-cite]");
            const note = t && noteOf(Number(t.dataset.cite));
            if (note) onOpenNote(note.id);
          }}
          dangerouslySetInnerHTML={{ __html: html }}
        />
      ) : streaming ? (
        <p className="faint think-wait">
          <span className="dot-pulse" aria-hidden /> Thinking…
        </p>
      ) : null}
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      {model && !streaming && <div className="chat-meta faint">{model}</div>}
    </div>
  );
}

function StepLine({ step, noteOf, onOpenNote }: { step: ChatStep; noteOf: (n: number) => ChatNote | undefined; onOpenNote: (id: string) => void }) {
  const [open, setOpen] = useState(false);
  const notes = step.notes.map(noteOf).filter((n): n is ChatNote => !!n);
  return (
    <li>
      <button className="chat-step" onClick={() => setOpen((o) => !o)} aria-expanded={open} disabled={notes.length === 0}>
        <ChevronRight size={12} className={open ? "is-open" : ""} aria-hidden />
        <span>{step.label}</span>
        {notes.length > 0 && <span className="faint">· {notes.length === 1 ? "1 note" : `${notes.length} notes`}</span>}
      </button>
      {open && (
        <ol className="source-list chat-step-notes">
          {notes.map((n) => (
            <li key={n.n}>
              <span className="source-num">{n.n}</span>
              <button className="link-row" onClick={() => onOpenNote(n.id)}>
                <span className="source-title">{n.title}</span>
                <span className="faint">
                  {formatDay(n.day)}
                  {n.source ? ` · ${n.source}` : ""}
                </span>
              </button>
            </li>
          ))}
        </ol>
      )}
    </li>
  );
}

function HistoryList({ currentId, onOpen }: { currentId: string | null; onOpen: (id: string) => void }) {
  const qc = useQueryClient();
  const { data: chats = [], isLoading } = useQuery({ queryKey: ["chats"], queryFn: api.chatList });
  if (isLoading) return null;
  if (chats.length === 0) return <p className="faint chat-empty">No chats yet.</p>;
  return (
    <ul className="chat-history">
      {chats.map((c) => (
        <li key={c.id} className={c.id === currentId ? "is-current" : ""}>
          <button className="chat-history-item" onClick={() => onOpen(c.id)}>
            <span className="chat-history-title">{c.title}</span>
            <span className="faint">
              {formatDay(c.updated.slice(0, 10))} · {c.scopeLabel} · {c.questions} {c.questions === 1 ? "question" : "questions"}
            </span>
          </button>
          <button
            className="icon-btn"
            aria-label={`Delete “${c.title}”`}
            title="Delete chat"
            onClick={() =>
              void api.chatDelete(c.id).then(() => {
                void qc.invalidateQueries({ queryKey: ["chats"] });
              })
            }
          >
            <Trash2 size={13} />
          </button>
        </li>
      ))}
    </ul>
  );
}

function ChatIntro({ onModels }: { onModels: (focus?: "local" | "remote") => void }) {
  return (
    <div className="chat-empty">
      <p>Chat with your notes: how an idea changed, what you left open, what a month was about. This needs a language model, and none is switched on.</p>
      <div className="think-choices">
        <button className="theme-card" onClick={() => onModels("local")}>
          <span className="theme-card-main">
            <span className="theme-name">
              <HardDrive size={15} aria-hidden /> Run a model on this computer
            </span>
            <span className="theme-meta faint">Private and free. A one-time download of about 2.5 GB.</span>
          </span>
        </button>
        <button className="theme-card" onClick={() => onModels("remote")}>
          <span className="theme-card-main">
            <span className="theme-name">
              <Cloud size={15} aria-hidden /> Use your own API key
            </span>
            <span className="theme-meta faint">Anthropic, OpenAI, Gemini, Ollama or any OpenAI-compatible server.</span>
          </span>
        </button>
      </div>
    </div>
  );
}
