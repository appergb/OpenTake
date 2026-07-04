import { describe, expect, it } from "vitest";
import { chooseTicks } from "./ruler";

// SPEC §5.3 (frontend-UI-1to1-SPEC.md:506-513):
//   MAJOR_SECONDS  = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 1200, 1800, 3600]
//   target ~80px (first interval with frames * ppf >= 80)
//   MINOR_SUBDIVISIONS = [10, 5, 4, 2] (first keeping cell >= 12px)
//   fallback: largest major interval (3600s) when nothing fits
//
// Invariant derived from the constants: under normal conditions (major*ppf
// >= 80) minorSubdivisions can only be 10 (when major*ppf >= 120) or 5.
// The 4 / 2 / 1 branches are only reachable on the fallback path where
// major = 3600*fps and major*ppf is small.

describe("chooseTicks — major interval", () => {
  it("picks 1 second at 30 fps / ppf=4 (30 frames * 4 = 120 >= 80)", () => {
    expect(chooseTicks(4, 30).majorInterval).toBe(30);
  });

  it("picks 1 second at 30 fps / ppf=8 (240 >= 80)", () => {
    expect(chooseTicks(8, 30).majorInterval).toBe(30);
  });

  it("picks 10 seconds at 30 fps / ppf=0.5 (300 frames * 0.5 = 150 >= 80; 5s gives 75 < 80)", () => {
    expect(chooseTicks(0.5, 30).majorInterval).toBe(300);
  });

  it("picks 5 seconds at 24 fps / ppf=1 (120 >= 80; 2s gives 48 < 80)", () => {
    expect(chooseTicks(1, 24).majorInterval).toBe(120);
  });

  it("picks 2 seconds at 30 fps / ppf=2 (60*2 = 120 >= 80; 1s gives 60 < 80)", () => {
    expect(chooseTicks(2, 30).majorInterval).toBe(60);
  });

  it("falls back to the largest interval (3600s) when nothing reaches 80px", () => {
    // ppf so small that even 3600s*30fps * ppf < 80.
    expect(chooseTicks(0.0001, 30).majorInterval).toBe(3600 * 30);
  });

  it("selects the FIRST qualifying interval, not the largest", () => {
    // ppf=4 → 1s already qualifies; should not skip to 2s.
    const r = chooseTicks(4, 30);
    expect(r.majorInterval).toBe(30); // 1s, not 60 (2s)
  });

  it("major interval is always an integer multiple of fps", () => {
    // Every entry in MAJOR_SECONDS is an integer; major = sec * fps.
    for (const ppf of [0.5, 1, 2, 4, 8, 16]) {
      const r = chooseTicks(ppf, 30);
      expect(r.majorInterval % 30).toBe(0);
    }
  });

  it("at 60 fps scales intervals by 60 (not 30)", () => {
    // 1s @ 60fps = 60 frames; with ppf=2 → 120 >= 80 → major=60.
    expect(chooseTicks(2, 60).majorInterval).toBe(60);
  });
});

describe("chooseTicks — minor subdivisions", () => {
  it("uses 10 when 10 cells each >= 12px (major=30, ppf=4 → cell=12)", () => {
    expect(chooseTicks(4, 30).minorSubdivisions).toBe(10);
  });

  it("uses 10 when major*ppf exactly 120 (boundary: cell=12 qualifies)", () => {
    // major=60 (2s@30), ppf=2 → 60/10*2 = 12 (>= 12)
    expect(chooseTicks(2, 30).minorSubdivisions).toBe(10);
  });

  it("drops to 5 when major*ppf in [80, 120) (cell at 10 < 12, cell at 5 >= 16)", () => {
    // ppf=3, fps=30 → major=30 (1s), major*ppf=90. cell@10 = 9 < 12, cell@5 = 18 ✓
    expect(chooseTicks(3, 30).minorSubdivisions).toBe(5);
  });

  it("drops to 4 only on the fallback path (very small ppf, major=3600s)", () => {
    // ppf=0.0005, fps=30 → major=108000, major*ppf=54.
    // cell@10 = 5.4, cell@5 = 10.8, cell@4 = 13.5 (>= 12) → 4
    expect(chooseTicks(0.0005, 30).minorSubdivisions).toBe(4);
  });

  it("drops to 2 only on the fallback path (major*ppf < 48)", () => {
    // ppf=0.0004, fps=30 → major=108000, major*ppf=43.2.
    // cell@10 = 4.32, cell@5 = 8.64, cell@4 = 10.8, cell@2 = 21.6 (>= 12) → 2
    expect(chooseTicks(0.0004, 30).minorSubdivisions).toBe(2);
  });

  it("falls back to 1 (no subdivisions) when even 2 cells < 12px", () => {
    // ppf=0.0001, fps=30 → major=108000, major*ppf=10.8.
    // cell@2 = 5.4 < 12 → none qualify → stays at default 1.
    expect(chooseTicks(0.0001, 30).minorSubdivisions).toBe(1);
  });

  it("selects the FIRST qualifying subdivision, not the densest", () => {
    // ppf=4 → cell@10 = 12 already qualifies; should not pick 5/4/2.
    expect(chooseTicks(4, 30).minorSubdivisions).toBe(10);
  });
});

describe("chooseTicks — combined / edge cases", () => {
  it("returns a complete RulerTicks object", () => {
    const r = chooseTicks(4, 30);
    expect(r).toEqual({ majorInterval: 30, minorSubdivisions: 10 });
  });

  it("falls back to safeFps=30 when fps <= 0", () => {
    // fps=0 should behave identically to fps=30.
    expect(chooseTicks(4, 0)).toEqual(chooseTicks(4, 30));
    expect(chooseTicks(4, -1)).toEqual(chooseTicks(4, 30));
  });

  it("handles ppf=0 by falling back to the largest major interval and 1 subdivision", () => {
    // 0 * anything = 0, never >= 80 → major stays at 3600*fps.
    // Then cell = (major/sub) * 0 = 0, never >= 12 → minor stays at 1.
    const r = chooseTicks(0, 30);
    expect(r.majorInterval).toBe(3600 * 30);
    expect(r.minorSubdivisions).toBe(1);
  });

  it("handles very large ppf by picking the smallest interval (1s)", () => {
    // ppf=1000 → 1s*30*1000 = 30000 >= 80 immediately.
    expect(chooseTicks(1000, 30).majorInterval).toBe(30);
  });
});
