import type { MonthCount } from "../lib/api";
import { formatMonth } from "../lib/format";

interface Props {
  months: MonthCount[];
  width?: number;
  height?: number;
}

/**
 * Notes per month for the last 12 months: muted bars, the current month in the
 * accent. Each bar has a hover tooltip; the whole chart has a text summary.
 */
export function Sparkline({ months, width = 132, height = 28 }: Props) {
  const max = Math.max(1, ...months.map((m) => m.count));
  const gap = 2;
  const bar = (width - gap * (months.length - 1)) / months.length;
  const active = months.filter((m) => m.count > 0);
  const summary =
    active.length === 0
      ? "No notes in the last 12 months"
      : `Notes per month: ${active.map((m) => `${formatMonth(m.month)} ${m.count}`).join(", ")}`;
  return (
    <svg className="sparkline" width={width} height={height} viewBox={`0 0 ${width} ${height}`} role="img" aria-label={summary}>
      {months.map((m, i) => {
        const h = m.count === 0 ? 1 : Math.max(3, (m.count / max) * (height - 2));
        const last = i === months.length - 1;
        return (
          <rect
            key={m.month}
            x={i * (bar + gap)}
            y={height - h}
            width={bar}
            height={h}
            rx={m.count === 0 ? 0 : 1.5}
            className={m.count === 0 ? "spark-empty" : last ? "spark-now" : "spark-bar"}
          >
            <title>{`${formatMonth(m.month)}: ${m.count} ${m.count === 1 ? "note" : "notes"}`}</title>
          </rect>
        );
      })}
    </svg>
  );
}
