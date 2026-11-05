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

/**
 * Whether a `vault-changed` event may have changed note `id`. An empty list
 * means "something changed, not saying what" (sent after a sync, which writes
 * and re-indexes files itself, so the watcher reports nothing for them).
 */
export function mayHaveChanged(changes: Change[], id: string): boolean {
  return changes.length === 0 || changes.some((c) => c.type === "upserted" && c.id === id);
}

// ---- M4: models and thinking ----------------------------------------------

export type ChatScope =
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
  /** Where notes go with the active provider (null for a local model). */
  activeDestination: string | null;
  /** The same, for people: a server address or the provider's name. */
  activeWhere: string | null;
  local: LocalEntry[];
  remotes: Array<{ remote: Remote; hasKey: boolean }>;
  providers: ProviderInfo[];
  localAvailable: boolean;
  remoteAvailable: boolean;
  /** Keys are kept in the OS keychain (false: only until Noughtpad quits). */
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

export interface ChatNote {
  /** The note's number in this chat, as the model cites it (`[n]`). */
  n: number;
  id: string;
  title: string;
  day: string;
  source: string | null;
}

export interface ChatStep {
  /** e.g. `Searched “ownership”` */
  label: string;
  notes: number[];
}

export interface ChatMessage {
  role: "user" | "assistant";
  text: string;
  steps: ChatStep[];
  model: string | null;
  error: string | null;
  at: string;
}

export interface Chat {
  id: string;
  title: string;
  created: string;
  updated: string;
  scope: ChatScope;
  scopeLabel: string;
  /** Remote destination approved for this chat. */
  approved: string | null;
  notes: ChatNote[];
  messages: ChatMessage[];
}

export interface ChatSummary {
  id: string;
  title: string;
  updated: string;
  scopeLabel: string;
  questions: number;
}

export interface ChatStepEvent {
  chatId: string;
  step: ChatStep;
  notes: ChatNote[];
}

export interface ChatDeltaEvent {
  chatId: string;
  text: string;
}

export interface ChatDoneEvent {
  chatId: string;
}

export interface UpdateCheck {
  current: string;
  available: { version: string; notes: string | null; date: string | null } | null;
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
  autostartEnabled: () => call<boolean>("autostart_enabled"),
  setAutostart: (on: boolean) => call<boolean>("set_autostart", { on }),
  syncStatus: () => call<import("./sync").SyncInfo>("sync_status"),
  /** Opens the browser and waits for Google; returns the account's email. */
  syncSignIn: () => call<string>("sync_sign_in"),
  syncSignOut: () => call<void>("sync_sign_out"),
  syncSetEnabled: (on: boolean) => call<void>("sync_set_enabled", { on }),
  syncNow: () => call<void>("sync_now"),
  appVersion: () => call<string>("app_version"),
  updateCheck: () => call<UpdateCheck>("update_check"),
  /** Restarts the app on success. */
  updateInstall: () => call<void>("update_install"),
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
  /** Load the active local model in the background. */
  llmWarm: () => call<void>("llm_warm"),
  chatList: () => call<ChatSummary[]>("chat_list"),
  chatGet: (id: string) => call<Chat>("chat_get", { id }),
  chatDelete: (id: string) => call<void>("chat_delete", { id }),
  chatSend: (input: { chatId: string | null; scope: ChatScope; scopeLabel: string; text: string; consent: boolean }) =>
    call<{ chatId: string }>("chat_send", input),
  chatCancel: (id: string) => call<void>("chat_cancel", { id }),
  /** Save pasted or dropped bytes into the notebook; returns the vault-relative path. */
  saveAttachment: (bytes: Uint8Array, extension: string): Promise<string> =>
    isTauri
      ? invoke<string>("save_attachment", bytes, { headers: { "x-extension": extension } })
      : (mock("save_attachment", { extension }) as Promise<string>),
  /** Save the image on the system clipboard, if any (for pastes the webview hands over empty). */
  pasteClipboardImage: () => call<string | null>("paste_clipboard_image"),
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
  if (!isTauri) return "/mock/Noughtpad";
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
