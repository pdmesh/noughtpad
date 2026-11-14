import { describe, expect, it } from "vitest";
import { back, canBack, canForward, emptyHistory, forward, replaceCurrent, visit, type NavHistory } from "./navHistory";

// Places are strings here: "x!" is "x" after it got its id (a saved new note, a chat that started).
const relate = (prev: string, next: string) => (prev === next ? "same" : next === `${prev}!` ? "replace" : "new");
const walk = (...places: string[]) => places.slice(1).reduce((h, p) => visit(h, p, relate), emptyHistory(places[0]));

describe("navigation history", () => {
  it("goes back and forward through visited places", () => {
    let h: NavHistory<string> = walk("a", "b", "c");
    let r = back(h);
    expect(r.place).toBe("b");
    r = back(r.history);
    expect(r.place).toBe("a");
    h = r.history;
    expect(canBack(h)).toBe(false);
    expect(back(h).place).toBeNull();
    expect(forward(h).place).toBe("b");
  });

  it("starts a new branch after going back", () => {
    const h = visit(back(walk("a", "b", "c")).history, "d", relate);
    expect(h.entries).toEqual(["a", "b", "d"]);
    expect(canForward(h)).toBe(false);
  });

  it("ignores a repeat visit", () => {
    expect(walk("a", "a").entries).toEqual(["a"]);
  });

  it("replaces a new note once it is saved", () => {
    expect(walk("a", "a!").entries).toEqual(["a!"]);
  });

  it("replaces a chat when it gets its id", () => {
    expect(walk("a", "b", "b!").entries).toEqual(["a", "b!"]);
  });

  it("keeps at most cap entries", () => {
    let h = emptyHistory("p0");
    for (let i = 1; i <= 105; i++) h = visit(h, `p${i}`, relate, 100);
    expect(h.entries).toHaveLength(100);
    expect(h.entries.at(-1)).toBe("p105");
    expect(h.index).toBe(99);
  });
});

describe("replacing the current place", () => {
  it("swaps a dead entry in place and keeps the places around it", () => {
    const h = back(walk("a", "x", "b")).history;
    const r = replaceCurrent(h, "blank");
    expect(r.entries).toEqual(["a", "blank", "b"]);
    expect(r.index).toBe(1);
    expect(back(r).place).toBe("a");
  });
});
