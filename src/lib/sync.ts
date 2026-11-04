import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./api";
import { useEvent } from "./useEvent";

export type SyncState = "off" | "idle" | "syncing" | "error" | "signedOut";

export interface SyncSummary {
  uploaded: number;
  downloaded: number;
  merged: number;
  deletedHere: number;
  deletedThere: number;
  restored: string[];
  conflicts: Array<{ path: string; copy: string }>;
  tooBig: string[];
  changedHere: boolean;
  userStateChanged: boolean;
}

export interface SyncInfo {
  /** This build includes Noughtpad Sync. */
  available: boolean;
  /** This build can sign in to Google (it has the client secret). */
  configured: boolean;
  state: SyncState;
  account: string | null;
  signedIn: boolean;
  notebook: string | null;
  lastSync: number | null;
  lastError: string | null;
  last: SyncSummary | null;
  /** Saved setting: sync this notebook. */
  enabled: boolean;
  /** The Drive folder (`Noughtpad/<name>`) this notebook syncs to. */
  notebookName: string | null;
}

export const SYNC_OFF: SyncInfo = {
  available: false,
  configured: false,
  state: "off",
  account: null,
  signedIn: false,
  notebook: null,
  lastSync: null,
  lastError: null,
  last: null,
  enabled: false,
  notebookName: null,
};

/** Live sync status: the command once, then `sync-status` events. */
export function useSync(): SyncInfo {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: ["sync"], queryFn: api.syncStatus, staleTime: Infinity });
  useEvent<Omit<SyncInfo, "enabled" | "notebookName">>("sync-status", (s) => {
    qc.setQueryData<SyncInfo>(["sync"], (old) => ({ ...(old ?? SYNC_OFF), ...s }));
  });
  return data ?? SYNC_OFF;
}

/** "just now", "3 min ago", "2 h ago", else a date. */
export function ago(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  return new Date(ms).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

/** One line for Settings and the top-bar dot's tooltip. */
export function describeSync(s: SyncInfo, now = Date.now()): string {
  switch (s.state) {
    case "syncing":
      return "Syncing…";
    case "signedOut":
      return "Signed out of Google Drive. Sign in again to keep syncing.";
    case "error":
      return `Couldn't sync${s.lastSync ? ` (last synced ${ago(s.lastSync, now)})` : ""}. Trying again in a minute. ${s.lastError ?? ""}`.trim();
    case "idle":
      return s.lastSync ? `Synced ${ago(s.lastSync, now)}` : "Starting…";
    default:
      return "Off";
  }
}

/** Notices worth showing after a sync, in plain words. */
export function syncNotices(sum: SyncSummary | null): string[] {
  if (!sum) return [];
  const out: string[] = [];
  const n = sum.conflicts.length;
  if (n) out.push(`${n} ${n === 1 ? "note was" : "notes were"} changed on two devices in the same place. The other version is saved next to it as "${name(sum.conflicts[0].copy)}"${n > 1 ? " and others" : ""}.`);
  const r = sum.restored.length;
  if (r) out.push(`${r} ${r === 1 ? "note was" : "notes were"} deleted on one device and edited on another, so ${r === 1 ? "it's" : "they're"} kept with the edit.`);
  const b = sum.tooBig.length;
  if (b) out.push(`${b} ${b === 1 ? "file is" : "files are"} over 50 MB and ${b === 1 ? "doesn't" : "don't"} sync: ${sum.tooBig.slice(0, 3).map(name).join(", ")}.`);
  return out;
}

function name(path: string): string {
  return path.split("/").pop() ?? path;
}
