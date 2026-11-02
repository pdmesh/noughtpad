import { useEffect } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, BookOpen, CornerDownRight, HelpCircle, RefreshCcw } from "lucide-react";
import { api, onEvent, type NoteSummary, type ThemeSummary } from "../lib/api";
import { formatDay, formatMonth, plainExcerpt } from "../lib/format";
import { useSemanticStatus, describeStatus } from "../lib/useSemantic";
import { Sparkline } from "../components/Sparkline";
import { EnableCard } from "../components/RelatedPanel";

interface Nav {
  onOpenNote: (id: string) => void;
  onOpenTheme: (id: string) => void;
}

/** Refetch ideas whenever the background worker recomputes them. */
function useIdeasEvents() {
  const qc = useQueryClient();
  useEffect(() => {
    let off = () => {};
    void onEvent("ideas-updated", () => qc.invalidateQueries({ queryKey: ["ideas"] })).then((u) => (off = u));
    return () => off();
  }, [qc]);
}

const monthOf = (day: string) => formatMonth(day.slice(0, 7));

export function IdeasView({ onOpenNote, onOpenTheme }: Nav) {
  useIdeasEvents();
  const status = useSemanticStatus();
  const { data, isLoading } = useQuery({
    queryKey: ["ideas", "overview", status.state],
    queryFn: api.ideasOverview,
    enabled: status.state !== "off",
  });

  const sections: Array<[string, string, ThemeSummary[]]> = data
    ? [
        ["You keep returning to", "Threads you've come back to across several months.", data.returning],
        ["Ideas forming", "New threads from the last few weeks.", data.forming],
        ["Dormant ideas", "Threads you haven't touched for a while.", data.dormant],
        ["Other threads", "", data.other],
      ]
    : [];
  const themeCount = data ? data.returning.length + data.forming.length + data.dormant.length + data.other.length : 0;

  return (
    <div className="ideas">
      <h2 className="journal-heading">Ideas over time</h2>
      <p className="ideas-lede">
        Patterns found in your notes on this computer. They are suggestions, not conclusions: open a thread to see the
        notes behind it.
      </p>

      {status.state === "off" && <EnableCard />}
      {status.state !== "off" && status.state !== "ready" && <p className="faint">{describeStatus(status)}</p>}
      {data?.stale && (
        <p className="ideas-stale faint">
          <RefreshCcw size={12} aria-hidden /> Updating with your latest notes…
        </p>
      )}
      {isLoading && status.state === "ready" && <p className="faint">Looking for threads…</p>}
      {data?.enabled && themeCount === 0 && !isLoading && (
        <p className="stream-empty">Threads appear once a few notes, written on different days, share an idea.</p>
      )}

      {sections.map(([title, hint, list]) =>
        list.length ? (
          <section key={title} className="ideas-section">
            <h3 className="ideas-section-title">{title}</h3>
            {hint && <p className="ideas-hint faint">{hint}</p>}
            <div className="theme-list">
              {list.map((t) => (
                <ThemeCard key={t.id} theme={t} onOpen={() => onOpenTheme(t.id)} />
              ))}
            </div>
          </section>
        ) : null,
      )}

      {data && data.influences.length > 0 && (
        <section className="ideas-section">
          <h3 className="ideas-section-title">What you read that seems to have shaped your thinking</h3>
          <ul className="influence-list">
            {data.influences.map((inf) => (
              <li key={inf.source.id} className="influence">
                <button className="influence-source" onClick={() => onOpenNote(inf.source.id)}>
                  <BookOpen size={13} aria-hidden />
                  <span>
                    <strong>{inf.source.source?.title || inf.source.title}</strong>
                    <span className="faint"> · {monthOf(inf.source.day)}</span>
                  </span>
                </button>
                <p className="influence-count faint">
                  May have fed into {inf.later.length} later {inf.later.length === 1 ? "note" : "notes"}
                  {inf.themeName && (
                    <>
                      {" "}in{" "}
                      <button className="link-btn" onClick={() => onOpenTheme(inf.themeId!)}>
                        {inf.themeName}
                      </button>
                    </>
                  )}
                </p>
                <ul className="influence-later">
                  {inf.later.slice(0, 3).map((n) => (
                    <li key={n.id}>
                      <button className="link-row" onClick={() => onOpenNote(n.id)}>
                        <CornerDownRight size={12} aria-hidden /> <span className="faint">{monthOf(n.day)}</span>{" "}
                        {short(n)}
                      </button>
                    </li>
                  ))}
                </ul>
              </li>
            ))}
          </ul>
        </section>
      )}

      {data && data.openQuestions.length > 0 && (
        <section className="ideas-section">
          <h3 className="ideas-section-title">Questions you haven't come back to</h3>
          <ul className="question-list">
            {data.openQuestions.map((q) => (
              <li key={q.note.id}>
                <button className="link-row question" onClick={() => onOpenNote(q.note.id)}>
                  <HelpCircle size={13} aria-hidden /> <span>{short(q.note, 140)}</span>
                </button>
                <span className="faint question-meta">
                  {formatDay(q.note.day)}
                  {q.themeName && (
                    <>
                      {" · "}
                      <button className="link-btn" onClick={() => onOpenTheme(q.themeId!)}>
                        {q.themeName}
                      </button>
                    </>
                  )}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function ThemeCard({ theme: t, onOpen }: { theme: ThemeSummary; onOpen: () => void }) {
  return (
    <button className="theme-card" onClick={onOpen}>
      <div className="theme-card-main">
        <span className="theme-name">{t.name}</span>
        <span className="theme-meta faint">
          {t.noteCount} notes · {t.activeMonths} {t.activeMonths === 1 ? "month" : "months"} · {monthOf(t.firstDay)} –{" "}
          {monthOf(t.lastDay)}
        </span>
        {t.labels.length > 0 && (
          <span className="theme-labels">
            {t.labels.map((l) => (
              <span key={l} className="chip chip-small">
                {l}
              </span>
            ))}
          </span>
        )}
      </div>
      <Sparkline months={t.months} />
    </button>
  );
}

function short(n: NoteSummary, max = 90) {
  const text = n.hasTitle ? n.title : plainExcerpt(n.excerpt);
  return text.length > max ? `${text.slice(0, max).trimEnd()}…` : text;
}

/** One thread, month by month: how the idea accumulated and moved. */
export function ThemeView({ id, onBack, onOpenNote }: { id: string; onBack: () => void; onOpenNote: (id: string) => void }) {
  useIdeasEvents();
  const { data, isLoading } = useQuery({ queryKey: ["ideas", "theme", id], queryFn: () => api.themeTimeline(id) });
  if (isLoading) return null;
  if (!data) {
    return (
      <div className="ideas">
        <button className="back-btn" onClick={onBack}>
          <ArrowLeft size={14} aria-hidden /> Ideas over time
        </button>
        <p className="stream-empty">This thread has changed since you opened it. It may have merged with another one.</p>
      </div>
    );
  }
  const t = data.theme;
  const byMonth: Array<[string, typeof data.entries]> = [];
  for (const e of data.entries) {
    const m = e.note.day.slice(0, 7);
    const last = byMonth[byMonth.length - 1];
    if (last && last[0] === m) last[1].push(e);
    else byMonth.push([m, [e]]);
  }
  return (
    <div className="ideas">
      <button className="back-btn" onClick={onBack}>
        <ArrowLeft size={14} aria-hidden /> Ideas over time
      </button>
      <h2 className="journal-heading">{t.name}</h2>
      <p className="theme-meta faint">
        {t.noteCount} notes over {t.activeMonths} {t.activeMonths === 1 ? "month" : "months"} · {t.personalCount} your own,{" "}
        {t.sourceCount} from sources · {monthOf(t.firstDay)} – {monthOf(t.lastDay)}
      </p>
      <div className="theme-header-row">
        {t.labels.map((l) => (
          <span key={l} className="chip chip-small">
            {l}
          </span>
        ))}
        <Sparkline months={t.months} width={240} height={36} />
      </div>

      <ol className="timeline">
        {byMonth.map(([month, entries]) => (
          <li key={month} className="timeline-month">
            <h3 className="timeline-month-title">{formatMonth(month)}</h3>
            <ol>
              {entries.map((e) => (
                <li key={e.note.id} className={`timeline-entry kind-${e.note.kind}`}>
                  <span className="timeline-dot" aria-hidden>
                    {e.note.kind === "source" ? <BookOpen size={11} /> : null}
                  </span>
                  <button className="timeline-note" onClick={() => onOpenNote(e.note.id)}>
                    <span className="timeline-date faint">
                      {formatDay(e.note.day)}
                      {e.note.kind === "source" && e.note.source?.title && ` · from ${e.note.source.title}`}
                    </span>
                    {e.note.hasTitle && <span className="timeline-title">{e.note.title}</span>}
                    <span className="timeline-text">{plainExcerpt(e.note.excerpt)}</span>
                  </button>
                  <div className="timeline-cues">
                    {e.influencedBy && (
                      <button className="cue" onClick={() => onOpenNote(e.influencedBy!.id)}>
                        <BookOpen size={11} aria-hidden /> May build on {e.influencedBy.source?.title || short(e.influencedBy, 40)} (
                        {monthOf(e.influencedBy.day)})
                      </button>
                    )}
                    {e.revisits && (
                      <button className={`cue ${e.mayRevise ? "cue-revise" : ""}`} onClick={() => onOpenNote(e.revisits!.id)}>
                        <CornerDownRight size={11} aria-hidden />
                        {e.mayRevise ? "May rethink" : "Returns to"} your note from {formatDay(e.revisits.day)}
                      </button>
                    )}
                    {e.isQuestion && (
                      <span className="cue cue-static">
                        <HelpCircle size={11} aria-hidden /> Asks a question
                      </span>
                    )}
                  </div>
                </li>
              ))}
            </ol>
          </li>
        ))}
      </ol>
    </div>
  );
}
