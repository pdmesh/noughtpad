import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";
import { formatMonth } from "../lib/format";
import { Stream } from "../components/Stream";

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

function DayWeekView({ mode, onOpen, onLabel }: { mode: "day" | "week" } & Props) {
  return (
    <Stream
      query={{ kind: "personal" }}
      groupByWeek={mode === "week"}
      onOpen={onOpen}
      onLabel={onLabel}
      emptyText="Your personal notes build the journal over time."
    />
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
          {log.personalCount === 0 ? (
            <p className="faint">No personal notes this month, only source notes.</p>
          ) : (
            <Stream
              key={log.month}
              query={{ kind: "personal", fromDay: `${log.month}-01`, toDay: `${log.month}-31` }}
              onOpen={onOpen}
              onLabel={onLabel}
            />
          )}
        </div>
      )}
    </>
  );
}

function Empty() {
  return <p className="stream-empty">Your personal notes build the journal over time.</p>;
}
