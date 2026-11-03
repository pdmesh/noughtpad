// Typed wrappers around the Tauri commands in src-tauri/src/lib.rs.
// When running in a plain browser (`pnpm dev` without Tauri), an in-memory
// mock backend is used so the UI can be developed and screenshotted.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { mock, mockOn } from "./mockBackend";

export type NoteKind = "personal" | "source";

export interface Source {
  title?: string | null;
  url?: string | null;
}

export interface NoteSummary {
  id: string;
  path: string;
  /** Explicit title if set, otherwise derived from the first line. */
  title: string;
  hasTitle: boolean;
  excerpt: string;
  created: string;
  day: string;
  kind: NoteKind;
  labels: string[];
  source: Source | null;
}

export interface NoteDoc {
  id: string;
  path: string;
  content: string;
  title: string | null;
  labels: string[];
  source: Source | null;
  kindOverride: NoteKind | null;
  kind: NoteKind;
  created: string;
  hash: string;
}

export interface EditableMeta {
  title: string | null;
  labels: string[];
  source: Source | null;
  kindOverride: NoteKind | null;
}

export interface SearchHit {
  note: NoteSummary;
  snippet: string;
  score: number;
  /** Matched the words, the meaning (local semantic search), or both. */
  matched: "text" | "meaning" | "both";
}

export type SemanticStatus =
  | { state: "off" }
  | { state: "downloading"; received: number; total: number }
  | { state: "loading" }
  | { state: "indexing"; done: number; total: number }
  | { state: "ready"; notes: number }
  | { state: "error"; message: string };

export interface Related {
  note: NoteSummary;
  score: number;
  strength: "strong" | "possible";
  passage: string;
  sourcePassage: string | null;
  sharedLabels: string[];
  sameSource: string | null;
  /** Days from the current note to the related one (negative = earlier). */
  daysApart: number;
}

export interface MonthCount {
  month: string;
  count: number;
}

export interface ThemeSummary {
  id: string;
  name: string;
  keywords: string[];
  labels: string[];
  noteCount: number;
  personalCount: number;
  sourceCount: number;
  firstDay: string;
  lastDay: string;
  activeMonths: number;
  /** Last 12 calendar months, oldest first. */
  months: MonthCount[];
}

export interface Influence {
  source: NoteSummary;
  later: NoteSummary[];
  themeId: string | null;
  themeName: string | null;
}

export interface OpenQuestion {
  note: NoteSummary;
  themeId: string | null;
  themeName: string | null;
}

export interface IdeasOverview {
  enabled: boolean;
  stale: boolean;
  returning: ThemeSummary[];
  forming: ThemeSummary[];
  dormant: ThemeSummary[];
  other: ThemeSummary[];
  influences: Influence[];
  openQuestions: OpenQuestion[];
}

export interface TimelineEntry {
  note: NoteSummary;
  influencedBy: NoteSummary | null;
  revisits: NoteSummary | null;
  mayRevise: boolean;
  isQuestion: boolean;
}

export interface ThemeTimeline {
  theme: ThemeSummary;
  entries: TimelineEntry[];
}

export interface RelatedNotes {
  enabled: boolean;
  pending: boolean;
  items: Related[];
  /** Suggestions are tuned by this user's own feedback. */
  personalized: boolean;
}

export interface LabelCount {
  label: string;
  count: number;
}

export interface DaySummary {
  day: string;
  notes: NoteSummary[];
}

export interface MonthLog {
  month: string;
  personalCount: number;
  sourceCount: number;
  activeDays: number;
  topLabels: LabelCount[];
  days: DaySummary[];
}

export interface ListQuery {
  label?: string;
  kind?: NoteKind;
  fromDay?: string;
  toDay?: string;
  beforeMs?: number;
  limit?: number;
}

export interface VaultInfo {
  path: string;
  noteCount: number;
  defaultPath: string;
}

export type Change =
  | { type: "unchanged" }
  | { type: "upserted"; id: string; path: string }
  | { type: "removed"; path: string };

// ---- M4: models and thinking ----------------------------------------------

export type ThinkScope =
  | { kind: "notebook" }
  | { kind: "note"; id: string }
  | { kind: "theme"; id: string }
  | { kind: "month"; month: string }
  | { kind: "label"; label: string }
  | { kind: "openQuestions" };

export interface LocalModelInfo {
  id: string;
  name: string;
  maker: string;
  licence: string;
  repo: string;
  revision: string;
  file: string;
  sha256: string;
  bytes: number;
  ramGb: number;
  blurb: string;
  recommended: boolean;
}

export interface LocalEntry {
  model: LocalModelInfo;
  downloaded: boolean;
  /** Bytes of an unfinished (resumable) or running download. */
  received: number;
  downloading: boolean;
}

export type RemoteKind = "anthropic" | "openAi" | "gemini" | "ollama" | "openAiCompatible";

export interface Remote {
  id: string;
  kind: RemoteKind;
  label: string;
  baseUrl?: string | null;
  model: string;
}

export interface ProviderInfo {
  kind: RemoteKind;
  label: string;
  needsKey: boolean;
  needsUrl: boolean;
  defaultUrl: string | null;
}

export type ActiveModel = { kind: "local"; model: string } | { kind: "remote"; id: string };

export interface LlmOverview {
  active: ActiveModel | null;
  activeLabel: string | null;
  local: LocalEntry[];
  remotes: Array<{ remote: Remote; hasKey: boolean }>;
  providers: ProviderInfo[];
  localAvailable: boolean;
  remoteAvailable: boolean;
  /** Keys are kept in the OS keychain (false: only until Nought quits). */
  keychain: boolean;
}

export interface DownloadEvent {
  id: string;
  state: "downloading" | "done" | "cancelled" | "error";
  received: number;
  total: number;
  message: string | null;
}

export interface RemoteInput {
  id?: string;
  kind: RemoteKind;
  label?: string;
  baseUrl?: string;
  model: string;
  apiKey?: string;
}

export interface ProviderView {
  label: string;
  model: string;
  onDevice: boolean;
  destination: string;
}

export interface Prepared {
  notes: NoteSummary[];
  provider: ProviderView | null;
  needsConsent: boolean;
}

export interface ThinkUsage {
  promptTokens: number | null;
  completionTokens: number | null;
  secs: number;
}

export interface ThinkDelta {
  requestId: string;
  text: string;
}

export interface ThinkDone {
  requestId: string;
  cited: string[];
  usage: ThinkUsage;
  error: string | null;
  cancelled: boolean;
}

export interface Kept {
  file: string;
  question: string;
  created: string;
  model: string;
  noteIds: string[];
  answer: string;
}

export interface ApiError {
  kind: "conflict" | "notFound" | "noVault" | "io" | "internal" | "invalid" | "provider" | "consent" | "noModel" | "cancelled";
  message: string;
}

export function isApiError(e: unknown): e is ApiError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) return mock(cmd, args) as Promise<T>;
  return invoke<T>(cmd, args);
}

export const api = {
  vaultInfo: () => call<VaultInfo | null>("vault_info"),
  defaultVault: () => call<string>("default_vault"),
  /** Logs time from process start to an interactive editor (the capture budget). */
  markReady: () => call<number>("mark_ready"),
  openVault: (path: string) => call<VaultInfo>("open_vault", { path }),
  listNotes: (query: ListQuery = {}) => call<NoteSummary[]>("list_notes", { query }),
  search: (query: string, limit = 50) => call<SearchHit[]>("search_notes", { query, limit }),
  getNote: (id: string) => call<NoteDoc>("get_note", { id }),
  createNote: (content: string, meta: EditableMeta) =>
    call<NoteDoc>("create_note", { input: { content, meta } }),
  updateNote: (
    id: string,
    update: { content?: string; meta?: EditableMeta; baseHash?: string },
  ) => call<NoteDoc>("update_note", { id, update }),
  trashNote: (id: string) => call<void>("trash_note", { id }),
  labels: () => call<LabelCount[]>("list_labels"),
  days: (query: ListQuery = {}) => call<DaySummary[]>("list_days", { query }),
  months: () => call<string[]>("list_months"),
  monthLog: (month: string) => call<MonthLog>("month_log", { month }),
  dayCounts: (fromDay: string) => call<Array<{ day: string; count: number }>>("day_counts", { fromDay }),
  rebuildIndex: () => call<{ indexed: number; unchanged: number; removed: number }>("rebuild_index"),
  semanticStatus: () => call<SemanticStatus>("semantic_status"),
  enableSemantic: () => call<SemanticStatus>("enable_semantic"),
  disableSemantic: () => call<SemanticStatus>("disable_semantic"),
  related: (id: string, limit = 8) => call<RelatedNotes>("related_notes", { id, limit }),
  dismissRelated: (a: string, b: string) => call<void>("dismiss_related", { a, b }),
  undismissRelated: (a: string, b: string) => call<void>("undismiss_related", { a, b }),
  relatedOpened: (a: string, b: string) => call<void>("related_opened", { a, b }),
  resetRelatedFeedback: () => call<void>("reset_related_feedback"),
  ideasOverview: () => call<IdeasOverview>("ideas_overview"),
  themeTimeline: (id: string) => call<ThemeTimeline | null>("theme_timeline", { id }),
  noteTheme: (id: string) => call<ThemeSummary | null>("note_theme", { id }),
  llmOverview: () => call<LlmOverview>("llm_overview"),
  llmDownload: (id: string) => call<void>("llm_download", { id }),
  llmCancelDownload: (id: string) => call<void>("llm_cancel_download", { id }),
  llmDeleteModel: (id: string) => call<void>("llm_delete_model", { id }),
  llmListRemoteModels: (kind: RemoteKind, baseUrl?: string, apiKey?: string, remoteId?: string) =>
    call<string[]>("llm_list_remote_models", { kind, baseUrl: baseUrl || null, apiKey: apiKey || null, remoteId: remoteId ?? null }),
  llmSaveRemote: (input: RemoteInput) => call<Remote>("llm_save_remote", { input }),
  llmRemoveRemote: (id: string) => call<void>("llm_remove_remote", { id }),
  llmUse: (active: ActiveModel) => call<void>("llm_use", { active }),
  llmTurnOff: () => call<void>("llm_turn_off"),
  thinkPrepare: (question: string, scope: ThinkScope) => call<Prepared>("think_prepare", { question, scope }),
  thinkAsk: (requestId: string, question: string, noteIds: string[], consent: boolean) =>
    call<void>("think_ask", { requestId, question, noteIds, consent }),
  thinkCancel: (requestId: string) => call<void>("think_cancel", { requestId }),
  keepAnswer: (input: { question: string; answer: string; noteIds: string[]; model: string }) => call<Kept>("keep_answer", { input }),
  keptAnswers: () => call<Kept[]>("kept_answers"),
  forgetKept: (file: string) => call<void>("forget_kept", { file }),
  saveAttachment: (bytes: Uint8Array, extension: string) =>
    call<string>("save_attachment", { bytes: Array.from(bytes), extension }),
};

/** Subscribe to a backend event. Returns an unsubscribe function. */
export async function onEvent<T>(name: string, handler: (payload: T) => void): Promise<() => void> {
  if (!isTauri) return mockOn(name, handler as (p: unknown) => void);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<T>(name, (e) => handler(e.payload));
}

/** Turn a vault-relative path (or URL) into something the webview can load. */
export function assetUrl(vaultRoot: string, rel: string): string {
  if (/^(https?:|data:|blob:)/i.test(rel)) return rel;
  if (!isTauri || !vaultRoot) return rel;
  const sep = vaultRoot.includes("\\") ? "\\" : "/";
  let path = rel;
  try {
    path = decodeURI(rel);
  } catch {
    /* keep as-is */
  }
  return convertFileSrc(`${vaultRoot}${sep}${path.replace(/\//g, sep)}`);
}

export async function pickFolder(): Promise<string | null> {
  if (!isTauri) return "/mock/Nought";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const res = await open({ directory: true, multiple: false, title: "Choose a folder for your notes" });
  return typeof res === "string" ? res : null;
}

export async function openExternal(url: string): Promise<void> {
  if (!isTauri) {
    window.open(url, "_blank", "noopener");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}
