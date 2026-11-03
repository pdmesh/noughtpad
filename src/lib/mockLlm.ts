// Browser-only stand-in for the M4 commands (models, providers, thinking).
// Downloads and answers are simulated; the privacy rules mirror the backend.

import type { ActiveModel, Kept, LlmOverview, LocalModelInfo, NoteSummary, Prepared, Remote, RemoteInput, ThinkScope } from "./api";

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
let lastDestination: string | null = null;
const runs = new Map<string, { cancelled: boolean }>();
const kept: Kept[] = [];
let seq = 0;

const destination = (r: Remote) => `${r.kind}:${r.baseUrl ?? providers.find((p) => p.kind === r.kind)?.defaultUrl ?? ""}`;
const activeRemote = () => {
  const a = active;
  return a?.kind === "remote" ? (remotes.find((r) => r.id === a.id) ?? null) : null;
};
const needsConsent = (r: Remote) => !consented.has(destination(r)) || (lastDestination !== null && lastDestination !== destination(r));

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
  };
}

function err(kind: string, message: string) {
  return { kind, message };
}

function context(host: MockHost, scope: ThinkScope, question: string): NoteSummary[] {
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

function fakeAnswer(question: string, notes: NoteSummary[], host: MockHost): string {
  if (!notes.length) return "None of the notes you chose cover this.";
  const lines = notes.slice(0, 4).map((n, i) => {
    const first = host
      .body(n.id)
      .replace(/[#>*_`[\]-]/g, "")
      .split(/(?<=[.?!])\s/)[0]
      .trim();
    const verb = n.kind === "source" ? `you noted from ${n.source?.title ?? "a source"}` : "you wrote";
    return `On ${n.day} ${verb}: “${first.slice(0, 140)}” [${i + 1}]`;
  });
  return `${lines.join("\n\n")}\n\n${/\bchang/i.test(question) ? "Read in order, the notes move from what you read to what you now question" : "These are the notes that speak to it most directly"}${notes.length > 1 ? ` [1][${Math.min(notes.length, 4)}]` : " [1]"}.`;
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
    case "llm_turn_off":
      active = null;
      return done();
    case "think_prepare": {
      const r = activeRemote();
      const provider = !active
        ? null
        : active.kind === "local"
          ? { label: catalog.find((m) => m.id === (active as { model: string }).model)!.name, model: "", onDevice: true, destination: "this computer" }
          : { label: r!.label, model: r!.model, onDevice: false, destination: r!.baseUrl ?? providers.find((p) => p.kind === r!.kind)?.defaultUrl ?? r!.label };
      const prepared: Prepared = { notes: context(host, a.scope, a.question), provider, needsConsent: !!r && needsConsent(r) };
      return done(prepared);
    }
    case "think_ask": {
      if (!active) throw err("noModel", "Choose a model on the Models page first");
      if (!a.noteIds.length) throw err("invalid", "No notes to think with");
      const r = activeRemote();
      if (r && needsConsent(r)) {
        if (!a.consent) throw err("consent", "Confirm which notes are sent first");
        consented.add(destination(r));
        lastDestination = destination(r);
      } else if (r) lastDestination = destination(r);
      const all = host.summaries();
      const notes = (a.noteIds as string[]).map((id) => all.find((n) => n.id === id)!).filter(Boolean);
      notes.sort((x, y) => x.created.localeCompare(y.created));
      const text = fakeAnswer(a.question, notes, host);
      const run = { cancelled: false };
      runs.set(a.requestId, run);
      const pieces = text.match(/\S+\s*/g) ?? [];
      const start = performance.now();
      let i = 0;
      const t = setInterval(() => {
        if (run.cancelled || i >= pieces.length) {
          clearInterval(t);
          runs.delete(a.requestId);
          const nums = [...text.matchAll(/\[(\d+)\]/g)].map((m) => Number(m[1]));
          const cited = [...new Set(nums)].filter((n) => n >= 1 && n <= notes.length).map((n) => notes[n - 1].id);
          host.emit("think-done", {
            requestId: a.requestId,
            cited: run.cancelled ? [] : cited,
            usage: { promptTokens: 400, completionTokens: pieces.length, secs: (performance.now() - start) / 1000 },
            error: null,
            cancelled: run.cancelled,
          });
          return;
        }
        host.emit("think-delta", { requestId: a.requestId, text: pieces[i++] });
      }, 25);
      return done();
    }
    case "think_cancel": {
      const run = runs.get(a.requestId);
      if (run) run.cancelled = true;
      return done();
    }
    case "keep_answer": {
      const k: Kept = { file: `kept-${++seq}.md`, created: new Date().toISOString(), ...a.input };
      kept.unshift(k);
      return done(k);
    }
    case "kept_answers":
      return done([...kept]);
    case "forget_kept": {
      const i = kept.findIndex((k) => k.file === a.file);
      if (i >= 0) kept.splice(i, 1);
      return done();
    }
    default:
      return { handled: false };
  }
}
