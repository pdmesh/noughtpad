import { useInfiniteQuery } from "@tanstack/react-query";
import { BookOpen } from "lucide-react";
import { api, type ListQuery, type NoteSummary } from "../lib/api";
import { formatDay, formatTime, plainExcerpt } from "../lib/format";

const PAGE = 60;

interface Props {
  query: ListQuery;
  currentId: string | null;
  onOpen: (id: string) => void;
  onLabel?: (label: string) => void;
}

/** The chronological stream: every note, newest first, grouped by day. */
export function Stream({ query, currentId, onOpen, onLabel }: Props) {
  const q = useInfiniteQuery({
    queryKey: ["notes", "stream", query],
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) => api.listNotes({ ...query, beforeMs: pageParam, limit: PAGE }),
    getNextPageParam: (last) => (last.length < PAGE ? undefined : Date.parse(last[last.length - 1].created)),
  });

  const notes = q.data?.pages.flat() ?? [];
  const days = groupByDay(notes);

  if (q.isSuccess && notes.length === 0) {
    return <p className="stream-empty">{query.label ? `No notes labelled “${query.label}” yet.` : "Your notes will appear here."}</p>;
  }

  return (
    <section className="stream" aria-label="Notes">
      {days.map(([day, items]) => (
        <div key={day} className="stream-day">
          <h3 className="stream-day-title">{formatDay(day)}</h3>
          {items.map((n) => (
            <NoteCard key={n.id} note={n} current={n.id === currentId} onOpen={onOpen} onLabel={onLabel} />
          ))}
        </div>
      ))}
      {q.hasNextPage && (
        <button className="load-more" onClick={() => q.fetchNextPage()} disabled={q.isFetchingNextPage}>
          {q.isFetchingNextPage ? "Loading…" : "Older notes"}
        </button>
      )}
    </section>
  );
}

export function NoteCard({
  note,
  current,
  onOpen,
  onLabel,
  showTime = true,
}: {
  note: NoteSummary;
  current?: boolean;
  onOpen: (id: string) => void;
  onLabel?: (label: string) => void;
  showTime?: boolean;
}) {
  const text = plainExcerpt(note.excerpt);
  return (
    <div
      role="button"
      tabIndex={0}
      className={`note-card ${current ? "is-current" : ""} kind-${note.kind}`}
      onClick={() => onOpen(note.id)}
      onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onOpen(note.id))}
    >
      <div className="note-card-text">{text || <span className="faint">Empty note</span>}</div>
      <div className="note-card-meta">
        {showTime && <span className="faint">{formatTime(note.created)}</span>}
        {note.source && (
          <span className="note-card-source">
            <BookOpen size={12} aria-hidden /> {note.source.title || hostOf(note.source.url)}
          </span>
        )}
        {note.labels.map((l) => (
          <button
            key={l}
            className="chip chip-small"
            onClick={(e) => {
              e.stopPropagation();
              onLabel?.(l);
            }}
          >
            {l}
          </button>
        ))}
      </div>
    </div>
  );
}

function hostOf(url?: string | null) {
  if (!url) return "";
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

function groupByDay(notes: NoteSummary[]): Array<[string, NoteSummary[]]> {
  const out: Array<[string, NoteSummary[]]> = [];
  for (const n of notes) {
    const last = out[out.length - 1];
    if (last && last[0] === n.day) last[1].push(n);
    else out.push([n.day, [n]]);
  }
  return out;
}
