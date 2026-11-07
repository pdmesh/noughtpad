import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowUp, ChevronRight, Cloud, HardDrive, Search, Square } from "lucide-react";
import { api, isApiError, openExternal, type ChatNote, type ChatStep } from "../lib/api";
import { answerHtml } from "../lib/cite";
import { formatDay } from "../lib/format";
import type { Live, ScopeChoice } from "../lib/chat";
import { useLlm } from "./Models";
import { thisDevice } from "../lib/platform";

const thisMonth = () => new Date().toISOString().slice(0, 7);
const message = (e: unknown) => (isApiError(e) ? e.message : String(e));

/** Within this distance of the bottom, new text keeps the page scrolled to the end. */
const STICK_PX = 80;
const atBottom = () => window.innerHeight + window.scrollY >= document.documentElement.scrollHeight - STICK_PX;
const toBottom = () => window.scrollTo({ top: document.documentElement.scrollHeight });

/**
 * Chat with your notes, as a page. The conversation scrolls with the page and
 * the input stays fixed at the bottom of the window. Past chats are listed in
 * the left sidebar. Online models search and read notes themselves (each step
 * shows); local models get the most relevant notes per question.
 */
export function ChatView({
  chatId,
  start,
  currentNote,
  live,
  onBegin,
  onCreated,
  onOpenNote,
  onModels,
}: {
  chatId: string | null;
  /** Scope for a new chat (from "Ask about this note" etc.). */
  start?: ScopeChoice;
  /** The note last open in the editor, offered as a scope. */
  currentNote: ScopeChoice | null;
  live: Live | undefined;
  onBegin: (id: string, question: string) => void;
  onCreated: (id: string) => void;
  onOpenNote: (id: string) => void;
  onModels: (focus?: "local" | "remote") => void;
}) {
  const qc = useQueryClient();
  const { data: llm } = useLlm();
  const [choice, setChoice] = useState<ScopeChoice>(start ?? { scope: { kind: "notebook" }, label: "Whole notebook" });
  const [input, setInput] = useState("");
  const [pending, setPending] = useState<string | null>(null); // a question awaiting consent
  const [error, setError] = useState<string | null>(null);

  // A local model takes a while to load the first time: start now, while the question is typed.
  const localModel = llm?.active?.kind === "local" ? llm.active.model : null;
  useEffect(() => {
    if (localModel) void api.llmWarm();
  }, [localModel]);

  const { data: chat } = useQuery({ queryKey: ["chat", chatId], queryFn: () => api.chatGet(chatId!), enabled: !!chatId });
  const running = !!live;

  // Follow new text while the reader is at the bottom; opening a chat starts at its end.
  const stick = useRef(true);
  useEffect(() => {
    const onScroll = () => (stick.current = atBottom());
    window.addEventListener("scroll", onScroll, { passive: true });
    return () => window.removeEventListener("scroll", onScroll);
  }, []);
  useEffect(() => {
    stick.current = true;
  }, [chatId]);
  useLayoutEffect(() => {
    if (stick.current) toBottom();
  }, [chat, live, pending]);

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

  const send = async (text: string, consent: boolean) => {
    setError(null);
    stick.current = true;
    try {
      const scope = chat?.scope ?? choice.scope;
      const scopeLabel = chat?.scopeLabel ?? choice.label;
      const { chatId: id } = await api.chatSend({ chatId, scope, scopeLabel, text, consent });
      onBegin(id, text);
      setPending(null);
      void qc.invalidateQueries({ queryKey: ["chat", id] });
      void qc.invalidateQueries({ queryKey: ["chats"] });
      if (id !== chatId) onCreated(id);
    } catch (e) {
      if (isApiError(e) && e.kind === "consent") setPending(text);
      else setError(message(e));
    }
  };

  const submit = () => {
    const text = input.trim();
    if (!text || running) return;
    setInput("");
    if (needsConsent) setPending(text);
    else void send(text, false);
  };

  if (!llm) return null;
  if (!llm.active) return <ChatIntro onModels={onModels} localAvailable={llm.localAvailable} />;

  const allNotes = (chat?.notes ?? []).concat(live?.notes ?? []);
  const noteOf = (n: number) => allNotes.find((x) => x.n === n);
  const lastIsUser = chat?.messages[chat.messages.length - 1]?.role === "user";
  const scopeLabel = chat?.scopeLabel ?? choice.label;

  return (
    <div className="chat-page">
      <header className="chat-page-head">
        <h2 className="journal-heading">{chat?.title ?? "New chat"}</h2>
        {chat && (
          <span className="chat-scope faint">
            <Search size={11} aria-hidden /> {chat.scopeLabel}
          </span>
        )}
      </header>

      <div className="chat-thread">
        {!chat && !running && !pending && (
          <div className="chat-empty">
            <p>Ask anything about your notes: how an idea changed, what you left open, what a month was about.</p>
            <p className="faint">
              {llm.active.kind === "local"
                ? `Runs on ${thisDevice}; your notes stay here.`
                : "The model searches and reads notes within the scope you pick below; each one it reads shows up here."}
            </p>
          </div>
        )}
        {chat?.messages.map((m, i) =>
          m.role === "user" ? (
            <div key={i} className="chat-user">
              {m.text}
            </div>
          ) : (
            <Answer key={i} steps={m.steps} text={m.text} error={m.error} model={m.model} noteOf={noteOf} onOpenNote={onOpenNote} />
          ),
        )}
        {live && (
          <>
            {!lastIsUser && live.question && <div className="chat-user">{live.question}</div>}
            <Answer steps={live.steps} text={live.text} streaming noteOf={noteOf} onOpenNote={onOpenNote} />
          </>
        )}
        {pending && (
          <>
            <div className="chat-user">{pending}</div>
            <div className="consent" role="group" aria-label="Approve sending notes">
              <p>
                <strong>Send notes to {llm.activeLabel}?</strong> To answer, the model will search and read your notes in <strong>{scopeLabel}</strong>. They
                go to <code>{llm.activeWhere}</code>. Each note it reads is listed in this chat. You approve once per chat.
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
          <p className="error" role="alert">
            {error}
          </p>
        )}
      </div>

      <footer className="chat-dock">
        {!chat && (
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
          <AutoGrow value={input} disabled={!!pending} onChange={setInput} onSubmit={submit} placeholder={chat ? "Ask a follow-up…" : "Ask your notes…"} />
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
    </div>
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
    el.style.height = `${Math.min(el.scrollHeight, 180)}px`;
  }, [value]);
  useEffect(() => ref.current?.focus(), []);
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

function Answer({
  steps,
  text,
  error,
  model,
  streaming,
  noteOf,
  onOpenNote,
}: {
  steps: ChatStep[];
  text: string;
  error?: string | null;
  model?: string | null;
  streaming?: boolean;
  noteOf: (n: number) => ChatNote | undefined;
  onOpenNote: (id: string) => void;
}) {
  const max = useMemo(() => {
    let m = 0;
    for (let n = 1; noteOf(n); n++) m = n;
    return m;
  }, [noteOf, steps]);
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
            // Links open in the system browser, never inside the app window.
            const a = (e.target as HTMLElement).closest<HTMLAnchorElement>("a[href]");
            if (a) {
              e.preventDefault();
              void openExternal(a.href);
              return;
            }
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

function ChatIntro({ onModels, localAvailable }: { onModels: (focus?: "local" | "remote") => void; localAvailable: boolean }) {
  return (
    <div className="chat-page">
      <h2 className="journal-heading">Chat</h2>
      <p className="ideas-lede">
        Chat with your notes: how an idea changed, what you left open, what a month was about. This needs a language model, and none is switched on.
      </p>
      <div className="think-choices">
        {/* Builds without the local runtime (mobile) can't offer this at all. */}
        {localAvailable && (
          <button className="theme-card" onClick={() => onModels("local")}>
            <span className="theme-card-main">
              <span className="theme-name">
                <HardDrive size={15} aria-hidden /> Run a model on this computer
              </span>
              <span className="theme-meta faint">Private and free. A one-time download of about 2.5 GB.</span>
            </span>
          </button>
        )}
        <button className="theme-card" onClick={() => onModels("remote")}>
          <span className="theme-card-main">
            <span className="theme-name">
              <Cloud size={15} aria-hidden /> Use your own API key
            </span>
            <span className="theme-meta faint">Anthropic, OpenAI, Gemini, Ollama or any OpenAI-compatible server. You see which notes are sent.</span>
          </span>
        </button>
      </div>
    </div>
  );
}
