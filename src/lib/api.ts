// Typed wrappers around the Tauri commands in src-tauri/src/lib.rs.
// When running in a plain browser (`pnpm dev` without Tauri), an in-memory
// mock backend is used so the UI can be developed and screenshotted.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { mock } from "./mockBackend";

export type NoteKind = "personal" | "source";

export interface Source {
  title?: string | null;
  url?: string | null;
}

export interface NoteSummary {
  id: string;
  path: string;
  title: string;
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
  labels: string[];
  source: Source | null;
  kindOverride: NoteKind | null;
  kind: NoteKind;
  created: string;
  hash: string;
}

export interface EditableMeta {
  labels: string[];
  source: Source | null;
  kindOverride: NoteKind | null;
}

export interface SearchHit {
  note: NoteSummary;
  snippet: string;
  score: number;
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

export interface ApiError {
  kind: "conflict" | "notFound" | "noVault" | "io" | "internal";
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
  rebuildIndex: () => call<{ indexed: number; unchanged: number; removed: number }>("rebuild_index"),
  saveAttachment: (bytes: Uint8Array, extension: string) =>
    call<string>("save_attachment", { bytes: Array.from(bytes), extension }),
};

/** Subscribe to a backend event. Returns an unsubscribe function. */
export async function onEvent<T>(name: string, handler: (payload: T) => void): Promise<() => void> {
  if (!isTauri) return () => {};
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
