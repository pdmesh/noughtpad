import { useQuery, useQueryClient } from "@tanstack/react-query";
import { BookOpen, Sprout, Tag, X } from "lucide-react";
import { api, type Related } from "../lib/api";
import { describeStatus, useSemanticStatus } from "../lib/useSemantic";
import { formatDay, plainExcerpt } from "../lib/format";

interface Props {
  noteId: string | null;
  onOpen: (id: string) => void;
  onOpenTheme: (id: string) => void;
  onClose: () => void;
  onNotice: (msg: string, undo?: () => void) => void;
}

/**
 * "Explore this note": notes that appear connected, each with the passage that
 * matched and why. Opened on demand only, never while writing.
 */
export function RelatedPanel({ noteId, onOpen, onOpenTheme, onClose, onNotice }: Props) {
  const status = useSemanticStatus();
  const qc = useQueryClient();
  const active = status.state === "ready" || status.state === "indexing";
  const { data, isFetching } = useQuery({
    queryKey: ["related", noteId],
    queryFn: () => api.related(noteId!),
    enabled: !!noteId && active,
    // A just-edited note is embedded a few seconds after typing stops.
    refetchInterval: (q) => (q.state.data?.pending ? 3000 : false),
  });

  const { data: theme } = useQuery({
    queryKey: ["ideas", "note-theme", noteId],
    queryFn: () => api.noteTheme(noteId!),
    enabled: !!noteId && active,
  });

  const dismiss = async (r: Related) => {
    if (!noteId) return;
    await api.dismissRelated(noteId, r.note.id);
    void qc.invalidateQueries({ queryKey: ["related"] });
    onNotice("Hidden. It won't be suggested for this note again.", async () => {
      await api.undismissRelated(noteId, r.note.id);
      void qc.invalidateQueries({ queryKey: ["related"] });
    });
  };

  return (
    <aside className="related" aria-label="Related notes">
      <header className="related-head">
        <h2>Related</h2>
        <button className="icon-btn" onClick={onClose} aria-label="Close related notes" title="Close (Esc)">
          <X size={15} />
        </button>
      </header>

      {theme && (
        <button className="related-theme" onClick={() => onOpenTheme(theme.id)}>
          <Sprout size={13} aria-hidden />
          <span>
            Part of a thread: <strong>{theme.name}</strong>
            <span className="faint">
              {" "}
              · {theme.noteCount} notes over {theme.activeMonths} months
            </span>
          </span>
        </button>
      )}
      {status.state === "off" && <EnableCard />}
      {status.state === "error" && (
        <div className="related-card">
          <p>Semantic search stopped: {status.message}</p>
          <button className="btn" onClick={() => void api.enableSemantic()}>
            Try again
          </button>
        </div>
      )}
      {(status.state === "downloading" || status.state === "loading") && <Progress status={status} />}
      {status.state === "indexing" && <p className="related-hint faint">{describeStatus(status)}</p>}

      {active && !noteId && <p className="related-hint faint">Start writing. Connections appear once the note is saved.</p>}
      {active && data?.pending && (
        <p className="related-hint faint">This note will be connected a few seconds after you pause writing.</p>
      )}
      {active && data && !data.pending && data.items.length === 0 && !isFetching && (
        <p className="related-hint faint">No clear connections yet. They appear as your notebook grows.</p>
      )}

      {active && data?.personalized && (
        <p className="related-hint faint">Tuned to what you open and dismiss here.</p>
      )}
      {active && data && data.items.length > 0 && (
        <ul className="related-list">
          {data.items.map((r) => (
            <li key={r.note.id} className="related-item">
              <div className="related-why">
                <span>{describe(r)}</span>
                <span className="faint"> · {timeRelation(r.daysApart)}</span>
              </div>
              <button
                className="related-link"
                onClick={() => {
                  // Opening a suggestion is the "this was useful" signal; no buttons to press.
                  if (noteId) void api.relatedOpened(noteId, r.note.id);
                  onOpen(r.note.id);
                }}
              >
                {r.note.hasTitle && <span className="related-title">{r.note.title}</span>}
                <blockquote className="related-passage">{plainExcerpt(r.passage)}</blockquote>
              </button>
              {r.sourcePassage && (
                <p className="related-source faint">Matches: “{plainExcerpt(r.sourcePassage)}”</p>
              )}
              <div className="related-meta">
                <span className="faint">{formatDay(r.note.day)}</span>
                {r.sameSource && (
                  <span className="related-tag">
                    <BookOpen size={11} aria-hidden /> Same source
                  </span>
                )}
                {r.sharedLabels.map((l) => (
                  <span key={l} className="related-tag">
                    <Tag size={11} aria-hidden /> {l}
                  </span>
                ))}
                <button className="related-dismiss" onClick={() => void dismiss(r)} title="Don't suggest this connection again">
                  Not related
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
    </aside>
  );
}

export function EnableCard() {
  return (
    <div className="related-card">
      <p>
        Related notes, search by meaning and Ideas over time use a small language model that runs entirely on this
        computer. Your notes never leave it.
      </p>
      <p className="faint">One-time download of about 65 MB from Hugging Face. You can turn it off any time.</p>
      <button className="btn btn-primary" onClick={() => void api.enableSemantic()}>
        Turn on
      </button>
    </div>
  );
}

function Progress({ status }: { status: { state: "downloading"; received: number; total: number } | { state: "loading" } }) {
  const pct = status.state === "downloading" && status.total ? (100 * status.received) / status.total : null;
  return (
    <div className="related-card">
      <p>{describeStatus(status)}</p>
      <div className="progress" role="progressbar" aria-valuenow={pct ?? undefined} aria-valuemin={0} aria-valuemax={100}>
        <div className="progress-bar" style={{ width: `${pct ?? 100}%` }} />
      </div>
    </div>
  );
}

/** Cautious wording: the models suggest, they don't conclude. */
function describe(r: Related): string {
  return r.strength === "strong" ? "These ideas appear connected" : "Possibly related";
}

function timeRelation(days: number): string {
  if (days === 0) return "same day";
  const abs = Math.abs(days);
  const span =
    abs < 14 ? `${abs} day${abs === 1 ? "" : "s"}` : abs < 60 ? `${Math.round(abs / 7)} weeks` : abs < 730 ? `${Math.round(abs / 30)} months` : `${Math.round(abs / 365)} years`;
  return days < 0 ? `${span} earlier` : `${span} later`;
}
