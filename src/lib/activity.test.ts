import { describe, expect, it } from "vitest";
import { calendarWeeks, level, streaks, thresholds } from "./activity";

const c = (day: string, count = 1) => ({ day, count });

describe("activity", () => {
  it("builds 53 Monday-first weeks ending with this week", () => {
    const weeks = calendarWeeks(new Date(2026, 10, 4)); // Wed 4 Nov 2026
    expect(weeks).toHaveLength(53);
    expect(weeks[52][0]).toBe("2026-11-02"); // Monday of this week
    expect(weeks[52][2]).toBe("2026-11-04");
    expect(weeks[0][0]).toBe("2025-11-03");
  });

  it("buckets counts, adapting to how much someone writes", () => {
    expect([0, 1, 2, 3, 4, 5, 12].map((n) => level(n))).toEqual([0, 1, 2, 3, 3, 4, 4]);
    const heavy = thresholds(Array.from({ length: 40 }, (_, i) => 4 + (i % 8))); // 4–11 notes a day
    expect(heavy[0]).toBeGreaterThan(4);
    expect([4, 7, 9, 11].map((n) => level(n, heavy))).toEqual([1, 2, 3, 4]);
  });

  it("counts current and longest streaks across month ends", () => {
    const counts = [c("2026-09-29"), c("2026-09-30"), c("2026-10-01"), c("2026-10-02", 3), c("2026-10-20"), c("2026-11-03"), c("2026-11-04")];
    const s = streaks(counts, new Date(2026, 10, 4));
    expect(s).toEqual({ current: 2, longest: 4, activeDays: 7, total: 9 });
  });

  it("keeps the streak alive until the end of today", () => {
    const s = streaks([c("2026-11-02"), c("2026-11-03")], new Date(2026, 10, 4));
    expect(s.current).toBe(2);
    expect(streaks([c("2026-11-01")], new Date(2026, 10, 4)).current).toBe(0);
  });
});
