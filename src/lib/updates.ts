import { useSyncExternalStore } from "react";
import { api, isApiError, onEvent } from "./api";

/**
 * Checking for updates: quietly in the background (shortly after start-up, then every few hours;
 * see `startUpdateChecks`), or when the user asks (Settings or the app menu). A newer version
 * shows as a button in the sidebar and a line in Settings; nothing is downloaded until the user
 * presses it, and there are no dialogs. Kept outside React so the menu command can start a check
 * before the Settings page has loaded.
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

/**
 * Ask GitHub for a newer version. `quiet` (the background checks) shows nothing while checking
 * and keeps a failure to itself (offline is normal), and leaves an offer or a failed install
 * alone.
 */
export async function checkForUpdates({ quiet = false }: { quiet?: boolean } = {}): Promise<void> {
  if (state.kind === "checking" || state.kind === "installing") return;
  if (quiet && (state.kind === "available" || (state.kind === "error" && state.version))) return;
  const before = state;
  if (!quiet) set({ kind: "checking" });
  try {
    const r = await api.updateCheck();
    // The user may have started an install meanwhile.
    if (getUpdateState().kind === "installing") return;
    set(r.available ? { kind: "available", ...r.available } : { kind: "upToDate", checkedAt: Date.now() });
  } catch (e) {
    if (quiet) set(before);
    else set({ kind: "error", message: message(e) });
  }
}

/** First background check after start-up (out of the way of opening the notes), then the interval. */
export const FIRST_CHECK_MS = 10_000;
export const CHECK_EVERY_MS = 4 * 60 * 60 * 1000;

/** Check quietly soon after start-up and then every few hours. Returns a stop function. */
export function startUpdateChecks(): () => void {
  const check = () => void checkForUpdates({ quiet: true });
  const first = setTimeout(check, FIRST_CHECK_MS);
  const every = setInterval(check, CHECK_EVERY_MS);
  return () => {
    clearTimeout(first);
    clearInterval(every);
  };
}

/** The sidebar button's label, when there's an update to offer or one in progress. */
export function updateButtonLabel(s: UpdateState): string | null {
  switch (s.kind) {
    case "available":
      return `Update to ${s.version}`;
    case "installing": {
      if (s.total && s.downloaded >= s.total) return "Restarting…";
      return s.total ? `Updating… ${Math.floor((s.downloaded / s.total) * 100)}%` : "Updating…";
    }
    case "error":
      return s.version ? `Update to ${s.version}` : null;
    default:
      return null;
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
