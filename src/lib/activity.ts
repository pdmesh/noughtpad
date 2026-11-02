// Pure helpers for the activity map: the calendar grid, intensity levels and streaks.

export interface DayCount {
  day: string;
  count: number;
}

export const isoDay = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;

/** 53 weeks (Monday-first columns) ending with the week containing `today`. */
export function calendarWeeks(today: Date): string[][] {
  const end = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  const monday = new Date(end);
  monday.setDate(end.getDate() - ((end.getDay() + 6) % 7));
  const start = new Date(monday);
  start.setDate(monday.getDate() - 52 * 7);
  const weeks: string[][] = [];
  for (let w = 0; w < 53; w++) {
    const week: string[] = [];
    for (let d = 0; d < 7; d++) {
      const day = new Date(start);
      day.setDate(start.getDate() + w * 7 + d);
      week.push(isoDay(day));
    }
    weeks.push(week);
  }
  return weeks;
}

/**
 * Level thresholds from the writer's own activity (quartiles of active days),
 * so the map shows variation whether someone writes one note a week or ten a
 * day. With little data, falls back to 1 / 2 / 3–4 / 5+.
 */
export function thresholds(counts: number[]): [number, number, number] {
  const active = counts.filter((c) => c > 0).sort((a, b) => a - b);
  if (active.length < 8) return [2, 3, 5];
  const q = (p: number) => active[Math.min(active.length - 1, Math.floor(p * active.length))];
  const t1 = Math.max(2, q(0.25) + 1);
  const t2 = Math.max(t1 + 1, q(0.5) + 1);
  const t3 = Math.max(t2 + 1, q(0.75) + 1);
  return [t1, t2, t3];
}

/** 0 = none; 1..4 = increasing activity relative to `t` (from `thresholds`). */
export function level(count: number, t: [number, number, number] = [2, 3, 5]): 0 | 1 | 2 | 3 | 4 {
  if (count <= 0) return 0;
  if (count < t[0]) return 1;
  if (count < t[1]) return 2;
  if (count < t[2]) return 3;
  return 4;
}

export interface Streaks {
  current: number;
  longest: number;
  activeDays: number;
  total: number;
}

/**
 * Current streak counts back from today, or from yesterday if nothing has been
 * written yet today, so the streak doesn't "break" first thing in the morning.
 */
export function streaks(counts: DayCount[], today: Date): Streaks {
  const active = new Set(counts.filter((c) => c.count > 0).map((c) => c.day));
  const step = (d: Date, n: number) => {
    const x = new Date(d);
    x.setDate(d.getDate() + n);
    return x;
  };
  let cursor = new Date(today.getFullYear(), today.getMonth(), today.getDate());
  if (!active.has(isoDay(cursor))) cursor = step(cursor, -1);
  let current = 0;
  while (active.has(isoDay(cursor))) {
    current++;
    cursor = step(cursor, -1);
  }
  // Longest run of consecutive calendar days.
  let longest = 0;
  let run = 0;
  let prev: Date | null = null;
  for (const d of [...active].sort()) {
    const day = parse(d);
    run = prev && isoDay(step(prev, 1)) === d ? run + 1 : 1;
    longest = Math.max(longest, run);
    prev = day;
  }
  return { current, longest, activeDays: active.size, total: counts.reduce((n, c) => n + c.count, 0) };
}

function parse(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, m - 1, d);
}
