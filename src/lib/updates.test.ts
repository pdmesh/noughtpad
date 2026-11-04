import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "./api";
import { checkForUpdates, describeUpdate, getUpdateState, installUpdate, resetUpdateState, updateButtonLabel } from "./updates";

describe("updates", () => {
  beforeEach(() => resetUpdateState());
  afterEach(() => vi.restoreAllMocks());

  it("says when this is the newest version", async () => {
    vi.spyOn(api, "updateCheck").mockResolvedValue({ current: "0.1.0", available: null });
    await checkForUpdates();
    expect(getUpdateState().kind).toBe("upToDate");
    expect(describeUpdate(getUpdateState())).toBe("You have the newest version.");
  });

  it("offers a newer version", async () => {
    vi.spyOn(api, "updateCheck").mockResolvedValue({
      current: "0.1.0",
      available: { version: "0.1.1", notes: "Fixes", date: "2026-11-03" },
    });
    await checkForUpdates();
    expect(describeUpdate(getUpdateState())).toBe("Version 0.1.1 is available (2026-11-03).");
  });

  it("shows the backend's plain message when the check fails", async () => {
    vi.spyOn(api, "updateCheck").mockRejectedValue({ kind: "update", message: "Couldn't reach GitHub." });
    await checkForUpdates();
    expect(getUpdateState()).toEqual({ kind: "error", message: "Couldn't reach GitHub." });
  });

  it("keeps the version after a failed install so it can be retried", async () => {
    vi.spyOn(api, "updateCheck").mockResolvedValue({ current: "0.1.0", available: { version: "0.1.1", notes: null, date: null } });
    vi.spyOn(api, "updateInstall").mockRejectedValue({ kind: "update", message: "Signature mismatch" });
    await checkForUpdates();
    await installUpdate();
    expect(getUpdateState()).toEqual({ kind: "error", message: "Signature mismatch", version: "0.1.1" });
  });

  it("describes download progress", () => {
    expect(describeUpdate({ kind: "installing", version: "0.1.1", downloaded: 5, total: 10 })).toBe("Downloading 0.1.1… 50%");
    expect(describeUpdate({ kind: "installing", version: "0.1.1", downloaded: 10, total: 10 })).toBe(
      "Installing 0.1.1… Noughtpad will restart.",
    );
  });

  it("a background check keeps a failure to itself", async () => {
    vi.spyOn(api, "updateCheck").mockRejectedValue({ kind: "update", message: "Couldn't reach GitHub." });
    await checkForUpdates({ quiet: true });
    expect(getUpdateState().kind).toBe("idle");
    expect(updateButtonLabel(getUpdateState())).toBeNull();
  });

  it("a background check offers the update in the sidebar", async () => {
    vi.spyOn(api, "updateCheck").mockResolvedValue({ current: "0.1.4", available: { version: "0.1.5", notes: null, date: null } });
    await checkForUpdates({ quiet: true });
    expect(updateButtonLabel(getUpdateState())).toBe("Update to 0.1.5");
  });

  it("a background check leaves a found update alone", async () => {
    const check = vi
      .spyOn(api, "updateCheck")
      .mockResolvedValue({ current: "0.1.4", available: { version: "0.1.5", notes: null, date: null } });
    await checkForUpdates({ quiet: true });
    await checkForUpdates({ quiet: true });
    expect(check).toHaveBeenCalledTimes(1);
  });

  it("the button shows download progress", () => {
    expect(updateButtonLabel({ kind: "installing", version: "0.1.5", downloaded: 30, total: 60 })).toBe("Updating… 50%");
    expect(updateButtonLabel({ kind: "installing", version: "0.1.5", downloaded: 60, total: 60 })).toBe("Restarting…");
    expect(updateButtonLabel({ kind: "upToDate", checkedAt: 0 })).toBeNull();
  });
});
