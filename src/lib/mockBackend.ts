// In-memory stand-in for the Rust backend, used only when the UI runs in a
// plain browser (no Tauri). It mimics command shapes, not search quality.

import type { DaySummary, EditableMeta, LabelCount, ListQuery, NoteDoc, NoteSummary, Related, RelatedNotes, SemanticStatus } from "./api";

interface MockNote {
  id: string;
  title?: string | null;
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

// `?mock=many` adds thousands of notes to exercise the virtualised lists.
if (typeof location !== "undefined" && new URLSearchParams(location.search).get("mock") === "many") {
  const words = "ownership fund portfolio returns product customer travel idea question thinking book decision".split(" ");
  const start = Date.parse("2026-10-30T12:00:00+05:30");
  for (let i = 0; i < 5000; i++) {
    const d = new Date(start - i * 3.7 * 3600_000);
    const iso = new Date(d.getTime() + 330 * 60000).toISOString().slice(0, 19) + "+05:30";
    notes.push({
      id: `many-${i}`,
      created: iso,
      content: Array.from({ length: 12 + (i % 30) }, (_, k) => words[(i * 7 + k * 5) % words.length]).join(" "),
      labels: i % 3 ? [`L${i % 9}`] : [],
      source: i % 4 === 0 ? { title: `Book ${i % 50}`, url: null } : null,
      kindOverride: null,
      title: i % 10 === 0 ? `Titled note ${i}` : null,
    });
  }
}

const kindOf = (n: MockNote) => n.kindOverride ?? (n.source ? "source" : "personal");
const titleOf = (c: string) =>
  (c.split("\n").find((l) => l.trim()) ?? "").replace(/^[#>\-*+\s]+/, "").replace(/^\[[ x]\]\s/, "").slice(0, 120);

function summary(n: MockNote): NoteSummary {
  return {
    id: n.id,
    path: `${n.created.slice(0, 4)}/${n.created.slice(5, 7)}/${n.id}.md`,
    title: n.title || titleOf(n.content),
    hasTitle: !!n.title,
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
    title: n.title ?? null,
    labels: n.labels,
    source: n.source,
    kindOverride: n.kindOverride,
    kind: kindOf(n),
    created: n.created,
    hash: String(n.content.length) + n.labels.join() + (n.title ?? ""),
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
  n.title = meta.title?.trim() || null;
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

// ---- semantic features (simulated with word overlap) ----------------------

type Listener = (payload: unknown) => void;
const listeners = new Map<string, Set<Listener>>();

export function mockOn(name: string, handler: Listener): () => void {
  if (!listeners.has(name)) listeners.set(name, new Set());
  listeners.get(name)!.add(handler);
  return () => listeners.get(name)!.delete(handler);
}

function emit(name: string, payload: unknown) {
  listeners.get(name)?.forEach((h) => h(payload));
}

let semantic: SemanticStatus = { state: "off" };
function setSemantic(s: SemanticStatus) {
  semantic = s;
  emit("semantic-status", s);
}

async function simulateEnable() {
  const total = 34_000_000;
  for (let r = 0; r <= total; r += 4_250_000) {
    setSemantic({ state: "downloading", received: r, total });
    await new Promise((res) => setTimeout(res, 120));
  }
  setSemantic({ state: "loading" });
  await new Promise((res) => setTimeout(res, 200));
  for (let d = 0; d <= notes.length; d += Math.max(1, Math.ceil(notes.length / 5))) {
    setSemantic({ state: "indexing", done: d, total: notes.length });
    await new Promise((res) => setTimeout(res, 150));
  }
  setSemantic({ state: "ready", notes: notes.length });
}

const dismissed = new Set<string>();
const pairKey = (a: string, b: string) => [a, b].sort().join("|");
const stop = new Set("the and that this with for are was not but what they how have from more than just into about".split(" "));
const wordsOf = (t: string) =>
  new Set(t.toLowerCase().split(/[^a-z0-9]+/).filter((w) => w.length > 3 && !stop.has(w)).map((w) => w.replace(/s$/, "")));

function related(id: string, limit: number): RelatedNotes {
  if (semantic.state !== "ready" && semantic.state !== "indexing") return { enabled: false, pending: false, items: [] };
  const me = get(id);
  const mine = wordsOf(me.content);
  const items: Related[] = notes
    .filter((n) => n.id !== id && !dismissed.has(pairKey(id, n.id)))
    .map((n) => {
      const theirs = wordsOf(n.content);
      const shared = [...mine].filter((w) => theirs.has(w)).length;
      const score = shared / Math.sqrt(Math.max(1, mine.size * theirs.size));
      return { n, score };
    })
    .filter((x) => x.score >= 0.12)
    .sort((a, b) => b.score - a.score)
    .slice(0, limit)
    .map(({ n, score }) => ({
      note: summary(n),
      score,
      strength: score >= 0.25 ? "strong" : "possible",
      passage: n.content.slice(0, 280),
      sourcePassage: null,
      sharedLabels: n.labels.filter((l) => me.labels.some((m) => m.toLowerCase() === l.toLowerCase())),
      sameSource: me.source?.title && n.source?.title === me.source.title ? n.source.title : null,
      daysApart: Math.round((Date.parse(n.created) - Date.parse(me.created)) / 86_400_000),
    }));
  return { enabled: true, pending: false, items };
}

// ---- ideas over time (mock: one thread per label with 3+ notes) -------------

interface MockTheme {
  id: string;
  name: string;
  label: string;
  members: MockNote[];
}

function mockThemes(): MockTheme[] {
  if (semantic.state !== "ready") return [];
  const by = new Map<string, MockNote[]>();
  for (const n of notes) {
    const l = n.labels[0];
    if (l) by.set(l, [...(by.get(l) ?? []), n]);
  }
  return [...by.entries()]
    .filter(([, ms]) => ms.length >= 3)
    .map(([label, ms]) => {
      const members = [...ms].sort((a, b) => Date.parse(a.created) - Date.parse(b.created));
      const words = [...wordsOf(members.map((m) => m.content).join(" "))].slice(0, 3);
      return { id: `t-${label.toLowerCase()}`, label, members, name: words.join(" · ") };
    });
}

function themeSummary(t: MockTheme) {
  const months = new Map<string, number>();
  for (const m of t.members) months.set(m.created.slice(0, 7), (months.get(m.created.slice(0, 7)) ?? 0) + 1);
  const now = new Date();
  const last12 = Array.from({ length: 12 }, (_, i) => {
    const d = new Date(now.getFullYear(), now.getMonth() - 11 + i, 1);
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
    return { month: key, count: months.get(key) ?? 0 };
  });
  const personal = t.members.filter((m) => kindOf(m) === "personal").length;
  return {
    id: t.id,
    name: t.name,
    keywords: t.name.split(" · "),
    labels: [t.label],
    noteCount: t.members.length,
    personalCount: personal,
    sourceCount: t.members.length - personal,
    firstDay: t.members[0].created.slice(0, 10),
    lastDay: t.members[t.members.length - 1].created.slice(0, 10),
    activeMonths: months.size,
    months: last12,
  };
}

function ideasOverview() {
  const themes = mockThemes();
  const enabled = semantic.state !== "off";
  const influences = themes.flatMap((t) => {
    const src = t.members.find((m) => kindOf(m) === "source");
    if (!src) return [];
    const later = t.members.filter((m) => kindOf(m) === "personal" && Date.parse(m.created) > Date.parse(src.created));
    return later.length ? [{ source: summary(src), later: later.map(summary), themeId: t.id, themeName: t.name }] : [];
  });
  const openQuestions = notes
    .filter((n) => kindOf(n) === "personal" && /\w\?/.test(n.content))
    .map((n) => {
      const t = themes.find((t) => t.members.includes(n));
      return { note: summary(n), themeId: t?.id ?? null, themeName: t?.name ?? null };
    });
  return {
    enabled,
    stale: false,
    returning: themes.filter((t) => new Set(t.members.map((m) => m.created.slice(0, 7))).size >= 3).map(themeSummary),
    forming: [],
    dormant: [],
    other: themes.filter((t) => new Set(t.members.map((m) => m.created.slice(0, 7))).size < 3).map(themeSummary),
    influences,
    openQuestions,
  };
}

function themeTimeline(id: string) {
  const t = mockThemes().find((t) => t.id === id);
  if (!t) return null;
  return {
    theme: themeSummary(t),
    entries: t.members.map((m, i) => {
      const earlierSource = t.members.slice(0, i).reverse().find((x) => kindOf(x) === "source");
      const earlierPersonal = t.members.slice(0, i).reverse().find((x) => kindOf(x) === "personal");
      const personal = kindOf(m) === "personal";
      return {
        note: summary(m),
        influencedBy: personal && earlierSource ? summary(earlierSource) : null,
        revisits: personal && earlierPersonal ? summary(earlierPersonal) : null,
        mayRevise: personal && /\b(but|however|tension)\b/i.test(m.content),
        isQuestion: /\w\?/.test(m.content),
      };
    }),
  };
}

export async function mock(cmd: string, a: Record<string, any> = {}): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 5));
  switch (cmd) {
    case "vault_info":
      return { path: "/mock/Nought", noteCount: notes.length, defaultPath: "/mock/Nought" };
    case "mark_ready":
      return Math.round(performance.now());
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
          return { note, snippet, score: 1, matched: semantic.state === "ready" ? "both" : "text" };
        });
    }
    case "semantic_status":
      return semantic;
    case "enable_semantic":
      void simulateEnable();
      return semantic;
    case "disable_semantic":
      setSemantic({ state: "off" });
      return semantic;
    case "ideas_overview":
      return ideasOverview();
    case "theme_timeline":
      return themeTimeline(a.id);
    case "note_theme": {
      const n = notes.find((x) => x.id === a.id);
      const t = mockThemes().find((t) => n && t.members.includes(n));
      return t ? themeSummary(t) : null;
    }
    case "related_notes":
      return related(a.id, a.limit ?? 8);
    case "dismiss_related":
      dismissed.add(pairKey(a.a, a.b));
      return null;
    case "undismiss_related":
      dismissed.delete(pairKey(a.a, a.b));
      return null;
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
