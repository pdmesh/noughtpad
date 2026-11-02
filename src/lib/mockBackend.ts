// In-memory stand-in for the Rust backend, used only when the UI runs in a
// plain browser (no Tauri). It mimics command shapes, not search quality.

import type { DaySummary, EditableMeta, LabelCount, ListQuery, NoteDoc, NoteSummary } from "./api";

interface MockNote {
  id: string;
  created: string;
  content: string;
  labels: string[];
  source: { title?: string | null; url?: string | null } | null;
  kindOverride: "personal" | "source" | null;
}

const seed: Array<[string, string, string[], string | null]> = [
  ["2026-01-14T09:10:00+05:30", "Read about VC power-law returns. A handful of outliers return the fund.", ["VC"], "Venture Deals"],
  ["2026-03-03T22:40:00+05:30", "Ownership may matter more than I expected.\n\nIf one company returns the fund, entry ownership decides *how much* it returns.", ["VC"], null],
  ["2026-06-20T08:05:00+05:30", "Read about portfolio construction — reserves, follow-ons, concentration.", ["VC", "Books"], "Secrets of Sand Hill Road"],
  ["2026-09-12T23:10:00+05:30", "Ownership seems to matter more than I initially thought in VC. Need to understand how this interacts with fund construction.", ["VC", "Product"], null],
  ["2026-09-12T23:25:00+05:30", "Had an idea for a better way to organise presentation software notes.", ["Decaframe"], null],
  ["2026-09-12T23:40:00+05:30", "Need to revisit the Tokyo and Kyoto itinerary.", ["Travel"], null],
  ["2026-09-14T10:00:00+05:30", "Customers often describe what they want rather than what they actually do.\n\n> This seems relevant to how I should interview users for Decaframe.", ["Product", "Books"], "The Mom Test"],
  ["2026-10-02T07:30:00+05:30", "## Is \"good company\" the right VC framework?\n\nThe question is not just whether a company is good, but whether the investment can produce a meaningful fund outcome.\n\n- [ ] compare with portfolio maths\n- [x] reread power-law note", ["VC"], null],
];

let counter = 0;
const notes: MockNote[] = seed.map(([created, content, labels, src]) => ({
  id: `mock-${++counter}`,
  created,
  content,
  labels,
  source: src ? { title: src, url: null } : null,
  kindOverride: null,
}));

const kindOf = (n: MockNote) => n.kindOverride ?? (n.source ? "source" : "personal");
const titleOf = (c: string) =>
  (c.split("\n").find((l) => l.trim()) ?? "").replace(/^[#>\-*+\s]+/, "").replace(/^\[[ x]\]\s/, "").slice(0, 120);

function summary(n: MockNote): NoteSummary {
  return {
    id: n.id,
    path: `${n.created.slice(0, 4)}/${n.created.slice(5, 7)}/${n.id}.md`,
    title: titleOf(n.content),
    excerpt: n.content.trim().slice(0, 320),
    created: n.created,
    day: n.created.slice(0, 10),
    kind: kindOf(n),
    labels: n.labels,
    source: n.source,
  };
}

function doc(n: MockNote): NoteDoc {
  return {
    id: n.id,
    path: summary(n).path,
    content: n.content,
    labels: n.labels,
    source: n.source,
    kindOverride: n.kindOverride,
    kind: kindOf(n),
    created: n.created,
    hash: String(n.content.length) + n.labels.join(),
  };
}

function list(q: ListQuery = {}): NoteSummary[] {
  return notes
    .filter((n) => !q.label || n.labels.some((l) => l.toLowerCase() === q.label!.toLowerCase()))
    .filter((n) => !q.kind || kindOf(n) === q.kind)
    .filter((n) => !q.fromDay || n.created.slice(0, 10) >= q.fromDay)
    .filter((n) => !q.toDay || n.created.slice(0, 10) <= q.toDay)
    .filter((n) => !q.beforeMs || Date.parse(n.created) < q.beforeMs)
    .sort((a, b) => Date.parse(b.created) - Date.parse(a.created))
    .slice(0, q.limit ?? 100)
    .map(summary);
}

function days(q: ListQuery = {}): DaySummary[] {
  const out: DaySummary[] = [];
  for (const n of list({ ...q, limit: 1000 })) {
    const last = out[out.length - 1];
    if (last && last.day === n.day) last.notes.push(n);
    else out.push({ day: n.day, notes: [n] });
  }
  return out;
}

function labels(filter?: (n: MockNote) => boolean): LabelCount[] {
  const m = new Map<string, LabelCount>();
  for (const n of notes.filter(filter ?? (() => true)))
    for (const l of n.labels) {
      const k = l.toLowerCase();
      m.set(k, { label: m.get(k)?.label ?? l, count: (m.get(k)?.count ?? 0) + 1 });
    }
  return [...m.values()].sort((a, b) => b.count - a.count || a.label.localeCompare(b.label));
}

function nowIso(): string {
  const d = new Date();
  const off = -d.getTimezoneOffset();
  const sign = off >= 0 ? "+" : "-";
  const pad = (x: number) => String(Math.floor(Math.abs(x))).padStart(2, "0");
  const local = new Date(d.getTime() + off * 60000).toISOString().slice(0, 19);
  return `${local}${sign}${pad(off / 60)}:${pad(off % 60)}`;
}

function applyMeta(n: MockNote, meta: EditableMeta) {
  n.labels = meta.labels.map((l) => l.trim()).filter(Boolean);
  const s = meta.source;
  n.source = s && (s.title?.trim() || s.url?.trim()) ? s : null;
  n.kindOverride = meta.kindOverride;
}

const get = (id: string) => {
  const n = notes.find((x) => x.id === id);
  if (!n) throw { kind: "notFound", message: id };
  return n;
};

export async function mock(cmd: string, a: Record<string, any> = {}): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 5));
  switch (cmd) {
    case "vault_info":
      return { path: "/mock/Nought", noteCount: notes.length, defaultPath: "/mock/Nought" };
    case "default_vault":
      return "/mock/Nought";
    case "open_vault":
      return { path: a.path, noteCount: notes.length, defaultPath: "/mock/Nought" };
    case "list_notes":
      return list(a.query);
    case "list_days":
      return days(a.query);
    case "list_labels":
      return labels();
    case "list_months":
      return [...new Set(notes.map((n) => n.created.slice(0, 7)))].sort().reverse();
    case "month_log": {
      const inMonth = (n: MockNote) => n.created.startsWith(a.month);
      const ns = notes.filter(inMonth);
      return {
        month: a.month,
        personalCount: ns.filter((n) => kindOf(n) === "personal").length,
        sourceCount: ns.filter((n) => kindOf(n) === "source").length,
        activeDays: new Set(ns.map((n) => n.created.slice(0, 10))).size,
        topLabels: labels(inMonth).slice(0, 8),
        days: days({ kind: "personal", fromDay: `${a.month}-01`, toDay: `${a.month}-31` }),
      };
    }
    case "search_notes": {
      const words = String(a.query).toLowerCase().split(/\s+/).filter(Boolean);
      return list({ limit: 1000 })
        .filter((s) => words.every((w) => (s.excerpt + " " + s.labels.join(" ") + " " + (s.source?.title ?? "")).toLowerCase().includes(w.replace(/^#|^label:/, ""))))
        .map((note) => {
          let snippet = note.excerpt;
          for (const w of words) snippet = snippet.replace(new RegExp(`(${w.replace(/[^\w]/g, "")})`, "ig"), "\u0002$1\u0003");
          return { note, snippet, score: 1 };
        });
    }
    case "get_note":
      return doc(get(a.id));
    case "create_note": {
      const n: MockNote = { id: `mock-${++counter}`, created: nowIso(), content: a.input.content, labels: [], source: null, kindOverride: null };
      applyMeta(n, a.input.meta);
      notes.push(n);
      return doc(n);
    }
    case "update_note": {
      const n = get(a.id);
      if (a.update.content !== undefined && a.update.content !== null) n.content = a.update.content;
      if (a.update.meta) applyMeta(n, a.update.meta);
      return doc(n);
    }
    case "trash_note":
      notes.splice(notes.indexOf(get(a.id)), 1);
      return null;
    case "rebuild_index":
      return { indexed: notes.length, unchanged: 0, removed: 0 };
    case "save_attachment":
      return `attachments/mock-${Date.now()}.${a.extension}`;
    default:
      throw { kind: "internal", message: `mock: unknown command ${cmd}` };
  }
}
