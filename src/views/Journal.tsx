import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, type DaySummary } from "../lib/api";
import { formatDay, formatMonth, formatWeek, weekStart } from "../lib/format";
import { NoteCard } from "../components/Stream";

type Mode = "day" | "week" | "month";

interface Props {
  onOpen: (id: string) => void;
  onLabel: (label: string) => void;
}

function readMode(): Mode {
  try {
    const m = localStorage.getItem("nought.journalMode");
    if (m === "day" || m === "week" || m === "month") return m;
  } catch {
    /* storage unavailable */
  }
  return "day";
}

/**
 * Journal = a view over personal notes, never a separate store.
 * Day, week and month groupings are deterministic. AI reflection will be an
 * explicit, on-demand action in a later milestone.
 */
export function Journal({ onOpen, onLabel }: Props) {
  const [mode, setModeState] = useState<Mode>(readMode);
  const setMode = (m: Mode) => {
    setModeState(m);
    try {
      localStorage.setItem("nought.journalMode", m);
    } catch {
      /* storage unavailable */
    }
  };

  return (
    <div className="journal">
      <div className="journal-head">
        <h2 className="journal-heading">Journal</h2>
        <div className="segmented" role="tablist" aria-label="Journal grouping">
          {(["day", "week", "month"] as const).map((m) => (
            <button key={m} role="tab" aria-selected={mode === m} className={mode === m ? "is-current" : ""} onClick={() => setMode(m)}>
              {{ day: "Days", week: "Weeks", month: "Months" }[m]}
            </button>
          ))}
        </div>
      </div>
      {mode === "month" ? <MonthView onOpen={onOpen} onLabel={onLabel} /> : <DayWeekView mode={mode} onOpen={onOpen} onLabel={onLabel} />}
    </div>
  );
}

function DayList({ days, onOpen, onLabel }: { days: DaySummary[] } & Props) {
  return (
    <>
      {days.map((d) => (
        <section key={d.day} className="stream-day">
          <h3 className="stream-day-title">{formatDay(d.day)}</h3>
          {[...d.notes].reverse().map((n) => (
            <NoteCard key={n.id} note={n} onOpen={onOpen} onLabel={onLabel} />
          ))}
        </section>
      ))}
    </>
  );
}

function DayWeekView({ mode, onOpen, onLabel }: { mode: "day" | "week" } & Props) {
  const { data: days = [], isSuccess } = useQuery({
    queryKey: ["notes", "journal-days"],
    queryFn: () => api.days({ kind: "personal", limit: 2000 }),
  });
  if (isSuccess && days.length === 0) return <Empty />;
  if (mode === "day") return <DayList days={days} onOpen={onOpen} onLabel={onLabel} />;

  const weeks: Array<{ start: string; days: DaySummary[] }> = [];
  for (const d of days) {
    const start = weekStart(d.day);
    const last = weeks[weeks.length - 1];
    if (last && last.start === start) last.days.push(d);
    else weeks.push({ start, days: [d] });
  }
  return (
    <>
      {weeks.map((w) => {
        const count = w.days.reduce((n, d) => n + d.notes.length, 0);
        return (
          <section key={w.start} className="journal-week">
            <h3 className="journal-week-title">
              {formatWeek(w.start)} <span className="faint">· {count} {count === 1 ? "note" : "notes"}</span>
            </h3>
            <DayList days={w.days} onOpen={onOpen} onLabel={onLabel} />
          </section>
        );
      })}
    </>
  );
}

function MonthView({ onOpen, onLabel }: Props) {
  const { data: months = [], isSuccess } = useQuery({ queryKey: ["notes", "months"], queryFn: api.months });
  const [month, setMonth] = useState<string | null>(null);
  useEffect(() => {
    if (!month && months.length) setMonth(months[0]);
  }, [month, months]);

  const { data: log } = useQuery({
    queryKey: ["notes", "month", month],
    queryFn: () => api.monthLog(month!),
    enabled: !!month,
  });

  if (isSuccess && !months.length) return <Empty />;

  return (
    <>
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
          {log.days.length === 0 && <p className="faint">No personal notes this month, only source notes.</p>}
          <DayList days={log.days} onOpen={onOpen} onLabel={onLabel} />
        </div>
      )}
    </>
  );
}

function Empty() {
  return <p className="stream-empty">Your personal notes build the journal over time.</p>;
}
