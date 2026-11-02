import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";
import { formatDay, formatMonth } from "../lib/format";
import { NoteCard } from "../components/Stream";

interface Props {
  onOpen: (id: string) => void;
  onLabel: (label: string) => void;
}

/**
 * Journal = a view over personal notes, never a separate store.
 * The monthly log is assembled deterministically (counts, labels, days);
 * AI reflection will be an explicit, on-demand action in a later milestone.
 */
export function Journal({ onOpen, onLabel }: Props) {
  const { data: months = [] } = useQuery({ queryKey: ["notes", "months"], queryFn: api.months });
  const [month, setMonth] = useState<string | null>(null);
  useEffect(() => {
    if (!month && months.length) setMonth(months[0]);
  }, [month, months]);

  const { data: log } = useQuery({
    queryKey: ["notes", "month", month],
    queryFn: () => api.monthLog(month!),
    enabled: !!month,
  });

  if (!months.length) return <p className="stream-empty">Personal notes will build your journal over time.</p>;

  return (
    <div className="journal">
      <nav className="journal-months" aria-label="Months">
        {months.map((m) => (
          <button key={m} className={`journal-month ${m === month ? "is-current" : ""}`} onClick={() => setMonth(m)}>
            {formatMonth(m)}
          </button>
        ))}
      </nav>
      {log && (
        <div className="journal-body">
          <h2 className="journal-title">{formatMonth(log.month)}</h2>
          <p className="journal-stats">
            {log.personalCount} personal {log.personalCount === 1 ? "note" : "notes"} · {log.sourceCount} from sources ·{" "}
            {log.activeDays} active {log.activeDays === 1 ? "day" : "days"}
          </p>
          {log.topLabels.length > 0 && (
            <div className="journal-labels">
              <span className="faint">Most on your mind:</span>
              {log.topLabels.map((l) => (
                <button key={l.label} className="chip" onClick={() => onLabel(l.label)}>
                  {l.label} <span className="faint">{l.count}</span>
                </button>
              ))}
            </div>
          )}
          {log.days.length === 0 && <p className="faint">No personal notes this month — only source notes.</p>}
          {log.days.map((d) => (
            <section key={d.day} className="stream-day">
              <h3 className="stream-day-title">{formatDay(d.day)}</h3>
              {[...d.notes].reverse().map((n) => (
                <NoteCard key={n.id} note={n} onOpen={onOpen} onLabel={onLabel} />
              ))}
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
