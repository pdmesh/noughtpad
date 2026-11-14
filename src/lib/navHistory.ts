// Back/forward across the app, with browser rules: going somewhere new after
// going back drops the places ahead. Generic over the place, so the rules of
// what counts as "the same place" stay with the caller (App.tsx).

export interface NavHistory<T> {
  entries: T[];
  index: number;
}

/** How a new place relates to the current one: nothing to record, the same
 *  place with more detail (a new note that got saved), or somewhere else. */
export type Relation = "same" | "replace" | "new";

export function emptyHistory<T>(first: T): NavHistory<T> {
  return { entries: [first], index: 0 };
}

export function visit<T>(h: NavHistory<T>, place: T, relate: (prev: T, next: T) => Relation, cap = 100): NavHistory<T> {
  const r = relate(h.entries[h.index], place);
  if (r === "same") return h;
  if (r === "replace") return replaceCurrent(h, place);
  const entries = [...h.entries.slice(0, h.index + 1), place].slice(-cap);
  return { entries, index: entries.length - 1 };
}

export function canBack<T>(h: NavHistory<T>): boolean {
  return h.index > 0;
}

export function canForward<T>(h: NavHistory<T>): boolean {
  return h.index < h.entries.length - 1;
}

export function back<T>(h: NavHistory<T>): { history: NavHistory<T>; place: T | null } {
  if (!canBack(h)) return { history: h, place: null };
  return { history: { ...h, index: h.index - 1 }, place: h.entries[h.index - 1] };
}

export function forward<T>(h: NavHistory<T>): { history: NavHistory<T>; place: T | null } {
  if (!canForward(h)) return { history: h, place: null };
  return { history: { ...h, index: h.index + 1 }, place: h.entries[h.index + 1] };
}

/** Swap the current place for another without moving (a place that no longer exists). */
export function replaceCurrent<T>(h: NavHistory<T>, place: T): NavHistory<T> {
  const entries = h.entries.slice();
  entries[h.index] = place;
  return { entries, index: h.index };
}
