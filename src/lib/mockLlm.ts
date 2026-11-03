// Browser-only stand-in for the M4 commands (models, providers, thinking).
// Downloads and answers are simulated; the privacy rules mirror the backend.

import type { ActiveModel, Chat, ChatNote, ChatScope, LlmOverview, LocalModelInfo, NoteSummary, Remote, RemoteInput } from "./api";

export interface MockHost {
  emit: (name: string, payload: unknown) => void;
  /** Newest first. */
  summaries: () => NoteSummary[];
  body: (id: string) => string;
}

const catalog: LocalModelInfo[] = [
  {
    id: "qwen3-4b-instruct-2507-q4km",
    name: "Qwen3 4B Instruct",
    maker: "Alibaba Qwen",
    licence: "Apache-2.0",
    repo: "unsloth/Qwen3-4B-Instruct-2507-GGUF",
    revision: "a06e946bb6b655725eafa393f4a9745d460374c9",
    file: "Qwen3-4B-Instruct-2507-Q4_K_M.gguf",
    sha256: "",
    bytes: 2_497_281_120,
    ramGb: 4,
    blurb: "Best balance: cites your notes reliably and runs on most laptops.",
    recommended: true,
  },
  {
    id: "qwen3-1.7b-q4km",
    name: "Qwen3 1.7B",
    maker: "Alibaba Qwen",
    licence: "Apache-2.0",
    repo: "unsloth/Qwen3-1.7B-GGUF",
    revision: "d7f544eead698dbd1f15126ef60b45a1e1933222",
    file: "Qwen3-1.7B-Q4_K_M.gguf",
    sha256: "",
    bytes: 1_107_409_472,
    ramGb: 2,
    blurb: "Smaller and faster, for older machines. Answers are simpler.",
    recommended: false,
  },
  {
    id: "qwen3.5-4b-q4km",
    name: "Qwen3.5 4B",
    maker: "Alibaba Qwen",
    licence: "Apache-2.0",
    repo: "unsloth/Qwen3.5-4B-GGUF",
    revision: "e87f176479d0855a907a41277aca2f8ee7a09523",
    file: "Qwen3.5-4B-Q4_K_M.gguf",
    sha256: "",
    bytes: 2_740_937_888,
    ramGb: 4.5,
    blurb: "Newer model; noticeably slower without a GPU.",
    recommended: false,
  },
];

const providers: LlmOverview["providers"] = [
  { kind: "anthropic", label: "Anthropic", needsKey: true, needsUrl: false, defaultUrl: null },
  { kind: "openAi", label: "OpenAI", needsKey: true, needsUrl: false, defaultUrl: null },
  { kind: "gemini", label: "Google Gemini", needsKey: true, needsUrl: false, defaultUrl: null },
  { kind: "ollama", label: "Ollama", needsKey: false, needsUrl: false, defaultUrl: "http://localhost:11434" },
  { kind: "openAiCompatible", label: "OpenAI-compatible", needsKey: false, needsUrl: true, defaultUrl: "http://localhost:1234/v1" },
];

const remoteModels: Record<string, string[]> = {
  anthropic: ["claude-haiku-5-5", "claude-opus-5-5", "claude-sonnet-5-5"],
  openAi: ["gpt-5-mini", "gpt-5.1"],
  gemini: ["gemini-3-flash", "gemini-3-pro"],
  ollama: ["llama4:8b", "qwen3:4b"],
  openAiCompatible: ["local-model"],
};

// `?llm=ready` starts with the recommended model downloaded and in use.
const preset = typeof location !== "undefined" ? new URLSearchParams(location.search).get("llm") : null;
const downloaded = new Set<string>(preset === "ready" ? [catalog[0].id] : []);
const received = new Map<string, number>();
const downloading = new Map<string, ReturnType<typeof setInterval>>();
const remotes: Remote[] = [];
const keys = new Map<string, string>();
let active: ActiveModel | null = preset === "ready" ? { kind: "local", model: catalog[0].id } : null;
const consented = new Set<string>();
const runs = new Map<string, { cancelled: boolean }>();
const chats = new Map<string, Chat>();
let seq = 0;

const destination = (r: Remote) => `${r.kind}:${r.baseUrl ?? providers.find((p) => p.kind === r.kind)?.defaultUrl ?? ""}`;
const activeRemote = () => {
  const a = active;
  return a?.kind === "remote" ? (remotes.find((r) => r.id === a.id) ?? null) : null;
};

function overview(): LlmOverview {
  const a = active;
  const label =
    a?.kind === "local"
      ? `${catalog.find((m) => m.id === a.model)?.name} (on this computer)`
      : activeRemote()
        ? `${activeRemote()!.label} · ${activeRemote()!.model}`
        : null;
  return {
    active,
    activeLabel: label,
    local: catalog.map((m) => ({
      model: m,
      downloaded: downloaded.has(m.id),
      received: received.get(m.id) ?? 0,
      downloading: downloading.has(m.id),
    })),
    remotes: remotes.map((r) => ({ remote: r, hasKey: keys.has(r.id) })),
    providers,
    localAvailable: true,
    remoteAvailable: true,
    keychain: true,
    activeDestination: activeRemote() ? destination(activeRemote()!) : null,
    activeWhere: activeRemote() ? (activeRemote()!.baseUrl ?? providers.find((p) => p.kind === activeRemote()!.kind)?.defaultUrl ?? providers.find((p) => p.kind === activeRemote()!.kind)!.label) : null,
  };
}

function err(kind: string, message: string) {
  return { kind, message };
}

function context(host: MockHost, scope: ChatScope, question: string): NoteSummary[] {
  const all = host.summaries();
  let picked: NoteSummary[];
  switch (scope.kind) {
    case "note": {
      const self = all.find((n) => n.id === scope.id);
      const words = new Set((self?.excerpt ?? "").toLowerCase().match(/[a-z]{5,}/g) ?? []);
      const near = all.filter((n) => n.id !== scope.id && (n.excerpt.toLowerCase().match(/[a-z]{5,}/g) ?? []).some((w) => words.has(w)));
      picked = [...(self ? [self] : []), ...near.slice(0, 5)];
      break;
    }
    case "month":
      picked = all.filter((n) => n.day.startsWith(scope.month));
      break;
    case "label":
      picked = all.filter((n) => n.labels.some((l) => l.toLowerCase() === scope.label.toLowerCase()));
      break;
    case "openQuestions":
      picked = all.filter((n) => /\w\?/.test(n.excerpt));
      break;
    case "theme":
      picked = all.filter((n) => n.labels.includes("VC"));
      break;
    default: {
      const words = question.toLowerCase().match(/[a-z]{4,}/g) ?? [];
      picked = all.filter((n) => words.some((w) => (n.excerpt + " " + n.labels.join(" ")).toLowerCase().includes(w)));
    }
  }
  return picked.slice(0, 8).sort((a, b) => a.created.localeCompare(b.created));
}

function fakeAnswer(question: string, notes: ChatNote[], host: MockHost): string {
  if (!notes.length) return "Your notes don't cover this yet.";
  const first = (id: string) =>
    host
      .body(id)
      .replace(/[#>*_`[\]-]/g, "")
      .split(/(?<=[.?!])\s/)[0]
      .trim()
      .toLowerCase();
  const a = notes[0];
  const b = notes[notes.length - 1];
  return [
    `Short answer: ${/\bchang/i.test(question) ? "it shifted from something you read to something you now question" : "your notes keep returning to the same idea"}.`,
    `It starts with “${first(a.id).slice(0, 90)}” [${a.n}], written on ${a.day}.`,
    notes.length > 1 ? `By ${b.day} you had moved to “${first(b.id).slice(0, 90)}” [${b.n}].` : "",
    "What's still open is whether this holds up when you test it against what you actually do.",
  ]
    .filter(Boolean)
    .join("\n\n");
}

export async function mockLlm(cmd: string, a: Record<string, any>, host: MockHost): Promise<{ handled: boolean; value?: unknown }> {
  const done = (value?: unknown) => ({ handled: true, value });
  switch (cmd) {
    case "llm_overview":
      return done(overview());
    case "llm_download": {
      const m = catalog.find((x) => x.id === a.id)!;
      if (downloading.has(m.id) || downloaded.has(m.id)) return done();
      const step = m.bytes / 30;
      const t = setInterval(() => {
        const r = Math.min(m.bytes, (received.get(m.id) ?? 0) + step);
        received.set(m.id, r);
        host.emit("llm-download", { id: m.id, state: "downloading", received: r, total: m.bytes, message: null });
        if (r >= m.bytes) {
          clearInterval(t);
          downloading.delete(m.id);
          received.delete(m.id);
          downloaded.add(m.id);
          host.emit("llm-download", { id: m.id, state: "done", received: m.bytes, total: m.bytes, message: null });
        }
      }, 100);
      downloading.set(m.id, t);
      return done();
    }
    case "llm_cancel_download": {
      const t = downloading.get(a.id);
      if (t) {
        clearInterval(t);
        downloading.delete(a.id);
        host.emit("llm-download", { id: a.id, state: "cancelled", received: received.get(a.id) ?? 0, total: 0, message: null });
      }
      return done();
    }
    case "llm_delete_model":
      downloaded.delete(a.id);
      received.delete(a.id);
      if (active?.kind === "local" && active.model === a.id) active = null;
      return done();
    case "llm_list_remote_models": {
      const p = providers.find((x) => x.kind === a.kind)!;
      const key = a.apiKey ?? (a.remoteId ? keys.get(a.remoteId) : null);
      if (p.needsKey && !key) throw err("provider", `${p.label} needs an API key`);
      if (key === "bad") throw err("provider", "the provider rejected the API key");
      return done(remoteModels[a.kind]);
    }
    case "llm_save_remote": {
      const input = a.input as RemoteInput;
      const p = providers.find((x) => x.kind === input.kind)!;
      if (!input.model?.trim()) throw err("invalid", "Choose a model");
      if (p.needsUrl && !input.baseUrl?.trim()) throw err("invalid", "Enter the server's address");
      const id = input.id ?? `remote-${++seq}`;
      if (input.apiKey?.trim()) keys.set(id, input.apiKey.trim());
      if (p.needsKey && !keys.has(id)) throw err("invalid", `${p.label} needs an API key`);
      const r: Remote = { id, kind: input.kind, label: input.label?.trim() || p.label, baseUrl: input.baseUrl?.trim() || null, model: input.model.trim() };
      const i = remotes.findIndex((x) => x.id === id);
      if (i >= 0) remotes[i] = r;
      else remotes.push(r);
      return done(r);
    }
    case "llm_remove_remote": {
      const i = remotes.findIndex((x) => x.id === a.id);
      if (i >= 0) remotes.splice(i, 1);
      keys.delete(a.id);
      if (active?.kind === "remote" && active.id === a.id) active = null;
      return done();
    }
    case "llm_use": {
      const next = a.active as ActiveModel;
      if (next.kind === "local" && !downloaded.has(next.model)) throw err("invalid", "Download it first");
      active = next;
      return done();
    }
    case "llm_warm":
      return done();
    case "llm_turn_off":
      active = null;
      return done();
    case "chat_list":
      return done(
        [...chats.values()]
          .sort((x, y) => y.updated.localeCompare(x.updated))
          .map((c) => ({ id: c.id, title: c.title, updated: c.updated, scopeLabel: c.scopeLabel, questions: c.messages.filter((m) => m.role === "user").length })),
      );
    case "chat_get": {
      const c = chats.get(a.id);
      if (!c) throw err("notFound", "That chat was deleted");
      return done(structuredClone(c));
    }
    case "chat_delete":
      chats.delete(a.id);
      return done();
    case "chat_cancel": {
      const run = runs.get(a.id);
      if (run) run.cancelled = true;
      return done();
    }
    case "chat_send": {
      if (!active) throw err("noModel", "Choose a model on the Models page first");
      const text = String(a.text).trim();
      const now = new Date().toISOString();
      let chat = a.chatId ? chats.get(a.chatId) : undefined;
      if (!chat) {
        chat = { id: `chat${++seq}`, title: text.slice(0, 60), created: now, updated: now, scope: a.scope, scopeLabel: a.scopeLabel, approved: null, notes: [], messages: [] };
      }
      if (runs.has(chat.id)) throw err("invalid", "Wait for the answer, or stop it");
      const r = activeRemote();
      if (r && chat.approved !== destination(r)) {
        if (!a.consent) throw err("consent", "Approve sending notes from this chat first");
        chat.approved = destination(r);
        consented.add(destination(r));
      }
      chat.messages.push({ role: "user", text, steps: [], model: null, error: null, at: now });
      chat.updated = now;
      chats.set(chat.id, chat);
      const c = chat;
      const run = { cancelled: false };
      runs.set(c.id, run);
      // Simulate: one step that surfaces notes, then a streamed answer.
      const found = context(host, c.scope, text);
      const nums: number[] = [];
      for (const s of found) {
        let cn = c.notes.find((x) => x.id === s.id);
        if (!cn) {
          cn = { n: c.notes.length + 1, id: s.id, title: s.title, day: s.day, source: s.source?.title ?? null };
          c.notes.push(cn);
        }
        nums.push(cn.n);
      }
      const label = r ? `Searched “${text.split(/\s+/).slice(0, 3).join(" ")}”` : `Found ${found.length} notes`;
      const step = { label, notes: nums };
      const stepNotes = c.notes.filter((x) => nums.includes(x.n));
      const answer = fakeAnswer(text, stepNotes, host);
      const pieces = answer.match(/\S+\s*/g) ?? [];
      let i = 0;
      let out = "";
      setTimeout(() => host.emit("chat-step", { chatId: c.id, step, notes: stepNotes }), 30);
      const t = setInterval(() => {
        if (run.cancelled || i >= pieces.length) {
          clearInterval(t);
          runs.delete(c.id);
          c.messages.push({ role: "assistant", text: out.trim(), steps: [step], model: overview().activeLabel, error: null, at: new Date().toISOString() });
          c.updated = new Date().toISOString();
          host.emit("chat-done", { chatId: c.id });
          return;
        }
        out += pieces[i];
        host.emit("chat-delta", { chatId: c.id, text: pieces[i++] });
      }, 20);
      return done({ chatId: c.id });
    }
    default:
      return { handled: false };
  }
}
