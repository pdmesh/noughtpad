const dayFmt = new Intl.DateTimeFormat(undefined, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
const monthFmt = new Intl.DateTimeFormat(undefined, { month: "long", year: "numeric" });
const timeFmt = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const dateTimeFmt = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", hour: "numeric", minute: "2-digit" });

/** `YYYY-MM-DD` as a calendar date (no timezone shifting). */
function dayToDate(day: string): Date {
  const [y, m, d] = day.split("-").map(Number);
  return new Date(y, (m ?? 1) - 1, d ?? 1);
}

export function formatDay(day: string): string {
  const today = new Date();
  const iso = (d: Date) =>
    `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  if (day === iso(today)) return "Today";
  const y = new Date(today);
  y.setDate(today.getDate() - 1);
  if (day === iso(y)) return "Yesterday";
  return dayFmt.format(dayToDate(day));
}

export function formatMonth(month: string): string {
  return monthFmt.format(dayToDate(`${month}-01`));
}

/** Time as written: uses the wall-clock time stored in the note, not the viewer's zone. */
export function formatTime(rfc3339: string): string {
  const m = /T(\d{2}):(\d{2})/.exec(rfc3339);
  if (!m) return "";
  const d = new Date(2000, 0, 1, Number(m[1]), Number(m[2]));
  return timeFmt.format(d);
}

export function formatDateTime(rfc3339: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(rfc3339);
  if (!m) return rfc3339;
  return dateTimeFmt.format(new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5]));
}

/** Split a backend snippet into [text, isMatch] parts (matches are wrapped in \u0002…\u0003). */
export function snippetParts(snippet: string): Array<[string, boolean]> {
  const out: Array<[string, boolean]> = [];
  const re = /\u0002([^\u0003]*)\u0003/g;
  let last = 0;
  for (let m = re.exec(snippet); m; m = re.exec(snippet)) {
    if (m.index > last) out.push([snippet.slice(last, m.index), false]);
    out.push([m[1], true]);
    last = re.lastIndex;
  }
  if (last < snippet.length) out.push([snippet.slice(last), false]);
  return out;
}

/** A light plain-text rendering of a Markdown excerpt for list views. */
export function plainExcerpt(md: string): string {
  return md
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^\s{0,3}(#{1,6}|>|[-*+]\s+\[[ xX]\]|[-*+]|\d+\.)\s+/gm, "")
    .replace(/(\*\*|__|\*|_|~~|`)/g, "")
    .replace(/\s+/g, " ")
    .trim();
}
