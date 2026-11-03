import { useSyncExternalStore } from "react";
import { api, isApiError, onEvent } from "./api";

/**
 * Checking for updates. Nothing happens unless the user asks (Settings or the app menu); the
 * result shows inline in Settings, with no dialogs. Kept outside React so the menu command can
 * start a check before the Settings page has loaded.
 */
export type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate"; checkedAt: number }
  | { kind: "available"; version: string; notes: string | null; date: string | null }
  | { kind: "installing"; version: string; downloaded: number; total: number | null }
  | { kind: "error"; message: string; version?: string };

let state: UpdateState = { kind: "idle" };
const listeners = new Set<() => void>();

function set(next: UpdateState) {
  state = next;
  listeners.forEach((l) => l());
}

export function getUpdateState(): UpdateState {
  return state;
}

export function useUpdateState(): UpdateState {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => state,
  );
}

const message = (e: unknown) => (isApiError(e) ? e.message : String(e));

export async function checkForUpdates(): Promise<void> {
  if (state.kind === "checking" || state.kind === "installing") return;
  set({ kind: "checking" });
  try {
    const r = await api.updateCheck();
    set(r.available ? { kind: "available", ...r.available } : { kind: "upToDate", checkedAt: Date.now() });
  } catch (e) {
    set({ kind: "error", message: message(e) });
  }
}

/** Downloads, checks the signature, installs, and restarts the app (the promise only settles on failure). */
export async function installUpdate(): Promise<void> {
  const version = state.kind === "available" ? state.version : state.kind === "error" ? state.version : undefined;
  if (!version) return;
  set({ kind: "installing", version, downloaded: 0, total: null });
  const off = await onEvent<{ downloaded: number; total: number | null }>("update-progress", (p) => {
    if (state.kind === "installing") set({ ...state, ...p });
  });
  try {
    await api.updateInstall();
  } catch (e) {
    set({ kind: "error", message: message(e), version });
  } finally {
    off();
  }
}

/** One line for Settings, in plain words. */
export function describeUpdate(s: UpdateState): string | null {
  switch (s.kind) {
    case "idle":
      return null;
    case "checking":
      return "Checking…";
    case "upToDate":
      return "You have the newest version.";
    case "available":
      return `Version ${s.version} is available${s.date ? ` (${s.date})` : ""}.`;
    case "installing": {
      if (s.total && s.downloaded >= s.total) return `Installing ${s.version}… Noughtpad will restart.`;
      const pct = s.total ? ` ${Math.floor((s.downloaded / s.total) * 100)}%` : "";
      return `Downloading ${s.version}…${pct}`;
    }
    case "error":
      return s.message;
  }
}

/** Test hook. */
export function resetUpdateState() {
  set({ kind: "idle" });
}
