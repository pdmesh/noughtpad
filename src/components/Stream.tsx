import { useLayoutEffect, useRef, useState, useEffect } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { useWindowVirtualizer } from "@tanstack/react-virtual";
import { BookOpen } from "lucide-react";
import { api, type ListQuery, type NoteSummary } from "../lib/api";
import { formatDay, formatTime, formatWeek, plainExcerpt, weekStart } from "../lib/format";

/** Notes fetched per page (keyset pagination on creation time). */
const PAGE = 100;
/** Start fetching the next page when this many rows remain below the viewport. */
const PREFETCH_ROWS = 20;

type Row =
  | { kind: "week"; key: string; start: string }
  | { kind: "day"; key: string; day: string }
  | { kind: "note"; key: string; note: NoteSummary };

interface Props {
  query: ListQuery;
  currentId?: string | null;
  onOpen: (id: string) => void;
  onLabel?: (label: string) => void;
  /** Add week headings above the day headings. */
  groupByWeek?: boolean;
  emptyText?: string;
}

/**
 * A chronological, infinitely scrolling list of notes, newest first, grouped by day.
 * Pages are fetched on demand and only rows near the viewport are rendered, so
 * it stays light with tens of thousands of notes.
 */
export function Stream({ query, currentId, onOpen, onLabel, groupByWeek, emptyText }: Props) {
  const q = useInfiniteQuery({
    queryKey: ["notes", "stream", query],
    initialPageParam: undefined as number | undefined,
    queryFn: ({ pageParam }) => api.listNotes({ ...query, beforeMs: pageParam, limit: PAGE }),
    getNextPageParam: (last) => (last.length < PAGE ? undefined : Date.parse(last[last.length - 1].created)),
  });

  const notes = q.data?.pages.flat() ?? [];
  const rows = toRows(notes, !!groupByWeek);

  // The list scrolls with the window; the virtualizer needs the list's offset in the page.
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  useLayoutEffect(() => {
    const el = listRef.current;
    if (!el) return;
    const update = () => setScrollMargin(el.getBoundingClientRect().top + window.scrollY);
    update();
    const ro = new ResizeObserver(update);
    ro.observe(document.body);
    return () => ro.disconnect();
  }, []);

  const virtualizer = useWindowVirtualizer({
    count: rows.length,
    estimateSize: (i) => (rows[i].kind === "note" ? 96 : rows[i].kind === "day" ? 44 : 56),
    overscan: 10,
    scrollMargin,
    getItemKey: (i) => rows[i].key,
  });
  const items = virtualizer.getVirtualItems();
  const lastIndex = items.length ? items[items.length - 1].index : 0;

  useEffect(() => {
    if (q.hasNextPage && !q.isFetchingNextPage && lastIndex >= rows.length - PREFETCH_ROWS) void q.fetchNextPage();
  }, [lastIndex, rows.length, q]);

  if (q.isSuccess && notes.length === 0) {
    return <p className="stream-empty">{emptyText ?? "Your notes will appear here."}</p>;
  }

  return (
    <section className="stream" aria-label="Notes" aria-busy={q.isFetching}>
      <div ref={listRef} style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {items.map((item) => {
          const row = rows[item.index];
          return (
            <div
              key={item.key}
              data-index={item.index}
              ref={virtualizer.measureElement}
              className="stream-row"
              style={{ transform: `translateY(${item.start - virtualizer.options.scrollMargin}px)` }}
            >
              {row.kind === "week" && <h3 className="journal-week-title">{formatWeek(row.start)}</h3>}
              {row.kind === "day" && <h4 className="stream-day-title">{formatDay(row.day)}</h4>}
              {row.kind === "note" && (
                <NoteCard note={row.note} current={row.note.id === currentId} onOpen={onOpen} onLabel={onLabel} />
              )}
            </div>
          );
        })}
      </div>
      {q.isFetchingNextPage && <p className="stream-loading faint">Loading older notes…</p>}
    </section>
  );
}

function toRows(notes: NoteSummary[], weeks: boolean): Row[] {
  const rows: Row[] = [];
  let day = "";
  let week = "";
  for (const n of notes) {
    if (weeks) {
      const w = weekStart(n.day);
      if (w !== week) {
        week = w;
        rows.push({ kind: "week", key: `w:${w}`, start: w });
      }
    }
    if (n.day !== day) {
      day = n.day;
      rows.push({ kind: "day", key: `d:${n.day}`, day: n.day });
    }
    rows.push({ kind: "note", key: n.id, note: n });
  }
  return rows;
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
      {note.hasTitle && <div className="note-card-title">{note.title}</div>}
      <div className={`note-card-text ${note.hasTitle ? "has-title" : ""}`}>
        {text || <span className="faint">Empty note</span>}
      </div>
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
