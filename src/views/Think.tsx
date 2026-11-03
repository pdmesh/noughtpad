import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowUp, BookOpen, Bookmark, BookmarkCheck, Copy, Cloud, HardDrive, Loader2, Square, Trash2 } from "lucide-react";
import { useEvent } from "../lib/useEvent";
import { api, isApiError, type NoteSummary, type Prepared, type ThinkDelta, type ThinkDone, type ThinkScope, type ThinkUsage } from "../lib/api";
import { answerHtml } from "../lib/cite";
import { formatDay } from "../lib/format";
import { useLlm } from "./Models";

export interface ScopeChoice {
  scope: ThinkScope;
  /** Shown on the chip, e.g. the note's title or the theme's name. */
  label: string;
}

interface Exchange {
  id: string;
  question: string;
  scopeLabel: string;
  prepared: Prepared;
  /** Notes the user left ticked (all by default). */
  include: Set<string>;
  status: "confirm" | "empty" | "streaming" | "done" | "error" | "cancelled";
  text: string;
  cited: string[];
  usage: ThinkUsage | null;
  error: string | null;
  kept: boolean;
}

const thisMonth = () => new Date().toISOString().slice(0, 7);

function suggestions(scope: ThinkScope): string[] {
  switch (scope.kind) {
    case "month":
      return ["Reflect on this month: what was I thinking about, and what changed?"];
    case "openQuestions":
      return ["Which of my open questions matter most, and what in my notes would help answer them?"];
    case "note":
      return ["What does this connect to, and how has my thinking on it changed?"];
    case "theme":
      return ["How has my thinking on this changed over time?", "Where do my notes on this disagree with each other?"];
    case "label":
      return ["Summarise what I actually think about this, with the open questions."];
    default:
      return [];
  }
}

/**
 * The Thinking workspace: ask a question about a scope of notes; the answer
 * streams in with numbered citations that open the notes they came from.
 */
export function ThinkView({
  initial,
  onOpenNote,
  onModels,
}: {
  initial?: ScopeChoice;
  onOpenNote: (id: string) => void;
  onModels: (focus?: "local" | "remote") => void;
}) {
  const { data: llm } = useLlm();
  const qc = useQueryClient();
  const base: ScopeChoice[] = useMemo(
    () => [
      { scope: { kind: "notebook" }, label: "Whole notebook" },
      { scope: { kind: "month", month: thisMonth() }, label: "This month" },
      { scope: { kind: "openQuestions" }, label: "Open questions" },
    ],
    [],
  );
  const choices = initial && initial.scope.kind !== "notebook" ? [initial, ...base] : base;
  const [choice, setChoice] = useState<ScopeChoice>(initial ?? base[0]);
  useEffect(() => {
    if (initial) setChoice(initial);
  }, [initial?.label]);

  const [input, setInput] = useState("");
  const [exchanges, setExchanges] = useState<Exchange[]>([]);
  const [error, setError] = useState<string | null>(null);
  const seq = useRef(0);
  const endRef = useRef<HTMLDivElement | null>(null);
  const update = useCallback((id: string, f: (e: Exchange) => Exchange) => setExchanges((xs) => xs.map((x) => (x.id === id ? f(x) : x))), []);

  useEvent<ThinkDelta>("think-delta", (d) => update(d.requestId, (x) => ({ ...x, text: x.text + d.text })));
  useEvent<ThinkDone>("think-done", (d) =>
    update(d.requestId, (x) => ({
      ...x,
      status: d.error ? "error" : d.cancelled ? "cancelled" : "done",
      error: d.error,
      cited: d.cited,
      usage: d.usage,
    })),
  );

  const send = useCallback(
    async (x: Exchange, consent: boolean) => {
      const ids = x.prepared.notes.filter((n) => x.include.has(n.id)).map((n) => n.id);
      update(x.id, (e) => ({
        ...e,
        status: "streaming",
        text: "",
        error: null,
      }));
      try {
        await api.thinkAsk(x.id, x.question, ids, consent);
      } catch (e) {
        update(x.id, (ex) => ({
          ...ex,
          status: "error",
          error: isApiError(e) ? e.message : String(e),
        }));
      }
    },
    [update],
  );

  const ask = async (question: string) => {
    const q = question.trim();
    if (!q) return;
    setError(null);
    let prepared: Prepared;
    try {
      prepared = await api.thinkPrepare(q, choice.scope);
    } catch (e) {
      setError(isApiError(e) ? e.message : String(e));
      return;
    }
    const x: Exchange = {
      id: `ask-${Date.now()}-${++seq.current}`,
      question: q,
      scopeLabel: choice.label,
      prepared,
      include: new Set(prepared.notes.map((n) => n.id)),
      status: prepared.notes.length === 0 ? "empty" : prepared.needsConsent ? "confirm" : "streaming",
      text: "",
      cited: [],
      usage: null,
      error: null,
      kept: false,
    };
    setExchanges((xs) => [...xs, x]);
    setInput("");
    requestAnimationFrame(() => endRef.current?.scrollIntoView({ block: "end", behavior: "smooth" }));
    if (x.status === "streaming") void send(x, false);
  };

  if (!llm) return null;
  if (!llm.active) return <ThinkIntro onModels={onModels} />;

  const running = exchanges.some((x) => x.status === "streaming");
  return (
    <div className="think">
      <h2 className="journal-heading">Think</h2>
      <p className="ideas-lede">
        Ask about your notes. Answers cite the notes they use; click a number to open it.{" "}
        <button className="link-btn think-model" onClick={() => onModels()}>
          {llm.active.kind === "local" ? <HardDrive size={12} aria-hidden /> : <Cloud size={12} aria-hidden />} {llm.activeLabel}
        </button>
      </p>

      {exchanges.map((x) => (
        <ExchangeView
          key={x.id}
          x={x}
          onOpenNote={onOpenNote}
          onToggle={(id) =>
            update(x.id, (e) => {
              const include = new Set(e.include);
              if (include.has(id)) include.delete(id);
              else include.add(id);
              return { ...e, include };
            })
          }
          onSend={() => void send(x, true)}
          onDiscard={() => setExchanges((xs) => xs.filter((e) => e.id !== x.id))}
          onStop={() => void api.thinkCancel(x.id)}
          onKeep={async () => {
            const ids = x.prepared.notes.filter((n) => x.include.has(n.id)).map((n) => n.id);
            await api.keepAnswer({
              question: x.question,
              answer: x.text,
              noteIds: ids,
              model: llm.activeLabel ?? "",
            });
            update(x.id, (e) => ({ ...e, kept: true }));
            void qc.invalidateQueries({ queryKey: ["kept"] });
          }}
        />
      ))}
      <div ref={endRef} />

      <form
        className="think-ask"
        onSubmit={(e) => {
          e.preventDefault();
          void ask(input);
        }}
      >
        <div className="think-scopes" role="radiogroup" aria-label="Notes to think with">
          {choices.map((c) => (
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
        <div className="think-input">
          <textarea
            aria-label="Ask your notes"
            rows={2}
            value={input}
            placeholder={choice.scope.kind === "notebook" ? "Ask your notes…  e.g. How has my thinking about ownership changed?" : "Ask about these notes…"}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                if (!running) void ask(input);
              }
            }}
          />
          <button type="submit" className="icon-btn think-send" disabled={!input.trim() || running} aria-label="Ask" title="Ask (Enter)">
            <ArrowUp size={16} />
          </button>
        </div>
        {suggestions(choice.scope).length > 0 && (
          <div className="think-suggest">
            {suggestions(choice.scope).map((s) => (
              <button type="button" key={s} className="link-btn" disabled={running} onClick={() => void ask(s)}>
                {s}
              </button>
            ))}
          </div>
        )}
        {error && (
          <p className="error" role="alert">
            {error}
          </p>
        )}
      </form>

      <KeptList onOpenNote={onOpenNote} />
    </div>
  );
}

function ThinkIntro({ onModels }: { onModels: (focus?: "local" | "remote") => void }) {
  return (
    <div className="think">
      <h2 className="journal-heading">Think</h2>
      <p className="ideas-lede">
        Ask questions across your notes and get answers that cite them: how an idea changed, what you left open, what a month was about. This needs a language
        model, and none is switched on.
      </p>
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
            <span className="theme-meta faint">Anthropic, OpenAI, Gemini, Ollama or any OpenAI-compatible server. You see which notes are sent.</span>
          </span>
        </button>
      </div>
    </div>
  );
}

function ExchangeView({
  x,
  onOpenNote,
  onToggle,
  onSend,
  onDiscard,
  onStop,
  onKeep,
}: {
  x: Exchange;
  onOpenNote: (id: string) => void;
  onToggle: (id: string) => void;
  onSend: () => void;
  onDiscard: () => void;
  onStop: () => void;
  onKeep: () => Promise<void>;
}) {
  const sent = x.prepared.notes.filter((n) => x.include.has(n.id));
  const p = x.prepared.provider;
  const html = useMemo(() => answerHtml(x.text, sent.length), [x.text, sent.length]);
  const [copied, setCopied] = useState(false);

  return (
    <article className="exchange" aria-label={x.question}>
      <div className="exchange-q">
        <span>{x.question}</span>
        <span className="faint exchange-scope">{x.scopeLabel}</span>
      </div>

      {x.status === "empty" && (
        <p className="faint">
          No notes matched this. Try other words or a different scope
          {x.prepared.notes.length === 0 ? "; turning on search by meaning (⌘K) helps with questions." : "."}
        </p>
      )}

      {x.status === "confirm" && p && (
        <div className="consent" role="group" aria-label="Confirm what is sent">
          <p>
            <strong>
              Send {sent.length} {sent.length === 1 ? "note" : "notes"} to {p.label}?
            </strong>{" "}
            They go to <code>{p.destination}</code> ({p.model}) to answer this question. Untick any you'd rather keep private. Nought asks again whenever the
            provider changes.
          </p>
          <SourceList notes={x.prepared.notes} include={x.include} onToggle={onToggle} onOpen={onOpenNote} />
          <div className="field-row">
            <button className="btn btn-small btn-primary" onClick={onSend} disabled={sent.length === 0}>
              Send and ask
            </button>
            <button className="btn btn-small" onClick={onDiscard}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {(x.status === "streaming" || x.status === "done" || x.status === "error" || x.status === "cancelled") && (
        <>
          <div
            className="answer"
            aria-live="polite"
            onClick={(e) => {
              const t = (e.target as HTMLElement).closest<HTMLElement>("[data-cite]");
              if (t) onOpenNote(sent[Number(t.dataset.cite) - 1].id);
            }}
          >
            {x.text ? (
              <div dangerouslySetInnerHTML={{ __html: html }} />
            ) : x.status === "streaming" ? (
              <p className="faint think-wait">
                <Loader2 size={14} className="spin" aria-hidden /> Reading {sent.length} {sent.length === 1 ? "note" : "notes"}
                {p?.onDevice ? " on this computer…" : "…"}
              </p>
            ) : null}
          </div>
          {x.status === "error" && (
            <p className="error" role="alert">
              {x.error}
            </p>
          )}
          {x.status === "done" && x.text && x.cited.length === 0 && (
            <p className="faint answer-warn">This answer cites none of your notes; treat it with care.</p>
          )}
          <details className="sources" open={x.status !== "streaming"}>
            <summary>
              {sent.length} {sent.length === 1 ? "note" : "notes"} read
              {x.status === "done" && ` · ${x.cited.length} cited`}
            </summary>
            <SourceList notes={sent} cited={x.status === "done" ? new Set(x.cited) : undefined} onOpen={onOpenNote} />
          </details>
          <div className="exchange-actions">
            {x.status === "streaming" ? (
              <button className="btn btn-small" onClick={onStop}>
                <Square size={12} aria-hidden /> Stop
              </button>
            ) : (
              x.text && (
                <>
                  <button className="btn btn-small" onClick={() => void onKeep()} disabled={x.kept} title="Save to .nought/artifacts">
                    {x.kept ? <BookmarkCheck size={13} aria-hidden /> : <Bookmark size={13} aria-hidden />} {x.kept ? "Kept" : "Keep"}
                  </button>
                  <button
                    className="btn btn-small"
                    onClick={() =>
                      void navigator.clipboard?.writeText(x.text).then(() => {
                        setCopied(true);
                        setTimeout(() => setCopied(false), 1500);
                      })
                    }
                  >
                    <Copy size={13} aria-hidden /> {copied ? "Copied" : "Copy"}
                  </button>
                </>
              )
            )}
            {x.usage && x.status === "done" && (
              <span className="faint exchange-meta">
                {p?.label}
                {p?.onDevice ? " · on this computer" : ""} · {x.usage.secs.toFixed(0)} s
              </span>
            )}
          </div>
        </>
      )}
    </article>
  );
}

function SourceList({
  notes,
  include,
  cited,
  onToggle,
  onOpen,
}: {
  notes: NoteSummary[];
  include?: Set<string>;
  cited?: Set<string>;
  onToggle?: (id: string) => void;
  onOpen: (id: string) => void;
}) {
  return (
    <ol className="source-list">
      {notes.map((n, i) => (
        <li key={n.id} className={cited && !cited.has(n.id) ? "is-uncited" : ""}>
          {onToggle && include && <input type="checkbox" checked={include.has(n.id)} onChange={() => onToggle(n.id)} aria-label={`Include “${n.title}”`} />}
          {!onToggle && <span className="source-num">{i + 1}</span>}
          <button className="link-row" onClick={() => onOpen(n.id)}>
            <span className="source-title">{n.title}</span>
            <span className="faint">
              {formatDay(n.day)}
              {n.source?.title && (
                <>
                  {" · "}
                  <BookOpen size={11} aria-hidden /> {n.source.title}
                </>
              )}
            </span>
          </button>
        </li>
      ))}
    </ol>
  );
}

function KeptList({ onOpenNote }: { onOpenNote: (id: string) => void }) {
  const qc = useQueryClient();
  const { data: kept = [] } = useQuery({
    queryKey: ["kept"],
    queryFn: api.keptAnswers,
  });
  if (kept.length === 0) return null;
  return (
    <section className="ideas-section">
      <h3 className="ideas-section-title">Kept</h3>
      <p className="ideas-hint faint">Saved answers, in your notes folder under .nought/artifacts. Your notes themselves are never changed.</p>
      <ul className="kept-list">
        {kept.map((k) => (
          <li key={k.file}>
            <details>
              <summary>
                <span>{k.question}</span> <span className="faint">· {formatDay(k.created.slice(0, 10))}</span>
              </summary>
              <div
                className="answer"
                onClick={(e) => {
                  const t = (e.target as HTMLElement).closest<HTMLElement>("[data-cite]");
                  const id = t && k.noteIds[Number(t.dataset.cite) - 1];
                  if (id) onOpenNote(id);
                }}
                dangerouslySetInnerHTML={{
                  __html: answerHtml(k.answer, k.noteIds.length),
                }}
              />
              <div className="exchange-actions">
                <span className="faint exchange-meta">{k.model}</span>
                <button
                  className="icon-btn"
                  aria-label="Forget this answer"
                  title="Forget"
                  onClick={() => void api.forgetKept(k.file).then(() => qc.invalidateQueries({ queryKey: ["kept"] }))}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </details>
          </li>
        ))}
      </ul>
    </section>
  );
}
