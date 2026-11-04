import { describe, expect, it } from "vitest";
import { ago, describeSync, SYNC_OFF, syncNotices, type SyncSummary } from "./sync";

const now = Date.UTC(2026, 10, 4, 12, 0, 0);
const sum: SyncSummary = {
  uploaded: 0,
  downloaded: 0,
  merged: 0,
  deletedHere: 0,
  deletedThere: 0,
  restored: [],
  conflicts: [],
  tooBig: [],
  changedHere: false,
  userStateChanged: false,
};

describe("sync status in words", () => {
  it("says how long ago", () => {
    expect(ago(now - 10_000, now)).toBe("just now");
    expect(ago(now - 3 * 60_000, now)).toBe("3 min ago");
    expect(ago(now - 2 * 3_600_000, now)).toBe("2 h ago");
  });

  it("describes each state", () => {
    expect(describeSync({ ...SYNC_OFF, state: "idle", lastSync: now - 60_000 }, now)).toBe("Synced 1 min ago");
    expect(describeSync({ ...SYNC_OFF, state: "syncing" }, now)).toBe("Syncing…");
    expect(describeSync({ ...SYNC_OFF, state: "signedOut" }, now)).toMatch(/Sign in again/);
    expect(describeSync({ ...SYNC_OFF, state: "error", lastError: "network: offline" }, now)).toBe(
      "Couldn't sync. Trying again in a minute. network: offline",
    );
  });

  it("explains conflicts, restores and big files", () => {
    expect(syncNotices(null)).toEqual([]);
    expect(syncNotices(sum)).toEqual([]);
    const n = syncNotices({
      ...sum,
      conflicts: [{ path: "2026/a.md", copy: "2026/a (conflict 2026-11-04 1200).md" }],
      restored: ["b.md"],
      tooBig: ["video.mov"],
    });
    expect(n[0]).toBe(
      '1 note was changed on two devices in the same place. The other version is saved next to it as "a (conflict 2026-11-04 1200).md".',
    );
    expect(n[1]).toMatch(/kept with the edit/);
    expect(n[2]).toMatch(/over 50 MB.*video\.mov/);
  });
});
