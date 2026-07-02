import { describe, expect, it } from "vitest";
import {
  isValidRange,
  normalizeRange,
  rangeContains,
  validRange,
  withRangeEnd,
  withRangeStart,
} from "./timelineRange";

describe("normalizeRange", () => {
  it("keeps an already-ordered range", () => {
    expect(normalizeRange({ startFrame: 10, endFrame: 40 })).toEqual({ startFrame: 10, endFrame: 40 });
  });
  it("swaps an inverted range", () => {
    expect(normalizeRange({ startFrame: 40, endFrame: 10 })).toEqual({ startFrame: 10, endFrame: 40 });
  });
});

describe("isValidRange", () => {
  it("is false for a collapsed (single-endpoint) range", () => {
    expect(isValidRange({ startFrame: 20, endFrame: 20 })).toBe(false);
  });
  it("is true for a non-empty span, even inverted", () => {
    expect(isValidRange({ startFrame: 40, endFrame: 10 })).toBe(true);
  });
});

describe("validRange", () => {
  it("returns null for null", () => {
    expect(validRange(null)).toBeNull();
  });
  it("returns null for a collapsed range", () => {
    expect(validRange({ startFrame: 5, endFrame: 5 })).toBeNull();
  });
  it("returns the normalized range for a valid inverted range", () => {
    expect(validRange({ startFrame: 40, endFrame: 10 })).toEqual({ startFrame: 10, endFrame: 40 });
  });
});

describe("rangeContains", () => {
  it("includes the start, excludes the end (half-open)", () => {
    const r = { startFrame: 10, endFrame: 40 };
    expect(rangeContains(r, 10)).toBe(true);
    expect(rangeContains(r, 39)).toBe(true);
    expect(rangeContains(r, 40)).toBe(false);
    expect(rangeContains(r, 9)).toBe(false);
  });
});

describe("withRangeStart / withRangeEnd", () => {
  it("marks the start, keeping the existing end", () => {
    expect(withRangeStart({ startFrame: 5, endFrame: 30 }, 12)).toEqual({ startFrame: 12, endFrame: 30 });
  });
  it("collapses to a point when there is no existing range", () => {
    expect(withRangeStart(null, 12)).toEqual({ startFrame: 12, endFrame: 12 });
    expect(withRangeEnd(null, 12)).toEqual({ startFrame: 12, endFrame: 12 });
  });
  it("marks the end, keeping the existing start", () => {
    expect(withRangeEnd({ startFrame: 5, endFrame: 30 }, 22)).toEqual({ startFrame: 5, endFrame: 22 });
  });
  it("clamps a negative frame to 0", () => {
    expect(withRangeStart(null, -8)).toEqual({ startFrame: 0, endFrame: 0 });
    expect(withRangeEnd({ startFrame: 4, endFrame: 9 }, -3)).toEqual({ startFrame: 4, endFrame: 0 });
  });
});
