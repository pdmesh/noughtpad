import { describe, expect, it } from "vitest";
import { relatePlaces, type Place } from "./place";

const write = (paneKey: string, noteId: string | null): Place => ({ view: { kind: "write" }, noteId, paneKey });

describe("what counts as the same place", () => {
  it("treats a new note that got saved as the same page", () => {
    expect(relatePlaces(write("new-1", null), write("new-1", "n1"))).toBe("replace");
  });

  it("treats opening a note from a blank page as going somewhere new", () => {
    expect(relatePlaces(write("new-1", null), write("n2", "n2"))).toBe("new");
  });

  it("treats a new chat that got its id as the same page", () => {
    const a: Place = { view: { kind: "chat", id: null, key: "new-9" }, noteId: null, paneKey: "n1" };
    const b: Place = { view: { kind: "chat", id: "c1", key: "new-9" }, noteId: null, paneKey: "n1" };
    expect(relatePlaces(a, b)).toBe("replace");
    expect(relatePlaces(b, b)).toBe("same");
  });

  it("ignores the hidden editor's pane on other pages", () => {
    const a: Place = { view: { kind: "journal" }, noteId: null, paneKey: "n1" };
    const b: Place = { view: { kind: "journal" }, noteId: null, paneKey: "n2" };
    expect(relatePlaces(a, b)).toBe("same");
  });
});
