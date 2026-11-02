import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "../lib/api";
import { calendarWeeks, level, streaks, thresholds } from "../lib/activity";

const CELL = 11;
const GAP = 2;
const LEFT = 26;
const TOP = 16;
const WEEKDAYS = ["Mon", "", "Wed", "", "Fri", "", ""];
const monthFmt = new Intl.DateTimeFormat(undefined, { month: "short" });
const dayFmt = new Intl.DateTimeFormat(undefined, { weekday: "short", day: "numeric", month: "short", year: "numeric" });
const parse = (d: string) => {
  const [y, m, dd] = d.split("-").map(Number);
  return new Date(y, m - 1, dd);
};

/**
 * A year of writing at a glance: one square per day, darker for more notes,
 * with streaks. Informational only: no goals, reminders or badges.
 */
export function ActivityMap() {
  const today = useMemo(() => new Date(), []);
  const weeks = useMemo(() => calendarWeeks(today), [today]);
  const { data = [] } = useQuery({
    queryKey: ["notes", "activity", weeks[0][0]],
    queryFn: () => api.dayCounts(weeks[0][0]),
  });
  const counts = useMemo(() => new Map(data.map((d) => [d.day, d.count])), [data]);
  const s = streaks(data, today);
  const t = useMemo(() => thresholds(data.map((d) => d.count)), [data]);
  // Viewport coordinates of the hovered square (the tooltip is fixed-positioned
  // so the scrolling chart can't clip it).
  const [hover, setHover] = useState<{ day: string; x: number; y: number } | null>(null);
  const todayIso = weeks[52].find((d) => parse(d).toDateString() === today.toDateString());

  const width = LEFT + weeks.length * (CELL + GAP);
  const height = TOP + 7 * (CELL + GAP);
  const monthLabels = weeks
    .map((w, i) => ({ i, d: parse(w[0]) }))
    .filter(({ i, d }) => i === 0 || d.getDate() <= 7)
    .filter(({ i }) => i < weeks.length - 1);

  const summary = `${s.total} notes on ${s.activeDays} days in the past year. Current streak ${s.current} days, longest ${s.longest} days.`;

  return (
    <section className="activity" aria-label="Writing activity">
      <div className="activity-stats">
        <Stat value={s.total} label={s.total === 1 ? "note this year" : "notes this year"} />
        <Stat value={s.activeDays} label="active days" />
        <Stat value={s.current} label="day streak" accent={s.current > 0} />
        <Stat value={s.longest} label="longest streak" />
      </div>
      <div className="activity-scroll">
        <svg width={width} height={height} role="img" aria-label={summary} className="activity-map">
          {monthLabels.map(({ i, d }) => (
            <text key={i} x={LEFT + i * (CELL + GAP)} y={10} className="activity-label">
              {monthFmt.format(d)}
            </text>
          ))}
          {WEEKDAYS.map((w, r) =>
            w ? (
              <text key={w} x={0} y={TOP + r * (CELL + GAP) + CELL - 2} className="activity-label">
                {w}
              </text>
            ) : null,
          )}
          {weeks.map((week, c) =>
            week.map((day, r) => {
              if (todayIso && day > todayIso) return null;
              const n = counts.get(day) ?? 0;
              const x = LEFT + c * (CELL + GAP);
              const y = TOP + r * (CELL + GAP);
              return (
                <rect
                  key={day}
                  x={x}
                  y={y}
                  width={CELL}
                  height={CELL}
                  rx={2}
                  className={`heat heat-${level(n, t)} ${day === todayIso ? "heat-today" : ""}`}
                  onMouseEnter={(e) => {
                    const r = e.currentTarget.getBoundingClientRect();
                    setHover({ day, x: r.left + r.width / 2, y: r.top });
                  }}
                  onMouseLeave={() => setHover(null)}
                >
                  <title>{`${n === 0 ? "No" : n} ${n === 1 ? "note" : "notes"} · ${dayFmt.format(parse(day))}`}</title>
                </rect>
              );
            }),
          )}
        </svg>
        {hover && (
          <div
            className="activity-tip"
            style={{ left: Math.max(110, Math.min(hover.x, window.innerWidth - 110)), top: hover.y }}
            role="presentation"
          >
            <strong>{counts.get(hover.day) ?? "No"}</strong> {(counts.get(hover.day) ?? 0) === 1 ? "note" : "notes"}
            <span className="faint"> · {dayFmt.format(parse(hover.day))}</span>
          </div>
        )}
      </div>
      <div className="activity-legend" aria-hidden>
        <span className="faint">Less</span>
        {[0, 1, 2, 3, 4].map((l) => (
          <svg key={l} width={CELL} height={CELL}>
            <rect width={CELL} height={CELL} rx={2} className={`heat heat-${l}`} />
          </svg>
        ))}
        <span className="faint">More</span>
      </div>
    </section>
  );
}

function Stat({ value, label, accent }: { value: number; label: string; accent?: boolean }) {
  return (
    <div className="activity-stat">
      <span className={`activity-value ${accent ? "is-accent" : ""}`}>{value.toLocaleString()}</span>
      <span className="activity-stat-label faint">{label}</span>
    </div>
  );
}
